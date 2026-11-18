// SPDX-License-Identifier: AGPL-3.0-or-later

//! Live DJ input: Icecast source protocol listener (`SOURCE` or `PUT`).
//!
//! When a DJ connects, automation smoothly ducks/fades out while the DJ audio
//! fades in. On disconnect, automation seamlessly returns without dead air.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::engine::Engine;
use crate::mixer::{AudioSource, CHANNELS, SAMPLE_RATE};

const RING_BUFFER_CAPACITY: usize = 10 * SAMPLE_RATE as usize * CHANNELS;

pub struct LiveAudioSource {
    consumer: rtrb::Consumer<f32>,
    finished: Arc<AtomicBool>,
}

impl LiveAudioSource {
    pub fn new(consumer: rtrb::Consumer<f32>, finished: Arc<AtomicBool>) -> Self {
        Self { consumer, finished }
    }
}

impl AudioSource for LiveAudioSource {
    fn read(&mut self, out: &mut [f32]) -> usize {
        let available = self.consumer.slots().min(out.len());
        let available = available - available % CHANNELS;
        let Ok(chunk) = self.consumer.read_chunk(available) else {
            return 0;
        };
        let (first, second) = chunk.as_slices();
        out[..first.len()].copy_from_slice(first);
        out[first.len()..available].copy_from_slice(second);
        chunk.commit_all();
        available / CHANNELS
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire) && self.consumer.is_empty()
    }
}

pub struct LiveDjServer {
    addr: SocketAddr,
    password: String,
    mount: String,
    engine: Arc<Engine>,
}

impl LiveDjServer {
    pub fn new(addr: SocketAddr, password: String, mount: String, engine: Arc<Engine>) -> Self {
        Self {
            addr,
            password,
            mount,
            engine,
        }
    }

    pub async fn run(
        self,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!(addr = %self.addr, mount = %self.mount, "live DJ listener bound");

        loop {
            tokio::select! {
                Ok((stream, client_addr)) = listener.accept() => {
                    info!(client = %client_addr, "live DJ connection incoming");
                    let password = self.password.clone();
                    let mount = self.mount.clone();
                    let engine = Arc::clone(&self.engine);
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, password, mount, engine).await {
                            warn!(client = %client_addr, error = %e, "live DJ session error");
                        }
                    });
                }
                _ = shutdown.recv() => {
                    info!("live DJ listener shutting down");
                    break;
                }
            }
        }
        Ok(())
    }
}

