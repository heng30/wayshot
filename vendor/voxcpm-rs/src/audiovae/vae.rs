//! Top-level AudioVAE v2 wrapper.

use crate::audiovae::decoder::CausalDecoder;
use crate::audiovae::encoder::CausalEncoder;
use crate::config::AudioVaeConfig;
use burn::prelude::*;

#[derive(Module, Debug)]
pub struct AudioVae {
    pub encoder: CausalEncoder,
    pub decoder: CausalDecoder,
    #[module(skip)]
    pub config: AudioVaeConfig,
}

impl AudioVae {
    pub fn new(config: AudioVaeConfig, device: &Device) -> Self {
        let encoder = CausalEncoder::new(
            config.encoder_dim,
            config.latent_dim,
            &config.encoder_rates,
            config.depthwise,
            device,
        );
        let decoder = CausalDecoder::new(
            config.latent_dim,
            config.decoder_dim,
            &config.decoder_rates,
            config.depthwise,
            config.use_noise_block,
            config.sr_bin_boundaries.clone(),
            &config.cond_type,
            device,
        );
        Self {
            encoder,
            decoder,
            config,
        }
    }

    /// Encode raw waveform `[B, 1, T]` → latent `[B, C, T']` (µ only).
    pub fn encode(&self, audio: Tensor<3>) -> Tensor<3> {
        let hop: usize = self.config.encoder_rates.iter().product();
        let t = audio.dims()[2];
        let pad = (t + hop - 1) / hop * hop - t;
        let audio = if pad > 0 {
            audio.pad((0, pad, 0, 0), 0.0)
        } else {
            audio
        };
        self.encoder.forward_mu(audio)
    }

    /// Decode latent `[B, C, T']` → waveform `[B, 1, T]`.
    pub fn decode(&self, z: Tensor<3>) -> Tensor<3> {
        let target_sr = self.config.out_sample_rate as i32;
        self.decoder.forward(z, target_sr)
    }

    pub fn sample_rate(&self) -> usize {
        self.config.sample_rate
    }
    pub fn out_sample_rate(&self) -> usize {
        self.config.out_sample_rate
    }
}
