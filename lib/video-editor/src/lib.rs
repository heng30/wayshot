pub mod commands;
pub mod export;
pub mod filters;
pub mod font;
pub mod media;
pub mod metadata;
pub mod preview;
pub mod project;
pub mod tracks;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),

    #[error("Parse Error {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("Video util Error {0}")]
    VideoUtil(#[from] video_utils::Error),

    #[error("Track segment Error {0}")]
    TrackSegment(String),

    #[error("Out of timestamp  Error {0}")]
    OutOfTimestamp(String),

    #[error("FFmpeg Error: {0}")]
    FFmpeg(String),

    #[error("FFmpeg Error: {0}")]
    FFmpegError(#[from] ffmpeg_next::Error),

    #[error("Image Error: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid file: {0}")]
    InvalidFile(String),

    #[error("Index out of bounds: index {0} >= length {1}")]
    IndexOutOfBounds(usize, usize),

    #[error("Cannot undo: {0}")]
    CannotUndo(String),

    #[error("Cannot redo: {0}")]
    CannotRedo(String),

    // Serialization errors
    #[error("Unsupported project version: {file_version}, current version: {current_version}")]
    UnsupportedProjectVersion {
        file_version: u32,
        current_version: u32,
    },

    #[error("Unknown filter type: '{filter_type}'")]
    UnknownFilter { filter_type: String },

    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),

    #[error("Invalid codec name: '{0}'")]
    InvalidCodecName(String),

    #[error("Invalid pixel format: '{0}'")]
    InvalidPixelFormat(String),

    #[error("Invalid sample format: '{0}'")]
    InvalidSampleFormat(String),

    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Export cancelled")]
    ExportCancelled,

    #[error("Sender error: '{0}")]
    Sender(String),

    #[error("Duplicate entry: '{0}'")]
    DuplicateEntry(String),
}

#[macro_export]
macro_rules! ensure_file_exists {
    ($path:expr) => {
        if !$path.exists() {
            return Err($crate::Error::IO(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", $path.display()),
            )));
        }
    };
}
