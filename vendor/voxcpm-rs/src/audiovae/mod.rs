//! AudioVAE v2 decoder port (inference-only, non-streaming).
//!
//! Faithful translation of `vendor/VoxCPM/src/voxcpm/modules/audiovae/audio_vae_v2.py`.
//! Weight normalization is expected to be materialized at load time:
//! `weight = weight_g * weight_v / ||weight_v||` per output channel.

pub mod layers;
pub mod encoder;
pub mod decoder;
pub mod vae;

pub use vae::AudioVae;
