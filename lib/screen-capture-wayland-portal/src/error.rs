use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Portal request failed: {0}")]
    PortalRequest(String),

    #[error("Screenshot failed: {0}")]
    Screenshot(String),

    #[error("ScreenCast failed: {0}")]
    ScreenCast(String),

    #[error("PipeWire error: {0}")]
    PipeWire(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(#[from] image::ImageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Async task error: {0}")]
    Async(#[from] tokio::task::JoinError),

    #[error("No available screens")]
    NoScreens,

    #[error("Screen not found: {0}")]
    ScreenNotFound(String),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Other: {0}")]
    Other(String),
}

