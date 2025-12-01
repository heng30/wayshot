use crate::errors::Opus2AacError;
use audiopus::MutSignals;
use audiopus::coder::Decoder as OpusDecoder;
use fdk_aac::enc::{AudioObjectType::Mpeg4LowComplexity, Encoder as AacEncoder, EncoderParams};

pub struct Opus2AacTranscoder {
    decoder_channels_size: usize,
    decoder: OpusDecoder,
    encoder: AacEncoder,
    pcm_data: Vec<i16>,
}

impl Opus2AacTranscoder {
    pub fn new(
        decoder_sample_rate: i32,
        decoder_channels: audiopus::Channels,
        encoder_sample_rate: u32,
        encoder_channels: fdk_aac::enc::ChannelMode,
    ) -> Result<Self, Opus2AacError> {
        let decoder = OpusDecoder::new(
            audiopus::SampleRate::try_from(decoder_sample_rate)?,
            decoder_channels,
        )?;
        let encoder = AacEncoder::new(EncoderParams {
            bit_rate: fdk_aac::enc::BitRate::VbrMedium,
            transport: fdk_aac::enc::Transport::Raw,
            channels: encoder_channels,
            sample_rate: encoder_sample_rate,
            audio_object_type: Mpeg4LowComplexity,
        })?;

        let decoder_channels_size = match decoder_channels {
            audiopus::Channels::Stereo | audiopus::Channels::Auto => 2,
            audiopus::Channels::Mono => 1,
        };

        Ok(Opus2AacTranscoder {
            decoder_channels_size,
            decoder,
            encoder,
            pcm_data: Vec::new(),
        })
    }

    pub fn transcode(&mut self, input: &[u8]) -> Result<Vec<Vec<u8>>, Opus2AacError> {
        //https://opus-codec.org/docs/opus_api-1.1.2/group__opus__decoder.html#ga7d1111f64c36027ddcb81799df9b3fc9
        let mut pcm_output: Vec<i16> = vec![0; 1024 * 2];
        let input_packet = audiopus::packet::Packet::try_from(input)?;
        let mut_signals = MutSignals::try_from(&mut pcm_output)?;
        let pcm_output_len = self
            .decoder
            .decode(Some(input_packet), mut_signals, false)?;
        self.pcm_data
            .extend_from_slice(&pcm_output[..pcm_output_len * self.decoder_channels_size]);

        let mut aac_output: Vec<u8> = vec![0; 1024 * 2];
        let mut result = Vec::new();
        while self.pcm_data.len() >= 1024 * 2 {
            let pcm = self.pcm_data.split_off(2048);
            let encoder_info = self.encoder.encode(&self.pcm_data, &mut aac_output)?;
            self.pcm_data = pcm;
            if encoder_info.output_size > 0 {
                result.push(aac_output[..encoder_info.output_size].to_vec());
            }
        }

        Ok(result)
    }
}
