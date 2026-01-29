//! Video speed change functionality
//!
//! Allows changing video playback speed (slow motion, fast forward, etc.).

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;
use std::time::Duration;

/// Speed change factor
pub type SpeedFactor = f64;

/// Configuration for video speed change
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SpeedConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Speed factor (>1 = faster, <1 = slower)
    #[derivative(Default(value = "1.0"))]
    pub speed: SpeedFactor,
    /// Whether to maintain audio pitch (requires audio processing)
    #[derivative(Default(value = "true"))]
    pub maintain_pitch: bool,
}

impl SpeedConfig {
    /// Create a new speed config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, speed: SpeedFactor) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_speed(speed)
    }

    /// Set pitch preservation (alias for maintain_pitch)
    pub fn with_pitch(mut self, maintain: bool) -> Self {
        self.maintain_pitch = maintain;
        self
    }
}

/// Change video playback speed
///
/// This function speeds up or slows down a video by adjusting frame timestamps.
///
/// # Arguments
/// * `config` - Speed configuration
///
/// # Example
/// ```no_run
/// use video_utils::editor::speed::{change_speed, SpeedConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Speed up to 2x
/// let config = SpeedConfig::new("input.mp4", "output_2x.mp4", 2.0);
/// change_speed(config)?;
///
/// // Slow down to 0.5x
/// let config = SpeedConfig::new("input.mp4", "output_05x.mp4", 0.5);
/// change_speed(config)?;
/// # Ok(())
/// # }
/// ```
pub fn change_speed(config: SpeedConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    if config.speed <= 0.0 {
        return Err(Error::InvalidConfig("Speed factor must be positive".to_string()));
    }

    log::info!("Changing video speed: {}x", config.speed);

    // Open input
    let input_ctx = ffmpeg::format::input(&Path::new(&config.input))
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    let video_stream = input_ctx.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found".to_string()))?;

    // Get video parameters
    let frame_rate = video_stream.avg_frame_rate();
    let original_fps = frame_rate.numerator() as f32 / frame_rate.denominator() as f32;

    let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .map_err(|e| Error::FFmpeg(format!("Failed to get codec context: {}", e)))?;
    let decoder = codec_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

    let _width = decoder.width();
    let _height = decoder.height();

    // Get duration
    let original_duration = input_ctx.duration() as f64 / 1_000_000.0;
    let new_duration = original_duration / config.speed;

    log::debug!("Original: {:.2}s @ {:.2} fps", original_duration, original_fps);
    log::debug!("New: {:.2}s @ {:.2} fps", new_duration, original_fps * config.speed as f32);

    // Calculate frame interval for extraction (smaller interval for slow motion to avoid frame duplication)
    let extract_interval = Duration::from_secs_f64(1.0 / original_fps as f64);

    // Extract all frames
    log::info!("Extracting frames from input...");
    let frames = extract_frames_interval(
        &config.input,
        Duration::ZERO,
        Duration::from_secs_f64(original_duration),
        extract_interval,
    )?;

    log::info!("Processing {} frames with {}x speed...", frames.len(), config.speed);

    // Setup encoder with adjusted frame rate
    let new_fps = original_fps * config.speed as f32;

    let encoder_config = MP4EncoderConfig {
        output_path: std::path::PathBuf::from(&config.output),
        frame_rate: new_fps as u32,
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

    let (encoder, video_tx, _audio_tx) = MP4Encoder::start(encoder_config)
        .map_err(|e| Error::FFmpeg(format!("Failed to start encoder: {}", e)))?;

    // Process frames with adjusted timestamps
    let frame_interval = Duration::from_secs_f64(1.0 / new_fps as f64);
    let mut current_timestamp = Duration::ZERO;

    // For slow motion, we may need to duplicate frames
    // For fast forward, we may need to skip frames
    let frame_step = if config.speed >= 1.0 {
        config.speed.round() as usize
    } else {
        1
    };

    let mut frames_sent = 0;

    for (idx, frame) in frames.iter().enumerate() {
        // Skip frames for speed up (>1x)
        if config.speed > 1.0 && idx % frame_step != 0 {
            continue;
        }

        // Duplicate frames for slow down (<1x)
        let duplicates = if config.speed < 1.0 {
            (1.0 / config.speed).round() as usize
        } else {
            1
        };

        for _ in 0..duplicates {
            let frame_data = FrameData {
                width: frame.width,
                height: frame.height,
                data: frame.data.clone(),
                timestamp: current_timestamp,
            };

            video_tx.send(frame_data)
                .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;

            current_timestamp += frame_interval;
            frames_sent += 1;
        }

        if (idx + 1) % 30 == 0 {
            log::debug!("Processed {}/{} frames", idx + 1, frames.len());
        }
    }

    log::info!("Encoding {} frames (original: {}, speed: {}x)", frames_sent, frames.len(), config.speed);

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Speed change complete: {} -> {} ({}x)", config.input, config.output, config.speed);

    Ok(())
}

/// Convenience function to speed up video
pub fn speed_up(input: &str, output: &str, factor: SpeedFactor) -> Result<()> {
    if factor < 1.0 {
        return Err(Error::InvalidConfig("Speed up factor must be >= 1.0".to_string()));
    }
    let config = SpeedConfig::new(input, output, factor);
    change_speed(config)
}

/// Convenience function to slow down video
pub fn slow_down(input: &str, output: &str, factor: SpeedFactor) -> Result<()> {
    if factor > 1.0 {
        return Err(Error::InvalidConfig("Slow down factor must be <= 1.0".to_string()));
    }
    let config = SpeedConfig::new(input, output, factor);
    change_speed(config)
}

/// Convenience function for reverse playback
pub fn reverse_video(_input: &str, _output: &str) -> Result<()> {
    // Reverse is just 1x speed with reversed frame order
    // Implementation would require extracting frames and sending them in reverse order
    Err(Error::FFmpeg("reverse_video not yet implemented - requires frame reordering".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_config() {
        let config = SpeedConfig::new("input.mp4", "output.mp4", 2.0);
        assert_eq!(config.speed, 2.0);
        assert!(config.maintain_pitch);
    }

    #[test]
    fn test_speed_config_with_pitch() {
        let config = SpeedConfig::new("input.mp4", "output.mp4", 0.5)
            .with_pitch(false);
        assert_eq!(config.speed, 0.5);
        assert!(!config.maintain_pitch);
    }
}
