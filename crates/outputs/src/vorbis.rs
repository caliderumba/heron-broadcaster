// SPDX-License-Identifier: AGPL-3.0-or-later

//! Vorbis encoder: master bus (f32, stereo, 48 kHz) to an Ogg Vorbis stream.

use std::num::{NonZeroU8, NonZeroU32};
use std::sync::{Arc, Mutex};

use heron_core::EncoderProfile;
use heron_dsp::StreamResampler;
use serde::Deserialize;
use vorbis_rs::{
    VorbisBitrateManagementStrategy, VorbisEncoder as RawVorbisEncoder, VorbisEncoderBuilder,
};

use crate::ConfigError;
use crate::encoder::{EncodeError, Encoder, INPUT_CHANNELS, INPUT_RATE, StreamInfo};

#[derive(Clone, Default)]
pub struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedBuffer {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    pub fn drain_into(&self, out: &mut Vec<u8>) {
        if let Ok(mut buf) = self.0.lock() {
            out.extend_from_slice(&buf);
            buf.clear();
        }
    }
}

impl std::io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut lock) = self.0.lock() {
            lock.extend_from_slice(buf);
            Ok(buf.len())
        } else {
            Err(std::io::Error::other("mutex poisoned"))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct VorbisOptions {
    /// "abr" (default) or "vbr".
    mode: String,
    /// Quality for VBR mode (-0.1 to 1.0, default 0.4).
    quality: f32,
}

impl Default for VorbisOptions {
    fn default() -> Self {
        Self {
            mode: "abr".to_owned(),
            quality: 0.4,
        }
    }
}

pub struct VorbisEncoder {
    info: StreamInfo,
    encoder: Option<RawVorbisEncoder<SharedBuffer>>,
    sink: SharedBuffer,
    resampler: StreamResampler,
    resampled: Vec<f32>,
    left: Vec<f32>,
    right: Vec<f32>,
    mono: Vec<f32>,
    channels: usize,
}

#[allow(unsafe_code)]
// SAFETY: RawVorbisEncoder exclusively owns its C state and is only accessed by one thread at a time.
unsafe impl Send for VorbisEncoder {}

impl VorbisEncoder {
    pub fn new(profile: &EncoderProfile) -> Result<Self, ConfigError> {
        let invalid = |reason: String| ConfigError::Profile {
            profile: profile.name.clone(),
            reason,
        };
        let options: VorbisOptions = if profile.options.is_null() {
            VorbisOptions::default()
        } else {
            serde_json::from_value(profile.options.clone())
                .map_err(|error| invalid(format!("invalid options: {error}")))?
        };

        if !matches!(profile.channels, 1 | 2) {
            return Err(invalid(format!(
                "{} channels; Vorbis supports 1 or 2",
                profile.channels
            )));
        }

        let non_zero_channels = NonZeroU8::new(profile.channels)
            .ok_or_else(|| invalid("invalid channels count".to_owned()))?;

        let non_zero_rate = NonZeroU32::new(profile.sample_rate)
            .ok_or_else(|| invalid("invalid sample rate".to_owned()))?;

        let bitrate_kbps = profile.bitrate_kbps.unwrap_or(128);
        if !(32..=500).contains(&bitrate_kbps) {
            return Err(invalid(format!(
                "Vorbis bitrate {bitrate_kbps} kbps out of range (32 to 500 kbps)"
            )));
        }

        let strategy = if options.mode.to_lowercase() == "vbr" {
            let q = options.quality.clamp(-0.1, 1.0);
            VorbisBitrateManagementStrategy::QualityVbr { target_quality: q }
        } else {
            let bps = NonZeroU32::new(u32::from(bitrate_kbps) * 1_000)
                .ok_or_else(|| invalid("invalid bitrate".to_owned()))?;
            VorbisBitrateManagementStrategy::Abr {
                average_bitrate: bps,
            }
        };

        let sink = SharedBuffer::new();
        let mut builder = VorbisEncoderBuilder::new(non_zero_rate, non_zero_channels, sink.clone())
            .map_err(|e| invalid(format!("cannot create Vorbis encoder builder: {e}")))?;

        builder.bitrate_management_strategy(strategy);
        builder.minimum_page_data_size(Some(1024));

        let encoder = builder
            .build()
            .map_err(|e| invalid(format!("cannot build Vorbis encoder: {e}")))?;

        let resampler = StreamResampler::new(INPUT_RATE, profile.sample_rate, INPUT_CHANNELS)
            .map_err(|e| invalid(e.to_string()))?;

        let info = StreamInfo {
            content_type: "audio/ogg",
            sample_rate: profile.sample_rate,
            channels: profile.channels,
            bitrate_kbps: Some(bitrate_kbps),
        };

        Ok(Self {
            info,
            encoder: Some(encoder),
            sink,
            resampler,
            resampled: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            mono: Vec::new(),
            channels: profile.channels as usize,
        })
    }
}

impl Encoder for VorbisEncoder {
    fn info(&self) -> &StreamInfo {
        &self.info
    }

    fn encode(&mut self, pcm: &[f32], out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let Some(encoder) = self.encoder.as_mut() else {
            return Err(EncodeError::Vorbis(
                "encoder was already finished".to_owned(),
            ));
        };

        self.resampled.clear();
        self.resampler.process(pcm, &mut self.resampled)?;

        if self.channels == 1 {
            self.mono.clear();
            for frame in self.resampled.as_chunks::<INPUT_CHANNELS>().0 {
                self.mono.push((frame[0] + frame[1]) * 0.5);
            }
            encoder
                .encode_audio_block([&self.mono[..]])
                .map_err(|e| EncodeError::Vorbis(e.to_string()))?;
        } else {
            self.left.clear();
            self.right.clear();
            for frame in self.resampled.as_chunks::<INPUT_CHANNELS>().0 {
                self.left.push(frame[0]);
                self.right.push(frame[1]);
            }
            encoder
                .encode_audio_block([&self.left[..], &self.right[..]])
                .map_err(|e| EncodeError::Vorbis(e.to_string()))?;
        }

        self.sink.drain_into(out);
        Ok(())
    }

    fn flush(&mut self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        if let Some(encoder) = self.encoder.take() {
            let _ = encoder.finish();
        }
        self.sink.drain_into(out);
        Ok(())
    }
}
