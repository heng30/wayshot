//! Top-level VoxCPM2 model and the high-level [`VoxCPM`] convenience wrapper.

pub mod model;
pub mod wrapper;

pub use model::{DitStep, InferenceState, VoxCpm2Model};
pub use wrapper::{
    BatchBuilder, CancelToken, GenerateOptions, GenerateOptionsBuilder, GenerateStream, Prompt,
    PromptAudio, VoxCPM, split_sentences,
};
