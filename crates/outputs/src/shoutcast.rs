// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shoutcast v1 and v2 source client.

use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

use crate::ConfigError;
use crate::connection::{Connection, HandedConnection};
use crate::encoder::StreamInfo;
use crate::sink::{NowPlaying, OutputSink, SinkError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_HEADER: usize = 16 * 1024;
const USER_AGENT: &str = concat!("HeronBroadcaster/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShoutcastVersion {
    V1,
    V2,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShoutcastConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub password: String,
    #[serde(default = "default_stream_id")]
    pub stream_id: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub public_url: Option<String>,
}

fn default_port() -> u16 {
    8000
}

fn default_stream_id() -> u32 {
    1
}

impl ShoutcastConfig {
    pub fn parse(output: &str, config: &serde_json::Value) -> Result<Self, ConfigError> {
        let invalid = |reason: String| ConfigError::Output {
            output: output.to_owned(),
            reason,
        };
        let config: Self = serde_json::from_value(config.clone())
            .map_err(|error| invalid(format!("invalid Shoutcast settings: {error}")))?;
        if config.host.trim().is_empty() {
            return Err(invalid("host cannot be empty".to_owned()));
        }
        if config.password.trim().is_empty() {
            return Err(invalid("password cannot be empty".to_owned()));
        }
        Ok(config)
    }

    pub fn listen_url(&self) -> String {
        self.public_url.clone().unwrap_or_else(|| {
            format!(
                "http://{}:{}/stream/{}",
                self.host, self.port, self.stream_id
            )
        })
    }
}

pub struct ShoutcastSink {
    version: ShoutcastVersion,
    config: ShoutcastConfig,
    info: StreamInfo,
    relay_socket: Option<PathBuf>,
    connection: Option<Connection>,
    adopted: Option<HandedConnection>,
    now_playing: NowPlaying,
}

impl ShoutcastSink {
    pub fn new(
        version: ShoutcastVersion,
        config: ShoutcastConfig,
        info: StreamInfo,
        relay_socket: Option<PathBuf>,
        handed: Option<HandedConnection>,
    ) -> Self {
        Self {
            version,
            config,
            info,
            relay_socket,
            connection: None,
            adopted: handed,
            now_playing: NowPlaying::default(),
        }
    }

    async fn connect_v1(&mut self, mut stream: Connection) -> Result<(), SinkError> {
        let bitrate_str = self
            .info
            .bitrate_kbps
            .map(|b| b.to_string())
            .unwrap_or_else(|| "128".to_owned());

        let req = format!(
            "{password}\r\nicy-name:{name}\r\nicy-genre:{genre}\r\nicy-url:{url}\r\nicy-pub:{pub_flag}\r\nicy-br:{br}\r\n\r\n",
            password = self.config.password,
            name = self.config.name.as_deref().unwrap_or("Heron Stream"),
            genre = self.config.genre.as_deref().unwrap_or("Various"),
            url = self.config.url.as_deref().unwrap_or(""),
            pub_flag = if self.config.public { 1 } else { 0 },
            br = bitrate_str,
        );

        stream
            .write_all(req.as_bytes())
            .await
            .map_err(SinkError::Io)?;
        let mut response = Vec::new();
        read_response(&mut stream, &mut response).await?;
        let text = String::from_utf8_lossy(&response);

        if !text.starts_with("OK")
            && !text.starts_with("HTTP/1.0 200")
            && !text.starts_with("HTTP/1.1 200")
        {
            return Err(SinkError::Refused(text.trim().to_owned()));
        }

        self.connection = Some(stream);
        Ok(())
    }

    async fn connect_v2(&mut self, mut stream: Connection) -> Result<(), SinkError> {
        let auth = if self.config.password.contains(':') {
            base64::engine::general_purpose::STANDARD.encode(&self.config.password)
        } else {
            base64::engine::general_purpose::STANDARD
                .encode(format!("admin:{}", self.config.password))
        };

        let req = format!(
            "POST /stream?sid={sid} HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             User-Agent: {USER_AGENT}\r\n\
             Authorization: Basic {auth}\r\n\
             Content-Type: {content_type}\r\n\
             ice-name: {name}\r\n\
             ice-public: {pub_flag}\r\n\
             \r\n",
            sid = self.config.stream_id,
            host = self.config.host,
            port = self.config.port,
            auth = auth,
            content_type = self.info.content_type,
            name = self.config.name.as_deref().unwrap_or("Heron Stream"),
            pub_flag = if self.config.public { 1 } else { 0 },
        );

        stream
            .write_all(req.as_bytes())
            .await
            .map_err(SinkError::Io)?;
        let mut response = Vec::new();
        let status = read_response(&mut stream, &mut response).await?;

        if status != 200 {
            return Err(SinkError::Refused(format!("server returned HTTP {status}")));
        }

        self.connection = Some(stream);
        Ok(())
    }
}

impl OutputSink for ShoutcastSink {
    async fn connect(&mut self) -> Result<(), SinkError> {
        self.connection = None;
        if let Some(handed) = self.adopted.take() {
            self.connection = Some(Connection::adopt(handed)?);
            return Ok(());
        }

        let stream = timeout(
            CONNECT_TIMEOUT,
            Connection::open(
                &self.config.host,
                self.config.port,
                self.relay_socket.as_deref(),
            ),
        )
        .await
        .map_err(|_| SinkError::Timeout("connecting to Shoutcast"))??;

        match self.version {
            ShoutcastVersion::V1 => self.connect_v1(stream).await,
            ShoutcastVersion::V2 => self.connect_v2(stream).await,
        }
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), SinkError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(SinkError::Io(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "not connected to Shoutcast",
            )));
        };

        timeout(WRITE_TIMEOUT, connection.write_all(data))
            .await
            .map_err(|_| SinkError::Timeout("sending audio to Shoutcast"))?
            .map_err(SinkError::Io)?;

        Ok(())
    }

    async fn update_metadata(&mut self, now_playing: &NowPlaying) -> Result<(), SinkError> {
        self.now_playing = now_playing.clone();
        let Some(title) = now_playing.stream_title() else {
            return Ok(());
        };

        // Shoutcast 1/2 admin cgi update: /admin.cgi?mode=updinfo&pass=...&song=...
        let song_encoded = utf8_percent_encode(&title, NON_ALPHANUMERIC).to_string();
        let path = match self.version {
            ShoutcastVersion::V1 => {
                format!(
                    "/admin.cgi?mode=updinfo&pass={}&song={}",
                    utf8_percent_encode(&self.config.password, NON_ALPHANUMERIC),
                    song_encoded
                )
            }
            ShoutcastVersion::V2 => {
                format!(
                    "/admin.cgi?sid={}&mode=updinfo&pass={}&song={}",
                    self.config.stream_id,
                    utf8_percent_encode(&self.config.password, NON_ALPHANUMERIC),
                    song_encoded
                )
            }
        };

        let req = format!(
            "GET {path} HTTP/1.0\r\n\
             Host: {host}:{port}\r\n\
             User-Agent: {USER_AGENT}\r\n\
             \r\n",
            host = self.config.host,
            port = self.config.port,
        );

        if let Ok(Ok(mut admin_conn)) = timeout(
            CONNECT_TIMEOUT,
            Connection::open(
                &self.config.host,
                self.config.port,
                self.relay_socket.as_deref(),
            ),
        )
        .await
        {
            let _ = admin_conn.write_all(req.as_bytes()).await;
        }

        Ok(())
    }

    fn disconnect(&mut self) {
        self.connection = None;
    }

    fn take_connection(&mut self) -> Option<HandedConnection> {
        self.connection.take()?.hand_over().ok()
    }
}

