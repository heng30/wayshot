use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Stream not found: {0}")]
    NoStream(String),

    #[error("Screen cast error.")]
    ScreencastError(#[from] ashpd::Error),

    #[error("PipeWire error: {0}")]
    PipeWire(#[from] pipewire::Error),

    #[error("Screen info error: {0}")]
    ScreenInfoError(String),

    #[error("cursor error: {0}")]
    CursorError(String),

    #[error("No output error: {0}")]
    NoOutput(String),

    #[error("Unimplemented: {0}")]
    Unimplemented(String),

    #[error("Other error: {0}")]
    Other(String),
}
