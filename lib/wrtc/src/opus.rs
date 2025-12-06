use audiopus::{
    coder::{Decoder, Encoder},
    error::Error as OpusError,
    {Application as Bitrate, Channels, SampleRate},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpusCoderError {
    #[error("Opus encoding/decoding error: {0}")]
    OpusError(#[from] OpusError),

    #[error("Invalid input data")]
    InvalidInput,

    #[error("Invalid output data")]
    InvalidOutput,
}

pub struct OpusCoder {
    decoder: Decoder,
    encoder: Encoder,
    channels: Channels,
    sample_rate: SampleRate,
    frame_size: usize,
}

impl OpusCoder {
    pub fn new(sample_rate: u32, channels: Channels) -> Result<Self, OpusCoderError> {
        let sample_rate: i32 = sample_rate
            .try_into()
            .map_err(|_| OpusCoderError::InvalidInput)?;
        let sample_rate =
            SampleRate::try_from(sample_rate).map_err(|_| OpusCoderError::InvalidInput)?;

        let encoder = Encoder::new(sample_rate, channels, Bitrate::Voip)?;
        let decoder = Decoder::new(sample_rate, channels)?;

        // Frame size depends on sample rate (common values: 48000 -> 960, 24000 -> 480, 16000 -> 320, 12000 -> 240, 8000 -> 160)
        let frame_size = (sample_rate as usize * 20) / 1000; // 20ms frames

        Ok(Self {
            decoder,
            encoder,
            channels,
            sample_rate,
            frame_size,
        })
    }

    pub fn encode(&mut self, input: &[f32]) -> Result<Vec<u8>, OpusCoderError> {
        if input.len() != self.frame_size * self.channels as usize {
            return Err(OpusCoderError::InvalidInput);
        }

        let mut i16_samples: Vec<i16> = Vec::with_capacity(input.len());
        for &sample in input {
            let clamped = sample.clamp(-1.0, 1.0);
            let i16_val = (clamped * i16::MAX as f32) as i16;
            i16_samples.push(i16_val);
        }

        let mut output = vec![0u8; 4000]; // Max Opus packet size
        let encoded_size = self.encoder.encode(&i16_samples, &mut output)?;

        output.truncate(encoded_size);
        Ok(output)
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<Vec<f32>, OpusCoderError> {
        let packet =
            audiopus::packet::Packet::try_from(input).map_err(|_| OpusCoderError::InvalidInput)?;

        let mut i16_output = vec![0i16; self.frame_size * self.channels as usize];
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

    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    pub fn samples_per_frame(&self) -> usize {
        self.frame_size * self.channels as usize
    }
}
