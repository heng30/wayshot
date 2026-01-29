//! Text overlay functionality for videos
//!
//! This module provides text burning/watermark functionality for video frames.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    /// Align text to left
    Left,
    /// Align text to center
    Center,
    /// Align text to right
    Right,
}

/// Text position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPosition {
    /// Custom position (x, y coordinates from top-left)
    Custom { x: u32, y: u32 },
    /// Top-left corner
    TopLeft,
    /// Top-center
    TopCenter,
    /// Top-right corner
    TopRight,
    /// Center-left
    CenterLeft,
    /// Center of screen
    Center,
    /// Center-right
    CenterRight,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom-center
    BottomCenter,
    /// Bottom-right corner
    BottomRight,
}

impl TextPosition {
    /// Calculate actual (x, y) coordinates based on video dimensions
    fn calculate_coords(&self, video_width: u32, video_height: u32, text_width: u32, text_height: u32) -> (u32, u32) {
        match self {
            TextPosition::Custom { x, y } => (*x, *y),
            TextPosition::TopLeft => (10, 10),
            TextPosition::TopCenter => ((video_width - text_width) / 2, 10),
            TextPosition::TopRight => (video_width - text_width - 10, 10),
            TextPosition::CenterLeft => (10, (video_height - text_height) / 2),
            TextPosition::Center => ((video_width - text_width) / 2, (video_height - text_height) / 2),
            TextPosition::CenterRight => (video_width - text_width - 10, (video_height - text_height) / 2),
            TextPosition::BottomLeft => (10, video_height - text_height - 10),
            TextPosition::BottomCenter => ((video_width - text_width) / 2, video_height - text_height - 10),
            TextPosition::BottomRight => (video_width - text_width - 10, video_height - text_height - 10),
        }
    }
}

/// Configuration for text overlay
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TextOverlayConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Text content to overlay
    #[derivative(Default(value = "String::new()"))]
    pub text: String,
    /// Font size in pixels
    #[derivative(Default(value = "24"))]
    pub font_size: u32,
    /// Text color (RGB)
    #[derivative(Default(value = "(255, 255, 255)"))]
    pub color: (u8, u8, u8),
    /// Background color (RGB), set to None for transparent
    #[derivative(Default(value = "Some((0, 0, 0))"))]
    pub background_color: Option<(u8, u8, u8)>,
    /// Text position
    #[derivative(Default(value = "TextPosition::BottomRight"))]
    pub position: TextPosition,
    /// Text alignment
    #[derivative(Default(value = "TextAlignment::Left"))]
    pub alignment: TextAlignment,
    /// Padding around text in pixels
    #[derivative(Default(value = "5"))]
    pub padding: u32,
}

impl TextOverlayConfig {
    /// Create a new text overlay config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, text: impl Into<String>) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_text(text.into())
    }

    /// Set text color (convenience method for RGB tuple)
    pub fn with_color_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.color = (r, g, b);
        self
    }

    /// Set background color (convenience method for RGB tuple)
    pub fn with_background_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.background_color = Some((r, g, b));
        self
    }

    /// Set transparent background
    pub fn with_transparent_background(mut self) -> Self {
        self.background_color = None;
        self
    }
}

