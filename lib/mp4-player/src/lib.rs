use thiserror::Error;

pub mod metadata;
pub mod player;
pub mod video_decoder;

pub use player::{Config, DecodedVideoFrame, Mp4Player, VideoFrame};

pub type Result<T> = std::result::Result<T, MP4PlayerError>;

#[derive(Error, Debug)]
pub enum MP4PlayerError {
    #[error("Failed to open MP4 file: {0}")]
    FileOpenError(#[from] std::io::Error),

    #[error("MP4 parsing error: {0}")]
    ParseError(#[from] mp4::Error),

    #[error("Channel communication error: {0}")]
    ChannelError(String),

    #[error("Tracker error: {0}")]
    TrackError(String),

    #[error("Frame parsing error: {0}")]
    FrameError(String),

    #[error("Mp4 Player stop error: {0}")]
    PlayerStopError(String),
}