async fn read_response(
    stream: &mut (impl AsyncRead + Unpin),
    response: &mut Vec<u8>,
) -> Result<u16, SinkError> {
    let mut buffer = [0u8; 1024];
    loop {
        let read = stream.read(&mut buffer).await.map_err(SinkError::Io)?;
        if read == 0 {
            return Err(SinkError::Refused(
                "closed before sending headers".to_owned(),
            ));
        }
        response.extend_from_slice(&buffer[..read]);
        if response.windows(4).any(|w| w == b"\r\n\r\n")
            || response.windows(2).any(|w| w == b"\n\n")
        {
            break;
        }
        if response.len() > MAX_RESPONSE_HEADER {
            return Err(SinkError::Refused("response header too large".to_owned()));
        }
    }

    let text = String::from_utf8_lossy(response);
    if text.starts_with("OK") {
        return Ok(200);
    }
    let mut parts = text.split_whitespace();
    let _version = parts.next();
    let status = parts
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(200);
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn shoutcast_v1_connects_and_streams() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let config = ShoutcastConfig {
            host: "127.0.0.1".to_owned(),
            port,
            password: "testpassword".to_owned(),
            stream_id: 1,
            name: Some("Test Radio".to_owned()),
            genre: None,
            url: None,
            public: false,
            public_url: None,
        };

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("testpassword"));
            assert!(req.contains("icy-name:Test Radio"));
            socket
                .write_all(b"OK2\r\nicy-caps:11\r\n\r\n")
                .await
                .unwrap();

            let mut audio = [0u8; 16];
            let n = socket.read(&mut audio).await.unwrap();
            assert_eq!(&audio[..n], b"audio payload");
        });

        let mut sink = ShoutcastSink::new(
            ShoutcastVersion::V1,
            config,
            StreamInfo {
                content_type: "audio/mpeg",
                sample_rate: 44100,
                channels: 2,
                bitrate_kbps: Some(128),
            },
            None,
            None,
        );

        sink.connect().await.unwrap();
        sink.send(b"audio payload").await.unwrap();
        sink.disconnect();
        server.await.unwrap();
    }
}
