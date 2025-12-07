use audiopus::{
    coder::{Decoder, Encoder},
    error::Error as OpusError,
    {Application as Bitrate, Channels, SampleRate},
};
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
            let samples = Self::resample_audio(
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

    pub fn input_samples_per_frame(&self) -> usize {
        ((self.sample_rate as usize * self.channels as usize * 20) / 1000) as usize
    }

    fn resample_audio(
        input_samples: &[f32],
        input_sample_rate: u32,
        output_sample_rate: u32,
        channels: u16,
    ) -> Result<Vec<f32>, OpusCoderError> {
        if input_sample_rate == output_sample_rate {
            return Ok(input_samples.to_vec());
        }

        let channels = channels as usize;
        let input_frames = input_samples.len() / channels;
        let ratio = output_sample_rate as f64 / input_sample_rate as f64;
        let output_frames = (input_frames as f64 * ratio).round() as usize;
        let output_samples = output_frames * channels;
        let mut output = vec![0.0f32; output_samples];

        // Simple linear interpolation resampling for each channel independently
        for ch in 0..channels {
            let input_channel: Vec<f32> = input_samples
                .iter()
                .skip(ch)
                .step_by(channels)
                .cloned()
                .collect();

            for out_frame in 0..output_frames {
                let input_pos = out_frame as f64 / ratio;
                let input_frame = input_pos.floor() as usize;
                let fraction = input_pos - input_frame as f64;

                if input_frame + 1 >= input_channel.len() {
                    // At the end, just copy the last sample
                    output[out_frame * channels + ch] = input_channel[input_channel.len() - 1];
                } else {
                    // Linear interpolation between neighboring samples
                    let sample1 = input_channel[input_frame];
                    let sample2 = input_channel[input_frame + 1];
                    let interpolated = sample1 + (sample2 - sample1) * fraction as f32;
                    output[out_frame * channels + ch] = interpolated;
                }
            }
        }

        let actual_ratio = output_samples as f64 / input_samples.len() as f64;
        let expected_ratio = output_sample_rate as f64 / input_sample_rate as f64;
        let ratio_error = (actual_ratio - expected_ratio).abs();
        if ratio_error > 0.001 {
            return Err(OpusCoderError::ResampleError(format!(
                "Ratio error: expected {:.6}, got {:.6}",
                expected_ratio, actual_ratio
            )));
        }

        Ok(output)
    }
}
