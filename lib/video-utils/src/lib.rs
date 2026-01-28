pub mod subtitle;

#[cfg(feature = "ffmpeg")]
pub mod subtitle_burn;

#[cfg(feature = "ffmpeg")]
pub mod audio_process;

#[cfg(feature = "ffmpeg")]
pub mod metadata;

#[cfg(feature = "ffmpeg")]
pub mod audio_extraction;

#[cfg(feature = "ffmpeg")]
pub mod video_frame;

// MP4 封装器
#[cfg(feature = "ffmpeg")]
pub mod mp4_muxer;

// MP4 编码器
#[cfg(feature = "ffmpeg")]
pub mod mp4_encoder;

#[cfg(feature = "ffmpeg")]
pub use subtitle_burn::{SubtitleBurnConfig, SubtitleStyle, add_subtitles, rgb_to_ass_color};

#[cfg(feature = "ffmpeg")]
pub use audio_process::{AudioProcessConfig, LoudnormConfig, process_audio};

#[cfg(feature = "ffmpeg")]
pub use metadata::{get_metadata, VideoMetadata};

#[cfg(feature = "ffmpeg")]
pub use audio_extraction::{extract_audio_interval, extract_all_audio, AudioSamples};

#[cfg(feature = "ffmpeg")]
pub use video_frame::{
    extract_all_frames,
    extract_frame_at_time,
    extract_frames_interval,
    save_frame_as_image,
    VideoFrame,
};

// MP4 封装器导出
#[cfg(feature = "ffmpeg")]
pub use mp4_muxer::{MP4Muxer, MP4MuxerConfig, AACConfig as MuxerAACConfig, FrameData as MuxerFrameData, AudioData as MuxerAudioData};

// MP4 编码器导出
#[cfg(feature = "ffmpeg")]
pub use mp4_encoder::{
    MP4Encoder, MP4EncoderConfig, H264Config, AACConfig as EncoderAACConfig, H264Preset,
    FrameData as EncoderFrameData, AudioData as EncoderAudioData,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),

    #[error("Parse Error {0}")]
    Parse(#[from] chrono::ParseError),

    #[cfg(feature = "ffmpeg")]
    #[error("FFmpeg Error: {0}")]
    FFmpeg(String),

    #[cfg(feature = "ffmpeg")]
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
