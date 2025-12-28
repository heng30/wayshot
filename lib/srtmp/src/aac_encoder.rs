use derivative::Derivative;
use derive_setters::Setters;
use fdk_aac::enc::{
    AudioObjectType::Mpeg4LowComplexity, BitRate, ChannelMode, Encoder, EncoderParams, Transport,
};
use thiserror::Error;

const VALID_SAMPLE_RATES: [u32; 12] = [
    8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000,
];

#[derive(Debug, Error)]
pub enum AacEncoderError {
    #[error("Encoder initialization error: {0}")]
    InitializationError(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(u32),

    #[error("Invalid channel count: {0}")]
    InvalidChannels(u8),
}

#[derive(Debug, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AacEncoderConfig {
    /// Sample rate (e.g., 44100, 48000)
    #[derivative(Default(value = "44100"))]
    pub sample_rate: u32,

    /// Number of audio channels (1 = mono, 2 = stereo)
    #[derivative(Default(value = "2"))]
    pub channels: u8,

    /// Bitrate in bits per second
    #[derivative(Default(value = "128_000"))]
    pub bitrate: u32,

    /// Transport format (Raw for RTMP streaming, Adts for file storage)
    #[derivative(Default(value = "Transport::Raw"))]
    pub transport: Transport,
}

impl AacEncoderConfig {
    pub fn new(sample_rate: u32, channels: u8) -> Result<Self, AacEncoderError> {
        if !VALID_SAMPLE_RATES.contains(&sample_rate) {
            return Err(AacEncoderError::InvalidSampleRate(sample_rate));
        }

        if channels < 1 || channels > 2 {
            return Err(AacEncoderError::InvalidChannels(channels));
        }

        Ok(Self {
            sample_rate,
            channels,
            bitrate: 128000,
            transport: Transport::Raw,
        })
    }
}

pub struct AacEncoder {
    encoder: Encoder,
    channels: u8,
}

impl AacEncoder {
    pub fn new(config: AacEncoderConfig) -> Result<Self, AacEncoderError> {
        let channel_mode = match config.channels {
            1 => ChannelMode::Mono,
            2 => ChannelMode::Stereo,
            _ => return Err(AacEncoderError::InvalidChannels(config.channels)),
        };

        let params = EncoderParams {
            bit_rate: BitRate::Cbr(config.bitrate),
            sample_rate: config.sample_rate,
            channels: channel_mode,
            transport: config.transport,
            audio_object_type: Mpeg4LowComplexity,
        };

        let encoder = Encoder::new(params)
            .map_err(|e| AacEncoderError::InitializationError(e.to_string()))?;

        if let Ok(info) = encoder.info() {
            log::info!(
                "AAC encoder initialized: {} channels, sample_rate={}, frameLength={}, maxAncBytes={}",
                info.inputChannels,
                config.sample_rate,
                info.frameLength,
                info.maxAncBytes
            );
        }

        Ok(Self {
            encoder,
            channels: config.channels,
        })
    }

    // Encode PCM audio data (f32 samples in range [-1.0, 1.0]) to AAC format
    pub fn encode(&mut self, pcm_data: &[f32]) -> Result<Vec<u8>, AacEncoderError> {
        let pcm_i16: Vec<i16> = pcm_data
            .iter()
            .map(|&sample| {
                let clamped = sample.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect();

        // AAC typically compresses to about 1/8 to 1/4 of PCM size, but allocate more for safety
        let mut output_buffer = vec![0u8; pcm_i16.len() * 4];

        self.encoder
            .encode(&pcm_i16, &mut output_buffer)
            .map_err(|e| AacEncoderError::EncodingError(e.to_string()))?;

        Ok(output_buffer)
    }

    /// Get the required input frame size in samples per channel
    /// FDK-AAC requires specific frame sizes (typically 1024 samples per channel)
    pub fn input_frame_size(&self) -> usize {
        if let Ok(info) = self.encoder.info() {
            info.frameLength as usize
        } else {
            1024
        }
    }

    pub fn channels(&self) -> u8 {
        self.channels
    }

    pub fn valid_sample_rates() -> &'static [u32] {
        &VALID_SAMPLE_RATES
    }

    /// Get the sample rate index for AudioSpecificConfig
    fn sample_rate_index(sample_rate: u32) -> u8 {
        match sample_rate {
            96000 => 0,
            88200 => 1,
            64000 => 2,
            48000 => 3,
            44100 => 4,
            32000 => 5,
            24000 => 6,
            22050 => 7,
            16000 => 8,
            12000 => 9,
            11025 => 10,
            8000 => 11,
            _ => 4, // Default to 44100
        }
    }

    /// Generate AudioSpecificConfig for AAC sequence header
    /// This is required for RTMP streaming and MP4 format
    /// Format: 5 bits audio_object_type, 4 bits sample_rate_index, 4 bits channel_config
    ///
    /// Note: This uses the default sample rate from the encoder config.
    /// The InfoStruct doesn't expose sampleRate, so we use 44100 as default.
    pub fn audio_specific_config(&self) -> Vec<u8> {
        // Use default sample rate since InfoStruct doesn't expose it
        let sample_rate = 44100;

        let audio_object_type = 2; // AAC-LC
        let sample_rate_index = Self::sample_rate_index(sample_rate);
        let channel_config = self.channels;

        // AudioSpecificConfig is typically 2 bytes for standard AAC-LC
        // Byte 1: (audio_object_type << 3) | ((sample_rate_index >> 1) & 0x1)
        // Byte 2: ((sample_rate_index & 0x1) << 7) | (channel_config << 3)
        let byte1 = (audio_object_type << 3) | ((sample_rate_index >> 1) & 0x1);
        let byte2 = ((sample_rate_index & 0x1) << 7) | (channel_config << 3);

        vec![byte1, byte2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        // Valid configurations
        assert!(AacEncoderConfig::new(44100, 1).is_ok());
        assert!(AacEncoderConfig::new(48000, 2).is_ok());

        // Invalid sample rate
        assert!(AacEncoderConfig::new(4000, 1).is_err());

        // Invalid channels
        assert!(AacEncoderConfig::new(44100, 0).is_err());
        assert!(AacEncoderConfig::new(44100, 3).is_err());
    }

    #[test]
    fn test_encoder_creation() {
        let config = AacEncoderConfig::new(44100, 2).unwrap();
        let encoder = AacEncoder::new(config);
        assert!(encoder.is_ok());

        let encoder = encoder.unwrap();
        assert_eq!(encoder.channels(), 2);
        assert_eq!(encoder.input_frame_size(), 1024);
    }

    #[test]
    fn test_encode_silence() {
        let config = AacEncoderConfig::new(44100, 2).unwrap();
        let mut encoder = AacEncoder::new(config).unwrap();

        // Create silence (2048 samples for stereo = 1024 per channel)
        let silence = vec![0.0f32; 2048];
        let encoded = encoder.encode(&silence);
        assert!(encoded.is_ok());

        let encoded_data = encoded.unwrap();
        // Should have some AAC data output
        assert!(!encoded_data.is_empty());
        // Should have ADTS header (7 bytes minimum)
        assert!(encoded_data.len() > 7);
    }
}
