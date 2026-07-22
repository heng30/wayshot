#![allow(unexpected_cfgs)]
#![warn(missing_docs)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! `speakrs` implements the full pyannote `community-1` style diarization
//! pipeline in Rust: segmentation, powerset decode, overlap-add aggregation,
//! binarization, embedding, PLDA, and VBx clustering.
//!
//! There is no Python runtime in the library path. Inference runs on ONNX
//! Runtime, and the rest of the pipeline stays in Rust.
//!
//! # Usage
//!
//! ```toml
//! # NVIDIA GPU
//! speakrs = { version = "0.5", features = ["cuda"] }
//!
//! # CPU only
//! speakrs = "0.5"
//!
//! # System OpenBLAS
//! speakrs = { version = "0.5", default-features = false, features = ["online", "openblas-system"] }
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use speakrs::{ExecutionMode, OwnedDiarizationPipeline};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let mut pipeline = OwnedDiarizationPipeline::from_pretrained(ExecutionMode::Cpu)?;
//!
//!     let audio: Vec<f32> = load_your_mono_16khz_audio_here();
//!     let result = pipeline.run(&audio)?;
//!
//!     print!("{}", result.rttm("my-audio"));
//!     Ok(())
//! }
//! # fn load_your_mono_16khz_audio_here() -> Vec<f32> { unimplemented!() }
//! ```
//!
//! ## Speaker turns
//!
//! ```no_run
//! # use speakrs::{ExecutionMode, OwnedDiarizationPipeline};
//!
//! # let mut pipeline = OwnedDiarizationPipeline::from_pretrained(ExecutionMode::Cpu)?;
//! # let audio: Vec<f32> = vec![];
//! let result = pipeline.run(&audio)?;
//!
//! for segment in result.discrete_diarization.to_segments() {
//!     println!("{:.3} - {:.3}  {}", segment.start, segment.end, segment.speaker);
//! }
//! # Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
//! ```
//!
//! ## Background queue
//!
//! [`QueueSender`] and [`QueueReceiver`] run a background worker. Push audio
//! from any thread and read results as they finish:
//!
//! ```no_run
//! use speakrs::{ExecutionMode, OwnedDiarizationPipeline, QueuedDiarizationRequest};
//!
//! # fn receive_files() -> Vec<(String, Vec<f32>)> { vec![] }
//! let pipeline = OwnedDiarizationPipeline::from_pretrained(ExecutionMode::Cpu)?;
//! let (tx, rx) = pipeline.into_queued()?;
//!
//! std::thread::spawn(move || {
//!     for (file_id, audio) in receive_files() {
//!         tx.push(QueuedDiarizationRequest::new(file_id, audio)).unwrap();
//!     }
//! });
//!
//! for result in rx {
//!     let result = result?;
//!     print!("{}", result.result?.rttm(&result.file_id));
//! }
//! # Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
//! ```
//!
//! ## Local models
//!
//! For offline or airgapped setups, load models from a local directory:
//!
//! ```no_run
//! use std::path::Path;
//! use speakrs::{ExecutionMode, OwnedDiarizationPipeline};
//!
//! # let audio: Vec<f32> = vec![];
//! let mut pipeline = OwnedDiarizationPipeline::from_dir(
//!     Path::new("/path/to/models"),
//!     ExecutionMode::Cpu,
//! )?;
//! let result = pipeline.run(&audio)?;
//! # Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
//! ```
//!
//! # Choosing a mode
//!
//! | Mode | Backend | Step | Use it for |
//! |------|---------|------|------------|
//! | `cpu` | ONNX Runtime CPU | 1s | CPU runs and widest compatibility |
//! | `cuda` | ONNX Runtime CUDA | 1s | NVIDIA GPU |
//! | `cuda-fast` | ONNX Runtime CUDA | 2s | NVIDIA GPU for higher throughput |
//!
//! The `*-fast` modes move the segmentation window every 2 seconds instead of
//! every 1 second. That gives the pipeline fewer windows to score, so it can be much faster, but speaker changes
//! may land a little farther from the exact word or pause where they happened.
//!
//! Use the 1 second modes when you care about exactly when each speaker starts and stops,
//! short clips, interviews with quick back-and-forth, or audio you plan to subtitle or edit. The 2 second modes
//! are usually worth trying for long recordings where speed matters more than exact speaker-change times, such as
//! meetings, lectures, podcasts, or bulk archives.
//!
//! # Benchmarks
//!
//! VoxConverse dev, collar=0ms:
//!
//! | Platform | Implementation | DER | Time | RTFx |
//! |----------|----------------|-----|------|------|
//! | RTX 4090 | `speakrs` `cuda` | **7.0%** | 1236s | 59x |
//! | RTX 4090 | `speakrs` `cuda-fast` | 7.4% | 604s | **121x** |
//! | RTX 4090 | pyannote community-1 (CUDA) | 7.2% | 2312s | 32x |
//!
//! See [benchmarks/](https://github.com/avencera/speakrs/tree/master/benchmarks) for
//! the full tables across all datasets.
//!
//! # Why not pyannote-rs?
//!
//! [pyannote-rs](https://github.com/thewh1teagle/pyannote-rs) is the main
//! Rust-only comparison point, but it targets a different tradeoff.
//!
//! | | `speakrs` | `pyannote-rs` |
//! |-|-----------|---------------|
//! | Pipeline | Full pyannote `community-1` style pipeline | Simpler window-level pipeline |
//! | Aggregation | Overlap-add plus binarization | No overlap-add or binarization |
//! | Clustering | PLDA + VBx | Cosine threshold |
//! | Goal | Stay close to pyannote behavior on CPU/CUDA | Lightweight Rust diarization |
//!
//! # Models
//!
//! With the default `online` feature, models download on first use from
//! [avencera/speakrs-models](https://huggingface.co/avencera/speakrs-models).
//! Set `SPEAKRS_MODELS_DIR` if you want to force a local bundle instead.
//!
//! # Features and build notes
//!
//! Common features:
//!
//! - `online` (default): model download via [`ModelManager`]
//! - `cuda`: NVIDIA CUDA backend via ONNX Runtime
//! - `load-dynamic`: load the ONNX Runtime library at startup instead of static linking
//!
//! BLAS backends matter if you disable default features:
//!
//! - `x86_64` defaults to statically linked Intel MKL
//! - non-`x86_64` defaults to statically linked OpenBLAS and needs a C toolchain
//! - no-default builds must enable exactly one of `intel-mkl`, `openblas-static`, or `openblas-system`
//!
//! ```toml
//! speakrs = { version = "0.5", default-features = false, features = ["online", "intel-mkl"] }
//! speakrs = { version = "0.5", default-features = false, features = ["online", "openblas-system"] }
//! ```
//!
//! The ONNX Runtime dependency (`ort` 2.0.0-rc.12) is still pre-release.
//!
//! # Public API
//!
//! Start here:
//!
//! - [`OwnedDiarizationPipeline`]: pipeline entry point
//! - [`QueueSender`] and [`QueueReceiver`]: background worker interface
//! - [`DiarizationResult`]: frame-level activations, segments, clusters, embeddings, RTTM
//! - [`PipelineConfig`] and [`RuntimeConfig`]: tuning knobs
//! - [`ModelManager`]: model download when `online` is enabled
//! - [`Segment`]: a single speaker turn


