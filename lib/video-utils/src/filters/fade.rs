//! Video and audio fade in/out effects
//!
//! This module provides fade effects for smooth transitions.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Fade type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeType {
    /// Fade in from black/transparent
    In,
    /// Fade out to black/transparent
    Out,
}

/// Fade configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FadeConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Fade type
    #[derivative(Default(value = "FadeType::In"))]
    pub fade_type: FadeType,
    /// Fade duration in seconds
    #[derivative(Default(value = "0.0"))]
    pub duration: f64,
    /// Color to fade from/to (RGB)
    #[derivative(Default(value = "(0, 0, 0)"))]
    pub color: (u8, u8, u8),
}

impl FadeConfig {
    /// Create a new fade config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, fade_type: FadeType, duration: f64) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_fade_type(fade_type)
            .with_duration(duration)
    }

    /// Set fade color (convenience method for RGB tuple)
    pub fn with_color_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.color = (r, g, b);
        self
    }
}

/// Apply fade effect to video
///
/// This function applies a fade in or fade out effect to the video.
///
/// # Arguments
/// * `config` - Fade configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::fade::{fade_video, FadeConfig, FadeType};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Fade in from black (1 second)
/// let config = FadeConfig::new("input.mp4", "output.mp4", FadeType::In, 1.0);
/// fade_video(config)?;
/// # Ok(())
/// # }
/// ```
pub fn fade_video(config: FadeConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    if config.duration <= 0.0 {
        return Err(Error::InvalidConfig("Fade duration must be positive".to_string()));
    }

    log::info!("Applying {:?} fade (duration: {:.2}s)", config.fade_type, config.duration);

    // Open input
    let input_ctx = ffmpeg::format::input(&Path::new(&config.input))
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    let video_stream = input_ctx.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found".to_string()))?;

    // Get video parameters
    let frame_rate = video_stream.avg_frame_rate();
    let fps = frame_rate.numerator() as f32 / frame_rate.denominator() as f32;

    let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .map_err(|e| Error::FFmpeg(format!("Failed to get codec context: {}", e)))?;
    let decoder = codec_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

    let _width = decoder.width();
    let _height = decoder.height();

    // Get duration
    let duration_secs = input_ctx.duration() as f64 / 1_000_000.0;
    let frame_interval = std::time::Duration::from_secs_f64(1.0 / fps as f64);
    let fade_frames = (config.duration * fps as f64).ceil() as usize;
    let total_frames = (duration_secs * fps as f64).ceil() as usize;

    log::debug!("Fade frames: {} / {}", fade_frames, total_frames);

    // Extract all frames
    log::info!("Extracting frames from input...");
    let frames = extract_frames_interval(
        &config.input,
        std::time::Duration::ZERO,
        std::time::Duration::from_secs_f64(duration_secs),
        frame_interval,
    )?;

    log::info!("Processing {} frames with fade effect...", frames.len());

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

    let (encoder, video_tx, _audio_tx) = MP4Encoder::start(encoder_config)
        .map_err(|e| Error::FFmpeg(format!("Failed to start encoder: {}", e)))?;

    // Process frames with fade effect
    for (idx, frame) in frames.iter().enumerate() {
        let (should_fade, progress) = match config.fade_type {
            FadeType::In => (idx < fade_frames, idx as f32 / fade_frames as f32),
            FadeType::Out => {
                let start_fade = total_frames.saturating_sub(fade_frames);
                (idx >= start_fade, (idx - start_fade) as f32 / fade_frames as f32)
            }
        };

        let processed_data = if should_fade {
            apply_fade_to_frame(&frame.data, frame.width, frame.height, progress, config.fade_type, config.color)
        } else {
            frame.data.clone()
        };

        let frame_data = FrameData {
            width: frame.width,
            height: frame.height,
            data: processed_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;

        if (idx + 1) % 30 == 0 {
            log::debug!("Processed frame {}/{}", idx + 1, frames.len());
        }
    }

    log::info!("Encoding faded video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Fade effect applied: {} -> {}", config.input, config.output);

    Ok(())
}

/// Apply fade effect to a single frame
fn apply_fade_to_frame(
    data: &[u8],
    _width: u32,
    _height: u32,
    progress: f32,
    fade_type: FadeType,
    color: (u8, u8, u8),
) -> Vec<u8> {
    let mut faded = vec![0u8; data.len()];

    let alpha = match fade_type {
        FadeType::In => progress,       // 0 -> 1 (fade in)
        FadeType::Out => 1.0 - progress, // 1 -> 0 (fade out)
    };

    for (i, chunk) in data.chunks_exact(3).enumerate() {
        let r = chunk[0] as f32;
        let g = chunk[1] as f32;
        let b = chunk[2] as f32;

        let faded_r = (r + (color.0 as f32 - r) * (1.0 - alpha)) as u8;
        let faded_g = (g + (color.1 as f32 - g) * (1.0 - alpha)) as u8;
        let faded_b = (b + (color.2 as f32 - b) * (1.0 - alpha)) as u8;

        faded[i * 3] = faded_r;
        faded[i * 3 + 1] = faded_g;
        faded[i * 3 + 2] = faded_b;
    }

    faded
}

/// Convenience function for fade in
pub fn fade_in(input: &str, output: &str, duration_sec: f64) -> Result<()> {
    let config = FadeConfig::new(input, output, FadeType::In, duration_sec);
    fade_video(config)
}

/// Convenience function for fade out
pub fn fade_out(input: &str, output: &str, duration_sec: f64) -> Result<()> {
    let config = FadeConfig::new(input, output, FadeType::Out, duration_sec);
    fade_video(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_config() {
        let config = FadeConfig::new("input.mp4", "output.mp4", FadeType::In, 2.0);
        assert_eq!(config.input, "input.mp4");
        assert_eq!(config.output, "output.mp4");
        assert_eq!(config.fade_type, FadeType::In);
        assert_eq!(config.duration, 2.0);
    }

    #[test]
    fn test_fade_config_with_color() {
        let config = FadeConfig::new("input.mp4", "output.mp4", FadeType::Out, 1.5)
            .with_color(255, 255, 255); // White fade

        assert_eq!(config.color, (255, 255, 255));
    }
}
