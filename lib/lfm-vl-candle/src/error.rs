//! Error types for the LFM2.5-VL inference library.

use std::path::PathBuf;

/// Errors that can occur during model loading, inference, or processing.
#[derive(Debug, thiserror::Error)]
pub enum LfmError {
    /// A model file was not found or could not be read.
    #[error("file not found: {0}")]
    FileNotFound(PathBuf),

    /// A required configuration value is missing or invalid.
    #[error("config error: {0}")]
    Config(String),

    /// An error occurred during image processing.
    #[error("image processing error: {0}")]
    ImageProcessing(String),

    /// An error occurred during tokenization.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    /// An error occurred during chat template rendering.
    #[error("template error: {0}")]
    Template(String),

    /// An error from the underlying tensor / candle operation.
    #[error("tensor error: {0}")]
    Tensor(#[from] candle_core::Error),

    /// An I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, LfmError>;

impl From<minijinja::Error> for LfmError {
    fn from(e: minijinja::Error) -> Self {
        LfmError::Template(e.to_string())
    }
}
