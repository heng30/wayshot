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

// 视频编辑操作
#[cfg(feature = "ffmpeg")]
pub mod editor;

// 视频滤镜
#[cfg(feature = "ffmpeg")]
pub mod filters;

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

// 编辑操作导出
#[cfg(feature = "ffmpeg")]
pub use editor::{
    trim_video, TrimConfig, extract_segment,
    concat_videos, ConcatConfig, concat_videos_simple,
    split_video, SplitConfig, split_equal, split_by_duration, split_at_points,
    change_speed, SpeedConfig, speed_up, slow_down, reverse_video, SpeedFactor,
};

// 滤镜导出
#[cfg(feature = "ffmpeg")]
pub use filters::{
    scale_video, ScaleConfig, ScaleQuality,
    scale_to_fit, scale_to_exact,
    rotate_video, flip_video, RotateAngle, FlipDirection,
    fade_video, FadeConfig, FadeType, fade_in, fade_out,
    crop_video, CropConfig, CropMode, crop_center, crop_to_aspect,
    adjust_color, ColorAdjustConfig, adjust_brightness, adjust_contrast, adjust_saturation,
    crossfade_videos, CrossfadeConfig,
    text_overlay, TextOverlayConfig, TextPosition, TextAlignment, add_watermark, add_title,
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
