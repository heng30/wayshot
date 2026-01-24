pub mod subtitle;

#[cfg(feature = "add-subtitle")]
pub mod subtitle_burn;

#[cfg(feature = "add-subtitle")]
pub use subtitle_burn::{
    rgb_to_ass_color, add_subtitles, SubtitleBurnConfig, SubtitleStyle,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),
    #[error("Parse Error {0}")]
    Parse(#[from] chrono::ParseError),
    #[cfg(feature = "add-subtitle")]
    #[error("FFmpeg Error: {0}")]
    FFmpeg(String),
    #[cfg(feature = "add-subtitle")]
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