/// Apply text overlay to video
///
/// This function burns text into each video frame.
///
/// # Arguments
/// * `config` - Text overlay configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::text_overlay::{text_overlay, TextOverlayConfig, TextPosition};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Add watermark to bottom-right
/// let config = TextOverlayConfig::new("input.mp4", "output.mp4", "Watermark")
///     .with_position(TextPosition::BottomRight)
///     .with_font_size(32)
///     .with_color_rgb(255, 255, 255);
///
/// text_overlay(config)?;
/// # Ok(())
/// # }
/// ```
pub fn text_overlay(config: TextOverlayConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    if config.text.is_empty() {
        return Err(Error::InvalidConfig("Text content cannot be empty".to_string()));
    }

    log::info!("Applying text overlay: \"{}\"", config.text);

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

    let width = decoder.width();
    let height = decoder.height();

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

    log::info!("Processing {} frames with text overlay...", frames.len());

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

    // Calculate text dimensions
    let (text_width, text_height) = estimate_text_size(&config.text, config.font_size);

    // Calculate position
    let (text_x, text_y) = config.position.calculate_coords(
        width,
        height,
        text_width + config.padding * 2,
        text_height + config.padding * 2,
    );

    log::debug!("Text position: ({}, {}), size: {}x{}", text_x, text_y, text_width, text_height);

    // Process frames with text overlay
    for (idx, frame) in frames.iter().enumerate() {
        if idx % 30 == 0 {
            log::debug!("Processing frame {}/{}", idx + 1, frames.len());
        }

        let overlaid_data = draw_text_on_frame(
            &frame.data,
            frame.width,
            frame.height,
            &config.text,
            config.font_size,
            config.color,
            config.background_color,
            text_x,
            text_y,
            config.padding,
        );

        let frame_data = FrameData {
            width: frame.width,
            height: frame.height,
            data: overlaid_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding video with text overlay...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Text overlay complete: {} -> {}", config.input, config.output);

    Ok(())
}

/// Estimate text dimensions based on font size and character count
fn estimate_text_size(text: &str, font_size: u32) -> (u32, u32) {
    // Approximate: each character is ~0.6x font_size wide
    let char_count = text.chars().count() as u32;
    let width = (char_count as f32 * font_size as f32 * 0.6) as u32;
    let height = font_size;
    (width.max(1), height.max(1))
}

/// Draw text on RGB24 frame
fn draw_text_on_frame(
    data: &[u8],
    width: u32,
    height: u32,
    text: &str,
    font_size: u32,
    text_color: (u8, u8, u8),
    background_color: Option<(u8, u8, u8)>,
    x: u32,
    y: u32,
    padding: u32,
) -> Vec<u8> {
    let mut result = data.to_vec();

    // Calculate text bounding box
    let (text_width, text_height) = estimate_text_size(text, font_size);
    let box_x = x.saturating_sub(padding);
    let box_y = y.saturating_sub(padding);
    let box_width = text_width + padding * 2;
    let box_height = text_height + padding * 2;

    // Draw background if specified
    if let Some(bg) = background_color {
        for by in box_y..(box_y + box_height).min(height) {
            for bx in box_x..(box_x + box_width).min(width) {
                let idx = (by * width + bx) * 3;
                if idx as usize + 2 < result.len() {
                    result[idx as usize] = bg.0;
                    result[idx as usize + 1] = bg.1;
                    result[idx as usize + 2] = bg.2;
                }
            }
        }
    }

    // Draw text (simplified - each character as a block)
    let char_width = (font_size as f32 * 0.6) as u32;
    let mut char_x = x;

    for c in text.chars() {
        // Draw character as a simple block (very basic rendering)
        for cy in y..(y + font_size).min(height) {
            for cx in char_x..(char_x + char_width).min(width) {
                // Only draw pixels for non-space characters
                if c != ' ' {
                    let idx = (cy * width + cx) * 3;
                    if idx as usize + 2 < result.len() {
                        // Simple pattern: draw on odd pixel positions for visibility
                        if (cx + cy) % 2 == 0 {
                            result[idx as usize] = text_color.0;
                            result[idx as usize + 1] = text_color.1;
                            result[idx as usize + 2] = text_color.2;
                        }
                    }
                }
            }
        }
        char_x += char_width;
    }

    result
}

/// Convenience function to add watermark
pub fn add_watermark(input: &str, output: &str, text: &str) -> Result<()> {
    let config = TextOverlayConfig::new(input, output, text)
        .with_position(TextPosition::BottomRight)
        .with_font_size(32)
        .with_color_rgb(255, 255, 255)
        .with_transparent_background();
    text_overlay(config)
}

/// Convenience function to add title
pub fn add_title(input: &str, output: &str, title: &str) -> Result<()> {
    let config = TextOverlayConfig::new(input, output, title)
        .with_position(TextPosition::TopCenter)
        .with_font_size(48)
        .with_color_rgb(255, 255, 255)
        .with_background_rgb(0, 0, 0);
    text_overlay(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_position_calculation() {
        let pos = TextPosition::TopLeft;
        assert_eq!(pos.calculate_coords(1920, 1080, 100, 50), (10, 10));

        let pos = TextPosition::Center;
        assert_eq!(pos.calculate_coords(1920, 1080, 100, 50), (910, 515));

        let pos = TextPosition::BottomRight;
        assert_eq!(pos.calculate_coords(1920, 1080, 100, 50), (1810, 1020));
    }

    #[test]
    fn test_estimate_text_size() {
        let (w, h) = estimate_text_size("Test", 24);
        assert_eq!(h, 24);
        assert!(w > 0 && w < 100);

        let (w, h) = estimate_text_size("Longer text", 32);
        assert_eq!(h, 32);
        assert!(w > 50);
    }

    #[test]
    fn test_text_overlay_config() {
        let config = TextOverlayConfig::new("input.mp4", "output.mp4", "Watermark");
        assert_eq!(config.text, "Watermark");
        assert_eq!(config.font_size, 24);

        let config = config
            .with_font_size(48)
            .with_color_rgb(255, 0, 0)
            .with_background_rgb(0, 0, 0);

        assert_eq!(config.font_size, 48);
        assert_eq!(config.color, (255, 0, 0));
        assert_eq!(config.background_color, Some((0, 0, 0)));
    }
}
