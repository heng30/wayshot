//! Error types for similar-video-segment.

use std::path::PathBuf;

/// Result type for similar-video-segment operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during video scanning and segment export.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Image loading error: {0}")]
    ImageLoad(String),

    #[error("Image processing error: {0}")]
    ImageProcess(String),

    #[error("FFmpeg error: {0}")]
    FFmpeg(String),

    #[error("CNN embedding error: {0}")]
    Embedding(String),

    #[error("Video not found: {0}")]
    VideoNotFound(PathBuf),

    #[error("No video stream in file: {0}")]
    NoVideoStream(PathBuf),

    #[error("MP4 export error: {0}")]
    Mp4Export(String),

    #[error("Video encoding error: {0}")]
    VideoEncode(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("Channel send error: {0}")]
    ChannelSend(String),
}
