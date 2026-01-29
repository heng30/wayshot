//! Video trimming/cutting functionality
//!
//! Allows extracting specific time ranges from videos or removing segments.

use crate::{Result, Error};
use std::path::Path;
use std::time::Duration;
use ffmpeg_next as ffmpeg;
use derivative::Derivative;
use derive_setters::Setters;

/// Configuration for video trimming operation
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TrimConfig {
    /// Path to input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Path to output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Start time of the segment to extract
    #[derivative(Default(value = "Duration::ZERO"))]
    pub start: Duration,
    /// Duration of the segment to extract (None means until end of video)
    #[derivative(Default(value = "None"))]
    pub duration: Option<Duration>,
}

impl TrimConfig {
    /// Create a new trim config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, start: Duration) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_start(start)
    }

    /// Set the end time (calculates duration from start)
    pub fn with_end(self, end: Duration) -> Self {
        let duration = if end > self.start {
            Some(end - self.start)
        } else {
            None
        };
        self.with_duration(Some(duration.unwrap_or(Duration::ZERO)))
    }

    /// Set the duration value (wraps in Option)
    pub fn with_duration_value(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Set duration from value (convenience alias for with_duration_value)
    pub fn with_duration_unwrap(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Trim a video to specified time range
///
/// This function extracts a segment from the input video and saves it to the output file.
/// The video is re-encoded during this process.
///
/// # Arguments
/// * `config` - Trim configuration
///
/// # Example
/// ```no_run
/// use std::time::Duration;
/// use video_utils::editor::trim::{trim_video, TrimConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Extract from 10 seconds to 30 seconds
/// let config = TrimConfig::new(
///     "input.mp4",
///     "output.mp4",
///     Duration::from_secs(10),
/// )
/// .with_end(Duration::from_secs(30));
///
/// trim_video(config)?;
/// # Ok(())
/// # }
/// ```
pub fn trim_video(config: TrimConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};

    // Open input to get metadata
    let input = ffmpeg::format::input(&Path::new(&config.input))
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    // Get video stream info
    let video_stream = input.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found".to_string()))?;

    let audio_stream = input.streams().best(ffmpeg::media::Type::Audio);

    // Get frame rate
    let frame_rate = video_stream.avg_frame_rate();
    let fps = frame_rate.numerator() as f32 / frame_rate.denominator() as f32;

    // Calculate duration
    let duration = config.duration.unwrap_or_else(|| {
        let input_duration = input.duration();
        Duration::from_millis((input_duration - (config.start.as_millis() as i64).max(0)) as u64)
    });

    // Calculate number of frames
    let frame_interval = Duration::from_secs_f64(1.0 / fps as f64);
    let _num_frames = (duration.as_secs_f64() / frame_interval.as_secs_f64()).ceil() as usize;

    // Get video dimensions
    let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .map_err(|e| Error::FFmpeg(format!("Failed to get codec context: {}", e)))?;
    let decoder = codec_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

    let _width = decoder.width();
    let _height = decoder.height();

    // Setup encoder
    let encoder_config = MP4EncoderConfig {
        output_path: std::path::PathBuf::from(&config.output),
        frame_rate: fps as u32,
        h264: H264Config {
            bitrate: 2_000_000,
            preset: H264Preset::Medium,
            crf: Some(23),
        },
        aac: AACConfig {
            bitrate: 128_000,
            sample_rate: 48000,
            channels: 2,
        },
    };

    let (encoder, video_tx, audio_tx) = MP4Encoder::start(encoder_config)
        .map_err(|e| Error::FFmpeg(format!("Failed to start encoder: {}", e)))?;

    // Extract and send frames
    let frames = extract_frames_interval(
        &config.input,
        config.start,
        config.start + duration,
        frame_interval,
    )?;

    for frame in frames {
        let frame_data = FrameData {
            width: frame.width,
            height: frame.height,
            data: frame.data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    // Extract and send audio if present
    // TODO: Implement audio extraction with actual sample data
    // The current extract_audio_interval only returns metadata, not the actual samples
    // We need to either:
    // 1. Extend AudioSamples to include the actual sample data
    // 2. Or create a new function that extracts and returns raw audio samples
    if audio_stream.is_some() {
        log::warn!("Audio extraction in trim_video not yet implemented - output will be video-only");
    }

    // Drop senders and stop encoder
    drop(video_tx);
    drop(audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    Ok(())
}

/// Extract a segment from video (convenience function)
///
/// # Arguments
/// * `input` - Input video path
/// * `output` - Output video path
/// * `start_sec` - Start time in seconds
/// * `duration_sec` - Duration in seconds
pub fn extract_segment(input: impl AsRef<str>, output: impl AsRef<str>, start_sec: f64, duration_sec: f64) -> Result<()> {
    let config = TrimConfig::new(
        input.as_ref(),
        output.as_ref(),
        Duration::from_secs_f64(start_sec),
    )
    .with_duration(Some(Duration::from_secs_f64(duration_sec)));

    trim_video(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_trim_config_creation() {
        let config = TrimConfig::new("input.mp4", "output.mp4", Duration::from_secs(10));
        assert_eq!(config.input, "input.mp4");
        assert_eq!(config.output, "output.mp4");
        assert_eq!(config.start, Duration::from_secs(10));
        assert!(config.duration.is_none());
    }

    #[test]
    fn test_trim_config_with_duration() {
        let config = TrimConfig::new("input.mp4", "output.mp4", Duration::from_secs(10))
            .with_duration(Duration::from_secs(20));

        assert_eq!(config.duration, Some(Duration::from_secs(20)));
    }

    #[test]
    fn test_trim_config_with_end() {
        let config = TrimConfig::new("input.mp4", "output.mp4", Duration::from_secs(10))
            .with_end(Duration::from_secs(40));

        assert_eq!(config.duration, Some(Duration::from_secs(30)));
    }
}
