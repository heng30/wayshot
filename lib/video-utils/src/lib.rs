pub mod common_type;
pub mod convert;
pub mod subtitle;

#[cfg(feature = "ffmpeg")]
pub mod ffmpeg;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),

    #[error("Parse Error {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("Srt parse Error {0}")]
    SrtParse(String),

    #[error("Image Buffer Error {0}")]
    ImageBuffer(#[from] fast_image_resize::ImageBufferError),

    #[error("Image Resize Error {0}")]
    ImageResize(#[from] fast_image_resize::ResizeError),

    #[error("Image From Error: invalid dimensions (expected: {expected:?}, got: {actual:?})")]
    ImageFrom {
        expected: (u32, u32),
        actual: (u32, u32),
    },

    #[cfg(feature = "ffmpeg")]
    #[error("FFmpeg Error {0}")]
    FFmpeg(String),
}
