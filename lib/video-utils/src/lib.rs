pub mod subtitle;

#[cfg(feature = "ffmpeg")]
pub mod subtitle_burn;

#[cfg(feature = "ffmpeg")]
pub mod audio_process;

#[cfg(feature = "ffmpeg")]
pub mod metadata;

#[cfg(feature = "ffmpeg")]
pub mod audio_extraction;

// #[cfg(feature = "ffmpeg")]
// pub mod video_frame;

#[cfg(feature = "ffmpeg")]
pub use subtitle_burn::{SubtitleBurnConfig, SubtitleStyle, add_subtitles, rgb_to_ass_color};

#[cfg(feature = "ffmpeg")]
pub use audio_process::{AudioProcessConfig, LoudnormConfig, process_audio};

#[cfg(feature = "ffmpeg")]
pub use metadata::{get_metadata, VideoMetadata};

#[cfg(feature = "ffmpeg")]
pub use audio_extraction::{extract_audio_interval, extract_all_audio, AudioSamples};

// #[cfg(feature = "ffmpeg")]
// pub use video_frame::{
//     extract_all_frames,
//     extract_frame_at_time,
//     extract_frames_interval,
//     save_frame_as_image,
//     VideoFrame,
// };

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),

    #[error("Parse Error {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg(feature = "ffmpeg")]
    #[error("FFmpeg Error: {0}")]
    FFmpeg(String),

    #[cfg(feature = "ffmpeg")]
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
