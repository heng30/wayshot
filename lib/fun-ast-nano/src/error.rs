use thiserror::Error;

/// Main error type for Fun-ASR-Nano
#[derive(Error, Debug)]
pub enum FunAsrError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tensor error: {0}")]
    Tensor(#[from] candle_core::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Audio processing error: {0}")]
    Audio(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// Convert from audio_utils::AudioProcessError
impl From<audio_utils::AudioProcessError> for FunAsrError {
    fn from(err: audio_utils::AudioProcessError) -> Self {
        match err {
            audio_utils::AudioProcessError::Audio(msg) => FunAsrError::Audio(msg),
            audio_utils::AudioProcessError::Io(e) => FunAsrError::Io(e),
            audio_utils::AudioProcessError::Candle(e) => FunAsrError::Tensor(e),
        }
    }
}

// Convert from tensor_utils::TensorUtilsError
impl From<tensor_utils::TensorUtilsError> for FunAsrError {
    fn from(err: tensor_utils::TensorUtilsError) -> Self {
        match err {
            tensor_utils::TensorUtilsError::InvalidInput(msg) => FunAsrError::InvalidInput(msg),
            tensor_utils::TensorUtilsError::Candle(e) => FunAsrError::Tensor(e),
        }
    }
}

/// Result type alias
pub type Result<T> = std::result::Result<T, FunAsrError>;
