use thiserror::Error;

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by `voxcpm-rs`.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Safetensors error: {0}")]
    Safetensors(#[from] safetensors::SafeTensorError),

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Audio decode error: {0}")]
    AudioDecode(String),

    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),

    #[error("Resampler error: {0}")]
    Resampler(String),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Missing weight tensor: {0}")]
    MissingWeight(String),

    #[error("Shape mismatch for `{name}`: expected {expected:?}, got {actual:?}")]
    ShapeMismatch {
        name: String,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Unsupported: {0}")]
    Unsupported(String),

    /// Generation was cancelled via [`crate::CancelToken`].
    #[error("generation cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl From<tokenizers::Error> for Error {
    fn from(e: tokenizers::Error) -> Self {
        Error::Tokenizer(e.to_string())
    }
}
