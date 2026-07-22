//! # voxcpm-rs
//!
//! Pure-Rust inference for [VoxCPM2](https://huggingface.co/openbmb/VoxCPM2) built on top
//! of the [Burn](https://burn.dev) ML framework. Supports Vulkan (via `wgpu`) and a CPU
//! fallback through `ndarray`.
//!
//! ## Quick start
//!
//! ```no_run
//! # #[cfg(feature = "cpu")] {
//! use voxcpm_rs::{GenerateOptions, Prompt, PromptAudio, VoxCPM};
//!
//! type B = burn::backend::NdArray<f32>;
//! let device = Default::default();
//! let model: VoxCPM<B> = VoxCPM::from_local("./pretrained_models/VoxCPM2", &device).unwrap();
//!
//! // Zero-shot:
//! let wav = model.generate("Hello, world!", GenerateOptions::default()).unwrap();
//!
//! // Voice cloning from a reference wav:
//! let opts = GenerateOptions::builder()
//!     .timesteps(10)
//!     .prompt(Prompt::Reference { audio: "speaker.wav".into() })
//!     .build();
//! let wav = model.generate("Hello, world!", opts).unwrap();
//!
//! voxcpm_rs::audio::write_wav("out.wav", &wav, model.sample_rate()).unwrap();
//! # }
//! ```
//!
//! See the [`VoxCPM`] struct for the convenience API, or the individual submodules
//! ([`minicpm4`], [`locdit`], [`locenc`], [`audiovae`]) for low-level access.

// Bumped from the default 128 because enabling burn's `fusion` + `autotune`
// features pushes the wgpu-core / naga generic chain past the limit.
#![recursion_limit = "256"]
#![warn(missing_debug_implementations)]

pub mod audio;
pub mod audiovae;
pub mod config;
pub mod error;
pub mod fsq;
pub mod locdit;
pub mod locenc;
pub mod minicpm4;
pub mod tokenizer;
pub mod voxcpm2;
pub mod weights;

pub use audiovae::AudioVae;
pub use config::{
    AudioVaeConfig, CfmConfig, LoraConfig, MiniCpm4Config, RopeScalingConfig, VoxCpm2Config,
    VoxCpmDitConfig, VoxCpmEncoderConfig,
};
pub use error::{Error, Result};
pub use voxcpm2::{
    CancelToken, GenerateOptions, GenerateOptionsBuilder, GenerateStream, Prompt, PromptAudio,
    VoxCPM,
};
pub use burn;
