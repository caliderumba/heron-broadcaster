// SPDX-License-Identifier: AGPL-3.0-or-later

//! Opus encoder: master bus (f32, stereo, 48 kHz) to an Ogg Opus stream.

use heron_core::EncoderProfile;
use heron_dsp::StreamResampler;
use opus_pure::{
    Application, MAX_PACKET_BYTES, OggOpusWriter, OpusEncoder as PureOpusEncoder, OpusHead,
    RateControl,
};
use serde::Deserialize;

use crate::ConfigError;
use crate::encoder::{EncodeError, Encoder, INPUT_CHANNELS, INPUT_RATE, StreamInfo};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OpusOptions {
    /// "audio" (default), "voip", or "low_delay".
    application: String,
    /// Whether variable bitrate is used (default true).
    vbr: bool,
    /// Encoder complexity from 0 (lowest CPU) to 10 (highest quality).
    complexity: u8,
}

impl Default for OpusOptions {
    fn default() -> Self {
        Self {
            application: "audio".to_owned(),
            vbr: true,
            complexity: 10,
        }
    }
}

pub struct OpusEncoder {
    info: StreamInfo,
    encoder: PureOpusEncoder,
    writer: OggOpusWriter<Vec<u8>>,
    resampler: StreamResampler,
    resampled: Vec<f32>,
    mono: Vec<f32>,
    pcm_buffer: Vec<f32>,
    packet_buffer: Vec<u8>,
    channels: usize,
    frame_samples: usize,
}

impl OpusEncoder {
    pub fn new(profile: &EncoderProfile) -> Result<Self, ConfigError> {
        let invalid = |reason: String| ConfigError::Profile {
            profile: profile.name.clone(),
            reason,
        };
        let options: OpusOptions = if profile.options.is_null() {
            OpusOptions::default()
        } else {
            serde_json::from_value(profile.options.clone())
                .map_err(|error| invalid(format!("invalid options: {error}")))?
        };

        if !matches!(profile.channels, 1 | 2) {
            return Err(invalid(format!(
                "{} channels; Opus supports 1 or 2",
                profile.channels
            )));
        }
        const RATES: [u32; 5] = [8_000, 12_000, 16_000, 24_000, 48_000];
        if !RATES.contains(&profile.sample_rate) {
            return Err(invalid(format!(
                "Opus does not support {} Hz (expected 8000, 12000, 16000, 24000, or 48000)",
                profile.sample_rate
            )));
        }

        let bitrate_kbps = profile.bitrate_kbps.unwrap_or(128);
        if !(6..=510).contains(&bitrate_kbps) {
            return Err(invalid(format!(
                "Opus bitrate {bitrate_kbps} kbps out of range (6 to 510 kbps)"
            )));
        }

        let app = match options.application.to_lowercase().as_str() {
            "voip" => Application::Voip,
            "low_delay" | "restricted_lowdelay" => Application::RestrictedLowDelay,
            _ => Application::Audio,
        };

        let mut encoder =
            PureOpusEncoder::new(profile.sample_rate as i32, profile.channels as usize, app)
                .map_err(|e| invalid(format!("cannot create Opus encoder: {e}")))?;

        encoder.bitrate_bps = i32::from(bitrate_kbps) * 1_000;
        encoder.complexity = i32::from(options.complexity.min(10));
        encoder.rate_control = if options.vbr {
            RateControl::Vbr
        } else {
            RateControl::Cbr
        };

        let head = OpusHead::for_encoder(&encoder, profile.sample_rate);
        let mut writer = OggOpusWriter::new(Vec::new(), head)
            .map_err(|e| invalid(format!("cannot create Ogg Opus writer: {e}")))?;
        // Flush pages around 1024 bytes to keep latency low for live streaming.
        writer.set_page_target(1024);

        let resampler = StreamResampler::new(INPUT_RATE, profile.sample_rate, INPUT_CHANNELS)
            .map_err(|e| invalid(e.to_string()))?;

        // 20 ms frame size in samples per channel at the configured sample rate.
        let frame_samples = (profile.sample_rate as usize * 20) / 1_000;

        let info = StreamInfo {
            content_type: "audio/ogg",
            sample_rate: profile.sample_rate,
            channels: profile.channels,
            bitrate_kbps: Some(bitrate_kbps),
        };

        Ok(Self {
            info,
            encoder,
            writer,
            resampler,
            resampled: Vec::new(),
            mono: Vec::new(),
            pcm_buffer: Vec::new(),
            packet_buffer: vec![0u8; MAX_PACKET_BYTES],
            channels: profile.channels as usize,
            frame_samples,
        })
    }

    fn encode_chunk(&mut self, chunk: &[f32]) -> Result<(), EncodeError> {
        let n = self
            .encoder
            .encode(chunk, self.frame_samples, &mut self.packet_buffer)
            .map_err(|e| EncodeError::Opus(e.to_string()))?;

        // Duration in 48 kHz samples as required by RFC 7845.
        let samples_48k = (48_000 * 20) / 1_000; // 960
        self.writer
            .write_packet_with_duration(&self.packet_buffer[..n], samples_48k)
            .map_err(|e| EncodeError::Opus(e.to_string()))?;

        Ok(())
    }
}

impl Encoder for OpusEncoder {
    fn info(&self) -> &StreamInfo {
        &self.info
    }

    fn encode(&mut self, pcm: &[f32], out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.resampled.clear();
        self.resampler.process(pcm, &mut self.resampled)?;

        let input = if self.channels == 1 {
            self.mono.clear();
            for frame in self.resampled.as_chunks::<INPUT_CHANNELS>().0 {
                self.mono.push((frame[0] + frame[1]) * 0.5);
            }
            &self.mono
        } else {
            &self.resampled
        };

        self.pcm_buffer.extend_from_slice(input);

        let step = self.frame_samples * self.channels;
        while self.pcm_buffer.len() >= step {
            let chunk: Vec<f32> = self.pcm_buffer.drain(..step).collect();
            self.encode_chunk(&chunk)?;
        }

        if let Some(sink) = self.writer.get_mut() {
            out.extend_from_slice(sink);
            sink.clear();
        }

        Ok(())
    }

    fn flush(&mut self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let step = self.frame_samples * self.channels;
        if !self.pcm_buffer.is_empty() {
            self.pcm_buffer.resize(step, 0.0);
            let chunk = std::mem::take(&mut self.pcm_buffer);
            self.encode_chunk(&chunk)?;
        }

        if let Some(sink) = self.writer.get_mut() {
            out.extend_from_slice(sink);
            sink.clear();
        }

        Ok(())
    }
}
