// SPDX-License-Identifier: AGPL-3.0-or-later

//! HLS sink: slices an encoded stream into audio segments and maintains a rolling .m3u8 playlist.

use std::collections::VecDeque;
use std::path::Path;

use serde::Deserialize;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::ConfigError;
use crate::encoder::StreamInfo;
use crate::sink::{NowPlaying, OutputSink, SinkError};

const DEFAULT_SEGMENT_DURATION_SECS: u32 = 4;
const DEFAULT_PLAYLIST_WINDOW: usize = 5;
const DEFAULT_PLAYLIST_NAME: &str = "live.m3u8";

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HlsConfig {
    /// Directory where segments and playlist are stored.
    pub directory: String,
    /// Segment duration in seconds (default 4).
    #[serde(default = "default_segment_duration")]
    pub segment_duration_seconds: u32,
    /// Number of segments retained in the rolling playlist (default 5).
    #[serde(default = "default_playlist_window")]
    pub playlist_window: usize,
    /// Name of the playlist file (default "live.m3u8").
    #[serde(default = "default_playlist_name")]
    pub playlist_name: String,
    /// Public URL where listeners or web players reach the playlist.
    #[serde(default)]
    pub public_url: Option<String>,
}

fn default_segment_duration() -> u32 {
    DEFAULT_SEGMENT_DURATION_SECS
}

fn default_playlist_window() -> usize {
    DEFAULT_PLAYLIST_WINDOW
}

fn default_playlist_name() -> String {
    DEFAULT_PLAYLIST_NAME.to_owned()
}

impl HlsConfig {
    pub fn parse(output: &str, config: &serde_json::Value) -> Result<Self, ConfigError> {
        let invalid = |reason: String| ConfigError::Output {
            output: output.to_owned(),
            reason,
        };
        let config: Self = serde_json::from_value(config.clone())
            .map_err(|error| invalid(format!("invalid HLS settings: {error}")))?;
        if config.directory.trim().is_empty() {
            return Err(invalid("directory cannot be empty".to_owned()));
        }
        if config.segment_duration_seconds == 0 {
            return Err(invalid(
                "segment_duration_seconds must be at least 1".to_owned(),
            ));
        }
        if config.playlist_window < 2 {
            return Err(invalid("playlist_window must be at least 2".to_owned()));
        }
        Ok(config)
    }

    pub fn listen_url(&self) -> String {
        self.public_url
            .clone()
            .unwrap_or_else(|| format!("{}/{}", self.directory, self.playlist_name))
    }
}

struct SegmentRecord {
    sequence: u64,
    filename: String,
    duration_secs: f64,
}

pub struct HlsSink {
    config: HlsConfig,
    _info: StreamInfo,
    extension: &'static str,
    target_bytes_per_segment: usize,
    buffer: Vec<u8>,
    segments: VecDeque<SegmentRecord>,
    next_sequence: u64,
    now_playing: NowPlaying,
}

impl HlsSink {
    pub fn new(config: HlsConfig, info: StreamInfo) -> Self {
        let extension = if info.content_type.contains("aac") {
            "aac"
        } else if info.content_type.contains("mpeg") {
            "mp3"
        } else {
            "ogg"
        };

        // Estimate target bytes from bitrate and duration
        let bitrate_bps = u64::from(info.bitrate_kbps.unwrap_or(128)) * 1_000;
        let target_bytes =
            ((bitrate_bps / 8) * u64::from(config.segment_duration_seconds)) as usize;

        Self {
            config,
            _info: info,
            extension,
            target_bytes_per_segment: target_bytes.max(1024),
            buffer: Vec::new(),
            segments: VecDeque::new(),
            next_sequence: 0,
            now_playing: NowPlaying::default(),
        }
    }

