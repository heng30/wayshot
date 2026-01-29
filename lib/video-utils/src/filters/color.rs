//! Video color adjustment filters
//!
//! This module provides color manipulation for video frames.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Color adjustment configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ColorAdjustConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Brightness adjustment (-100 to +100, 0 = no change)
    #[derivative(Default(value = "0"))]
    pub brightness: i32,
    /// Contrast adjustment (-100 to +100, 0 = no change)
    #[derivative(Default(value = "0"))]
    pub contrast: i32,
    /// Saturation adjustment (-100 to +100, 0 = grayscale, >0 = more saturated)
    #[derivative(Default(value = "0"))]
    pub saturation: i32,
}

impl ColorAdjustConfig {
    /// Create a new color adjust config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
    }

    /// Set brightness with clamping
    pub fn with_brightness_clamped(mut self, brightness: i32) -> Self {
        self.brightness = brightness.clamp(-100, 100);
        self
    }

    /// Set contrast with clamping
    pub fn with_contrast_clamped(mut self, contrast: i32) -> Self {
        self.contrast = contrast.clamp(-100, 100);
        self
    }

    /// Set saturation with clamping
    pub fn with_saturation_clamped(mut self, saturation: i32) -> Self {
        self.saturation = saturation.clamp(-100, 100);
        self
    }
}

/// Adjust video colors (brightness, contrast, saturation)
///
/// # Arguments
/// * `config` - Color adjustment configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::color::{adjust_color, ColorAdjustConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ColorAdjustConfig::new("input.mp4", "output.mp4")
///     .with_brightness(20)
///     .with_contrast(10)
///     .with_saturation(30);
/// adjust_color(config)?;
/// # Ok(())
/// # }
/// ```
pub fn adjust_color(config: ColorAdjustConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    log::info!("Adjusting video colors: {} (brightness={}, contrast={}, saturation={})",
        config.input, config.brightness, config.contrast, config.saturation);

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

    // Extract all frames
    log::info!("Extracting frames from input...");
    let frames = extract_frames_interval(
        &config.input,
        std::time::Duration::ZERO,
        std::time::Duration::from_secs_f64(duration_secs),
        frame_interval,
    )?;

    log::info!("Applying color adjustments to {} frames...", frames.len());

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

    // Process and adjust colors for each frame
    for (idx, frame) in frames.iter().enumerate() {
        if idx % 30 == 0 {
            log::debug!("Adjusting colors for frame {}/{}", idx + 1, frames.len());
        }

        let adjusted_data = adjust_colors_rgb24(
            &frame.data,
            frame.width,
            frame.height,
            config.brightness,
            config.contrast,
            config.saturation,
        );

        let frame_data = FrameData {
            width: frame.width,
            height: frame.height,
            data: adjusted_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding adjusted video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Color adjustment complete: {} -> {}", config.input, config.output);

    Ok(())
}

/// Adjust colors for RGB24 frame data
fn adjust_colors_rgb24(
    data: &[u8],
    width: u32,
    height: u32,
    brightness: i32,
    contrast: i32,
    saturation: i32,
) -> Vec<u8> {
    let mut adjusted = vec![0u8; data.len()];

    // Calculate adjustment factors
    let brightness_factor = brightness as f32 / 100.0; // -1.0 to 1.0
    let contrast_factor = if contrast >= 0 {
        1.0 + (contrast as f32 / 100.0) // 1.0 to 2.0
    } else {
        1.0 + (contrast as f32 / 100.0) // 0.0 to 1.0
    };
    let saturation_factor = 1.0 + (saturation as f32 / 100.0); // 0.0 to 2.0

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;

            let mut r = data[idx as usize] as f32;
            let mut g = data[idx as usize + 1] as f32;
            let mut b = data[idx as usize + 2] as f32;

            // Apply brightness
            r += brightness_factor * 255.0;
            g += brightness_factor * 255.0;
            b += brightness_factor * 255.0;

            // Apply contrast
            r = ((r - 128.0) * contrast_factor) + 128.0;
            g = ((g - 128.0) * contrast_factor) + 128.0;
            b = ((b - 128.0) * contrast_factor) + 128.0;

            // Apply saturation
            if saturation_factor != 1.0 {
                // Convert to grayscale
                let gray = 0.299 * r + 0.587 * g + 0.114 * b;

                // Interpolate between color and grayscale
                r = gray + (r - gray) * saturation_factor;
                g = gray + (g - gray) * saturation_factor;
                b = gray + (b - gray) * saturation_factor;
            }

            // Clamp to 0-255 and convert back to u8
            adjusted[idx as usize] = r.clamp(0.0, 255.0) as u8;
            adjusted[idx as usize + 1] = g.clamp(0.0, 255.0) as u8;
            adjusted[idx as usize + 2] = b.clamp(0.0, 255.0) as u8;
        }
    }

    adjusted
}

/// Convenience function to adjust only brightness
pub fn adjust_brightness(input: &str, output: &str, brightness: i32) -> Result<()> {
    let config = ColorAdjustConfig::new(input, output)
        .with_brightness(brightness);
    adjust_color(config)
}

/// Convenience function to adjust only contrast
pub fn adjust_contrast(input: &str, output: &str, contrast: i32) -> Result<()> {
    let config = ColorAdjustConfig::new(input, output)
        .with_contrast(contrast);
    adjust_color(config)
}

/// Convenience function to adjust only saturation
pub fn adjust_saturation(input: &str, output: &str, saturation: i32) -> Result<()> {
    let config = ColorAdjustConfig::new(input, output)
        .with_saturation(saturation);
    adjust_color(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_adjust_config() {
        let config = ColorAdjustConfig::new("in.mp4", "out.mp4")
            .with_brightness(50)
            .with_contrast(-30)
            .with_saturation(20);

        assert_eq!(config.brightness, 50);
        assert_eq!(config.contrast, -30);
        assert_eq!(config.saturation, 20);
    }

    #[test]
    fn test_clamping() {
        let config = ColorAdjustConfig::new("in.mp4", "out.mp4")
            .with_brightness(150) // Should clamp to 100
            .with_contrast(-200); // Should clamp to -100

        assert_eq!(config.brightness, 100);
        assert_eq!(config.contrast, -100);
    }

    #[test]
    fn test_brightness_increase() {
        let data = vec![100, 100, 100, 150, 150, 150];
        let adjusted = adjust_colors_rgb24(&data, 2, 1, 50, 0, 0);

        // With +50% brightness, 100 should become ~178
        assert!(adjusted[0] > 170 && adjusted[0] < 185);
        assert!(adjusted[1] > 170 && adjusted[1] < 185);
        assert!(adjusted[2] > 170 && adjusted[2] < 185);
    }

    #[test]
    fn test_grayscale() {
        let data = vec![255, 0, 0, 0, 255, 0]; // Red and Green pixels
        let adjusted = adjust_colors_rgb24(&data, 2, 1, 0, 0, -100);

        // With -100 saturation, should be grayscale
        // Red (255, 0, 0) -> gray (~76)
        // Green (0, 255, 0) -> gray (~150)
        assert!((adjusted[0] as i32 - adjusted[1] as i32).abs() < 5);
        assert!((adjusted[0] as i32 - adjusted[2] as i32).abs() < 5);
    }
}
