// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pipe sink: writes an encoded stream to the standard input of a child process.

use std::process::Stdio;

use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};

use crate::ConfigError;
use crate::sink::{NowPlaying, OutputSink, SinkError};

/// Settings of a pipe output (`outputs.config`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PipeConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl PipeConfig {
    pub fn parse(output_name: &str, value: &serde_json::Value) -> Result<Self, ConfigError> {
        let config: Self =
            serde_json::from_value(value.clone()).map_err(|error| ConfigError::Output {
                output: output_name.to_owned(),
                reason: format!("invalid pipe settings: {error}"),
            })?;
        if config.command.trim().is_empty() {
            return Err(ConfigError::Output {
                output: output_name.to_owned(),
                reason: "command cannot be empty".to_owned(),
            });
        }
        Ok(config)
    }
}

pub struct PipeSink {
    config: PipeConfig,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    now_playing: NowPlaying,
}

impl PipeSink {
    pub fn new(config: PipeConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            now_playing: NowPlaying::default(),
        }
    }
}

impl OutputSink for PipeSink {
    async fn connect(&mut self) -> Result<(), SinkError> {
        self.disconnect();

        let mut command = Command::new(&self.config.command);
        command
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());

        let mut child = command.spawn().map_err(SinkError::Connect)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            SinkError::Connect(std::io::Error::other("cannot capture child stdin"))
        })?;

        self.child = Some(child);
        self.stdin = Some(stdin);
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), SinkError> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(SinkError::Io(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "pipe child process is not running",
            )));
        };

        if let Some(child) = self.child.as_mut()
            && let Ok(Some(status)) = child.try_wait()
        {
            return Err(SinkError::Io(std::io::Error::other(format!(
                "child process exited prematurely with {status}"
            ))));
        }

        stdin.write_all(data).await.map_err(SinkError::Io)?;
        Ok(())
    }

    async fn update_metadata(&mut self, now_playing: &NowPlaying) -> Result<(), SinkError> {
        self.now_playing = now_playing.clone();
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        self.stdin = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pipes_data_to_cat() {
        let config = PipeConfig {
            command: "cat".to_owned(),
            args: Vec::new(),
        };
        let mut sink = PipeSink::new(config);
        sink.connect().await.unwrap();
        sink.send(b"hello pipe").await.unwrap();
        sink.disconnect();
    }
}