    async fn write_segment(&mut self, data: Vec<u8>) -> Result<(), SinkError> {
        let seq = self.next_sequence;
        self.next_sequence += 1;

        let filename = format!("segment_{seq}.{}", self.extension);
        let path = Path::new(&self.config.directory).join(&filename);

        let mut file = File::create(&path).await.map_err(SinkError::Io)?;
        file.write_all(&data).await.map_err(SinkError::Io)?;
        file.flush().await.map_err(SinkError::Io)?;

        let duration = self.config.segment_duration_seconds as f64;
        self.segments.push_back(SegmentRecord {
            sequence: seq,
            filename,
            duration_secs: duration,
        });

        // Prune old segments beyond the rolling window
        while self.segments.len() > self.config.playlist_window {
            if let Some(old) = self.segments.pop_front() {
                let old_path = Path::new(&self.config.directory).join(&old.filename);
                let _ = fs::remove_file(old_path).await;
            }
        }

        self.update_playlist().await?;
        Ok(())
    }

    async fn update_playlist(&self) -> Result<(), SinkError> {
        if self.segments.is_empty() {
            return Ok(());
        }

        let first_seq = self.segments.front().map_or(0, |s| s.sequence);
        let target_duration = self.config.segment_duration_seconds;

        let mut m3u8 = String::new();
        m3u8.push_str("#EXTM3U\n");
        m3u8.push_str("#EXT-X-VERSION:3\n");
        m3u8.push_str(&format!("#EXT-X-TARGETDURATION:{target_duration}\n"));
        m3u8.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{first_seq}\n"));

        for seg in &self.segments {
            m3u8.push_str(&format!("#EXTINF:{:.3},\n", seg.duration_secs));
            m3u8.push_str(&format!("{}\n", seg.filename));
        }

        let dir = Path::new(&self.config.directory);
        let final_path = dir.join(&self.config.playlist_name);
        let tmp_path = dir.join(format!(".{}.tmp", self.config.playlist_name));

        fs::write(&tmp_path, m3u8.as_bytes())
            .await
            .map_err(SinkError::Io)?;

        fs::rename(&tmp_path, &final_path)
            .await
            .map_err(SinkError::Io)?;

        Ok(())
    }
}

impl OutputSink for HlsSink {
    async fn connect(&mut self) -> Result<(), SinkError> {
        fs::create_dir_all(&self.config.directory)
            .await
            .map_err(SinkError::Connect)?;
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), SinkError> {
        self.buffer.extend_from_slice(data);

        while self.buffer.len() >= self.target_bytes_per_segment {
            let chunk: Vec<u8> = self.buffer.drain(..self.target_bytes_per_segment).collect();
            self.write_segment(chunk).await?;
        }

        Ok(())
    }

    async fn update_metadata(&mut self, now_playing: &NowPlaying) -> Result<(), SinkError> {
        self.now_playing = now_playing.clone();
        Ok(())
    }

    fn disconnect(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_segments_and_rolling_playlist() {
        let dir = tempfile::tempdir().unwrap();
        let config = HlsConfig {
            directory: dir.path().to_string_lossy().to_string(),
            segment_duration_seconds: 2,
            playlist_window: 3,
            playlist_name: "test.m3u8".to_owned(),
            public_url: None,
        };

        let info = StreamInfo {
            content_type: "audio/aac",
            sample_rate: 48000,
            channels: 2,
            bitrate_kbps: Some(96),
        };

        let mut sink = HlsSink::new(config, info);
        sink.connect().await.unwrap();

        // Target bytes = 96000 / 8 * 2 = 24000 bytes
        let chunk = vec![0xABu8; 24000];
        // Send 4 segments
        for _ in 0..4 {
            sink.send(&chunk).await.unwrap();
        }

        let playlist_path = dir.path().join("test.m3u8");
        assert!(playlist_path.exists());
        let m3u8_content = std::fs::read_to_string(&playlist_path).unwrap();
        assert!(m3u8_content.contains("#EXTM3U"));
        assert!(m3u8_content.contains("#EXT-X-TARGETDURATION:2"));
        // Window is 3, so sequence starts at 1 (0 was pruned)
        assert!(m3u8_content.contains("#EXT-X-MEDIA-SEQUENCE:1"));
        assert!(!m3u8_content.contains("segment_0.aac"));
        assert!(m3u8_content.contains("segment_1.aac"));
        assert!(m3u8_content.contains("segment_3.aac"));

        // Verify segment_0 was removed from disk
        assert!(!dir.path().join("segment_0.aac").exists());
        assert!(dir.path().join("segment_1.aac").exists());
        assert!(dir.path().join("segment_3.aac").exists());

        sink.disconnect();
    }
}
