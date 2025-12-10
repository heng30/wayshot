use audiopus::{
    coder::{Decoder, Encoder},
    error::Error as OpusError,
    {Application as Bitrate, Channels, SampleRate},
};
use mp4m::AudioProcessor;
use std::time::Duration;
use thiserror::Error;

pub const OPUS_SAMPLE_RATE: u64 = 48000;

#[derive(Debug, Error)]
pub enum OpusCoderError {
    #[error("Opus encoding/decoding error: {0}")]
    OpusError(#[from] OpusError),

    #[error("Invalid input data")]
    InvalidInput,

    #[error("Invalid output data")]
    InvalidOutput,

    #[error("Resample error: {0}")]
    ResampleError(String),
}

pub struct OpusCoder {
    decoder: Decoder,
    encoder: Encoder,
    channels: Channels,
    sample_rate: u32,
    frame_size: usize,
}

impl OpusCoder {
    pub fn new(sample_rate: u32, channels: Channels) -> Result<Self, OpusCoderError> {
        let encoder = Encoder::new(SampleRate::Hz48000, channels, Bitrate::Voip)?;
        let decoder = Decoder::new(SampleRate::Hz48000, channels)?;
        let frame_size = OPUS_SAMPLE_RATE as usize * 20 / 1000; // 960 samples for 20ms at 48kHz

        Ok(Self {
            decoder,
            encoder,
            channels,
            sample_rate,
            frame_size,
        })
    }

    pub fn encode(&mut self, input: &[f32]) -> Result<Vec<u8>, OpusCoderError> {
        let expected_frame_size = (self.sample_rate as usize * 20) / 1000;
        if input.len() != expected_frame_size * self.channels as usize {
            return Err(OpusCoderError::InvalidInput);
        }

        let input = if self.sample_rate != SampleRate::Hz48000 as u32 {
            let samples = AudioProcessor::<f32>::resample_audio(
                input,
                self.sample_rate,
                SampleRate::Hz48000 as u32,
                self.channels as u16,
            )
            .map_err(|e| OpusCoderError::ResampleError(e.to_string()))?;

            if samples.len() != self.samples_per_frame() {
                return Err(OpusCoderError::ResampleError(format!(
                    "expected frame size: {}, got {}",
                    self.samples_per_frame(),
                    samples.len()
                )));
            }

            samples
        } else {
            input.to_vec()
        };

        let mut i16_samples = Vec::with_capacity(input.len());
        for &sample in &input {
            let clamped = sample.clamp(-1.0, 1.0);
            let i16_val = (clamped * i16::MAX as f32) as i16;
            i16_samples.push(i16_val);
        }

        let mut output = vec![0u8; 4000]; // Max Opus packet size
        let encoded_size = self.encoder.encode(&i16_samples, &mut output)?;

        output.truncate(encoded_size);
        Ok(output)
    }

    // only support Hz48000 sample rate
    pub fn decode(&mut self, input: &[u8]) -> Result<Vec<f32>, OpusCoderError> {
        let packet =
            audiopus::packet::Packet::try_from(input).map_err(|_| OpusCoderError::InvalidInput)?;

        let frame_size = (SampleRate::Hz48000 as usize * 20) / 1000; // 20ms frames
        let mut i16_output = vec![0i16; frame_size * self.channels as usize];
        let mut_signals: audiopus::MutSignals<i16> = i16_output
            .as_mut_slice()
            .try_into()
            .map_err(|_| OpusCoderError::InvalidOutput)?;
        let decoded_samples_per_channel = self.decoder.decode(Some(packet), mut_signals, false)?;
        let decoded_len = decoded_samples_per_channel * self.channels as usize;

        let mut f32_output: Vec<f32> = Vec::with_capacity(decoded_len);
        for &sample in &i16_output[..decoded_len] {
            let f32_val = sample as f32 / (i16::MAX as f32 + 1.0);
            f32_output.push(f32_val);
        }

        Ok(f32_output)
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    pub fn channels(&self) -> Channels {
        self.channels
    }

    pub fn samples_per_frame(&self) -> usize {
        self.frame_size * self.channels as usize
    }

    pub fn frame_duration(&self) -> Duration {
        Duration::from_millis(20)
    }

    pub fn input_samples_per_frame(&self) -> usize {
        ((self.sample_rate as usize * self.channels as usize * 20) / 1000) as usize
    }
}