async fn handle_client(
    mut stream: TcpStream,
    expected_password: String,
    expected_mount: String,
    engine: Arc<Engine>,
) -> anyhow::Result<()> {
    let mut header_buf = [0u8; 4096];
    let mut total_read = 0;

    // Read HTTP request header
    while total_read < header_buf.len() {
        let n = stream.read(&mut header_buf[total_read..]).await?;
        if n == 0 {
            anyhow::bail!("client disconnected before sending full header");
        }
        total_read += n;
        if header_buf[..total_read]
            .windows(4)
            .any(|w| w == b"\r\n\r\n")
            || header_buf[..total_read].windows(2).any(|w| w == b"\n\n")
        {
            break;
        }
    }

    let header_text = String::from_utf8_lossy(&header_buf[..total_read]);
    let first_line = header_text.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_uppercase();
    let path = parts.next().unwrap_or_default();

    if method != "SOURCE" && method != "PUT" {
        stream
            .write_all(b"HTTP/1.0 405 Method Not Allowed\r\n\r\n")
            .await?;
        anyhow::bail!("unsupported method: {method}");
    }

    let mount_matches = path == expected_mount
        || path.starts_with(&format!("{expected_mount}?"))
        || expected_mount == "/live";

    if !mount_matches {
        stream.write_all(b"HTTP/1.0 404 Not Found\r\n\r\n").await?;
        anyhow::bail!("unknown mount: {path}");
    }

    // Authenticate
    let mut authorized = false;
    for line in header_text.lines() {
        let auth_val = line
            .strip_prefix("Authorization: Basic ")
            .or_else(|| line.strip_prefix("authorization: Basic "));

        if let Some(auth) = auth_val
            && let Ok(decoded_vec) =
                base64::engine::general_purpose::STANDARD.decode(auth.trim().as_bytes())
        {
            let creds = String::from_utf8_lossy(&decoded_vec);
            let password = creds.split(':').nth(1).unwrap_or(&creds);
            if password == expected_password {
                authorized = true;
                break;
            }
        }
    }

    if !authorized {
        stream
            .write_all(
                b"HTTP/1.0 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"Heron\"\r\n\r\n",
            )
            .await?;
        anyhow::bail!("unauthorized live DJ attempt");
    }

    // Accept stream
    stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n").await?;
    info!("live DJ authenticated and accepted onto air");

    let (mut producer, consumer) = rtrb::RingBuffer::new(RING_BUFFER_CAPACITY);
    let finished = Arc::new(AtomicBool::new(false));
    let source = LiveAudioSource::new(consumer, Arc::clone(&finished));

    if let Err(e) = engine.start_live(Box::new(source)) {
        error!(error = ?e, "cannot feed live DJ into mixer");
        return Err(anyhow::anyhow!("mixer refused live source"));
    }

    let mut buf = [0u8; 4096];
    let mut sample_buf = Vec::new();

    // Streaming audio loop: reads audio stream from socket and pushes float samples
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => {
                info!("live DJ disconnected (EOF)");
                break;
            }
            Ok(n) => {
                // If raw PCM or decoded audio bytes: convert 16-bit to float stereo
                // or feed to ring buffer
                sample_buf.clear();
                for chunk in buf[..n].as_chunks::<2>().0 {
                    let s16 = i16::from_le_bytes([chunk[0], chunk[1]]);
                    let f = s16 as f32 / 32768.0;
                    sample_buf.push(f);
                }

                if !sample_buf.is_empty() {
                    let mut offset = 0;
                    while offset < sample_buf.len() {
                        let slots = producer.slots().min(sample_buf.len() - offset);
                        if slots > 0 {
                            if let Ok(mut chunk) = producer.write_chunk(slots) {
                                let (first, second) = chunk.as_mut_slices();
                                let first_len = first.len();
                                first.copy_from_slice(&sample_buf[offset..offset + first_len]);
                                second.copy_from_slice(
                                    &sample_buf[offset + first_len..offset + slots],
                                );
                                chunk.commit_all();
                                offset += slots;
                            }
                        } else {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "live DJ stream read error");
                break;
            }
        }
    }

    finished.store(true, Ordering::Release);
    let _ = engine.stop_live();
    info!("live DJ session ended, automation seamlessly restored");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn live_source_reads_and_finishes() {
        let (mut producer, consumer) = rtrb::RingBuffer::new(1024);
        let finished = Arc::new(AtomicBool::new(false));
        let mut source = LiveAudioSource::new(consumer, Arc::clone(&finished));

        assert!(!source.is_finished());

        let samples = [0.1f32, -0.1f32, 0.2f32, -0.2f32];
        let mut chunk = producer.write_chunk(4).unwrap();
        chunk.as_mut_slices().0.copy_from_slice(&samples);
        chunk.commit_all();

        let mut out = [0.0f32; 4];
        let read = source.read(&mut out);
        assert_eq!(read, 2); // 2 stereo frames
        assert_eq!(out, samples);

        finished.store(true, Ordering::Release);
        assert!(source.is_finished());
    }

    #[tokio::test]
    async fn rejects_unauthorized_and_wrong_method() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let state_dir = tempfile::tempdir().unwrap();
            let library_dir = tempfile::tempdir().unwrap();
            let options = crate::engine::EngineOptions::new(
                library_dir.path().to_path_buf(),
                state_dir.path().to_path_buf(),
            );
            let engine = Engine::start(options, tokio::runtime::Handle::current()).unwrap();
            let _ =
                handle_client(stream, "secretpass".to_owned(), "/live".to_owned(), engine).await;
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client
            .write_all(b"GET /live HTTP/1.0\r\n\r\n")
            .await
            .unwrap();
        let mut response = [0u8; 1024];
        let n = client.read(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response[..n]);
        assert!(text.contains("405 Method Not Allowed"));

        let _ = server_task.await;
    }

    #[tokio::test]
    async fn rejects_unauthorized_password() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let state_dir = tempfile::tempdir().unwrap();
            let library_dir = tempfile::tempdir().unwrap();
            let options = crate::engine::EngineOptions::new(
                library_dir.path().to_path_buf(),
                state_dir.path().to_path_buf(),
            );
            let engine = Engine::start(options, tokio::runtime::Handle::current()).unwrap();
            let _ =
                handle_client(stream, "secretpass".to_owned(), "/live".to_owned(), engine).await;
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client
            .write_all(b"SOURCE /live HTTP/1.0\r\nAuthorization: Basic dXNlcjp3cm9uZw==\r\n\r\n")
            .await
            .unwrap();
        let mut response = [0u8; 1024];
        let n = client.read(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response[..n]);
        assert!(text.contains("401 Unauthorized"));

        let _ = server_task.await;
    }
}
