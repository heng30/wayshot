pub mod audio;
pub mod vad;

#[cfg(feature = "extraction")]
pub mod extract;

pub type Result<T> = std::result::Result<T, AudioProcessError>;

#[derive(thiserror::Error, Debug)]
pub enum AudioProcessError {
    #[error("Audio processing error: {0}")]
    Audio(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "extraction")]
    #[error("Candle error: {0}")]
    Candle(#[from] candle_core::Error),
}

#[cfg(feature = "extraction")]
impl From<tensor_utils::TensorUtilsError> for AudioProcessError {
    fn from(err: tensor_utils::TensorUtilsError) -> Self {
        match err {
            tensor_utils::TensorUtilsError::InvalidInput(msg) => AudioProcessError::Audio(msg),
            tensor_utils::TensorUtilsError::Candle(e) => AudioProcessError::Candle(e),
        }
    }
}
