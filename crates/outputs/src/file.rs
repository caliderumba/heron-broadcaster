// SPDX-License-Identifier: AGPL-3.0-or-later

//! File sink: writes an encoded stream to disk, with optional rotation.

use std::path::PathBuf;
use std::time::Instant;

use jiff::Zoned;
use serde::Deserialize;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::ConfigError;
use crate::sink::{NowPlaying, OutputSink, SinkError};

/// Settings of a file recording output (`outputs.config`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    /// Path pattern, formatted with strftime-like specifiers (%Y, %m, %d, %H, %M, %S).
    pub path: String,
    /// Rotation interval in seconds (e.g. 3600 for hourly files). 0 or missing for none.
    #[serde(default)]
    pub rotation_interval_seconds: Option<u64>,
}

impl FileConfig {
    pub fn parse(output_name: &str, value: &serde_json::Value) -> Result<Self, ConfigError> {
        let config: Self =
            serde_json::from_value(value.clone()).map_err(|error| ConfigError::Output {
                output: output_name.to_owned(),
                reason: format!("invalid file settings: {error}"),
            })?;
        if config.path.trim().is_empty() {
            return Err(ConfigError::Output {
                output: output_name.to_owned(),
                reason: "path cannot be empty".to_owned(),
            });
        }
        Ok(config)
    }

    /// Expands date/time specifiers for the given zoned timestamp.
    pub fn format_path(&self, now: &Zoned) -> PathBuf {
        let formatted = now.strftime(&self.path).to_string();
        PathBuf::from(formatted)
    }
}

pub struct FileSink {
    config: FileConfig,
    file: Option<File>,
    current_path: Option<PathBuf>,
    opened_at: Option<Instant>,
    now_playing: NowPlaying,
}

impl FileSink {
    pub fn new(config: FileConfig) -> Self {
        Self {
            config,
            file: None,
            current_path: None,
            opened_at: None,
            now_playing: NowPlaying::default(),
        }
    }

    async fn open_file(&mut self) -> Result<(), SinkError> {
        let now = Zoned::now();
        let target_path = self.config.format_path(&now);

        if let Some(parent) = target_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(SinkError::Connect)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target_path)
            .await
            .map_err(SinkError::Connect)?;

        self.file = Some(file);
        self.current_path = Some(target_path);
        self.opened_at = Some(Instant::now());
        Ok(())
    }

    fn should_rotate(&self) -> bool {
        let Some(interval) = self.config.rotation_interval_seconds.filter(|&i| i > 0) else {
            return false;
        };
        let Some(opened) = self.opened_at else {
            return false;
        };
        opened.elapsed().as_secs() >= interval
    }
}

impl OutputSink for FileSink {
    async fn connect(&mut self) -> Result<(), SinkError> {
        self.open_file().await
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), SinkError> {
        if self.should_rotate() {
            if let Some(mut old) = self.file.take() {
                let _ = old.flush().await;
            }
            self.open_file().await?;
        }

        let Some(file) = self.file.as_mut() else {
            self.open_file().await?;
            let file = self.file.as_mut().ok_or_else(|| {
                SinkError::Io(std::io::Error::other("failed to open output file"))
            })?;
            file.write_all(data).await.map_err(SinkError::Io)?;
            file.flush().await.map_err(SinkError::Io)?;
            return Ok(());
        };

        file.write_all(data).await.map_err(SinkError::Io)?;
        file.flush().await.map_err(SinkError::Io)?;
        Ok(())
    }

    async fn update_metadata(&mut self, now_playing: &NowPlaying) -> Result<(), SinkError> {
        self.now_playing = now_playing.clone();
        Ok(())
    }

    fn disconnect(&mut self) {
        self.file = None;
        self.opened_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_audio_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let path_template = dir
            .path()
            .join("rec_%Y%m%d_%H%M%S.mp3")
            .to_string_lossy()
            .to_string();

        let config = FileConfig {
            path: path_template,
            rotation_interval_seconds: Some(1),
        };

        let mut sink = FileSink::new(config);
        sink.connect().await.unwrap();
        sink.send(b"first block").await.unwrap();

        let first_file = sink.current_path.clone().unwrap();
        assert!(first_file.exists());
        let content = std::fs::read(&first_file).unwrap();
        assert_eq!(content, b"first block");

        // Wait for rotation interval to pass
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

        sink.send(b"second block").await.unwrap();
        let second_file = sink.current_path.clone().unwrap();
        assert!(second_file.exists());

        sink.disconnect();
    }
}