pub(crate) mod binarize;
pub(crate) mod clustering;
/// Segmentation and embedding model wrappers
pub mod inference;
pub(crate) mod linalg;
/// Diarization error rate (DER) evaluation utilities
#[cfg(feature = "_metrics")]
pub mod metrics;
/// Model paths and HuggingFace download support
pub mod models;
/// High-level diarization pipeline and result types
pub mod pipeline;
pub(crate) mod powerset;
pub(crate) mod reconstruct;
/// Speaker segments, merging, and RTTM output
pub mod segment;
pub(crate) mod utils;

// crate-root re-exports for the main import path
pub use inference::ExecutionMode;
pub use models::ModelBundle;
#[cfg(feature = "online")]
#[cfg_attr(docsrs, doc(cfg(feature = "online")))]
pub use models::ModelManager;
pub use pipeline::{
    AhcConfig, BatchInput, BinarizeConfig, DiarizationPipeline, DiarizationResult,
    OwnedDiarizationPipeline, PipelineBuilder, PipelineConfig, PipelineError, QueueError,
    QueueReceiver, QueueReceiverIter, QueueSender, QueuedDiarizationJobId,
    QueuedDiarizationRequest, QueuedDiarizationResult, RuntimeConfig, VbxConfig,
};
pub use segment::Segment;

#[cfg(feature = "_metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "_metrics")))]
pub use powerset::PowersetMapping;
