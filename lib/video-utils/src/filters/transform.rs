//! Video transformation filters (rotate, flip)
//!
//! This module provides geometric transformations for video frames.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Rotation angle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateAngle {
    /// 90 degrees clockwise
    Degrees90,
    /// 180 degrees
    Degrees180,
    /// 270 degrees clockwise (90 degrees counter-clockwise)
    Degrees270,
}

impl RotateAngle {
    /// Get rotation angle in degrees
    pub fn degrees(&self) -> u32 {
        match self {
            RotateAngle::Degrees90 => 90,
            RotateAngle::Degrees180 => 180,
            RotateAngle::Degrees270 => 270,
        }
    }
}

/// Flip direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlipDirection {
    /// Horizontal flip (mirror left-right)
    Horizontal,
    /// Vertical flip (mirror top-bottom)
    Vertical,
}

/// Configuration for video rotation
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct RotateConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Rotation angle
    #[derivative(Default(value = "RotateAngle::Degrees90"))]
    pub angle: RotateAngle,
}

impl RotateConfig {
    /// Create a new rotate config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, angle: RotateAngle) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_angle(angle)
    }
}

/// Configuration for video flip
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FlipConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Flip direction
    #[derivative(Default(value = "FlipDirection::Horizontal"))]
    pub direction: FlipDirection,
}

impl FlipConfig {
    /// Create a new flip config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, direction: FlipDirection) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_direction(direction)
    }
}

/// Rotate a video by specified angle
///
/// # Arguments
/// * `config` - Rotation configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::transform::{rotate_video, RotateConfig, RotateAngle};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RotateConfig::new("input.mp4", "output_90.mp4", RotateAngle::Degrees90);
/// rotate_video(config)?;
/// # Ok(())
/// # }
/// ```
pub fn rotate_video(config: RotateConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    log::info!("Rotating video: {} by {} degrees", config.input, config.angle.degrees());

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

    let src_width = decoder.width();
    let src_height = decoder.height();

    // Calculate output dimensions after rotation
    let (dst_width, dst_height) = match config.angle {
        RotateAngle::Degrees90 | RotateAngle::Degrees270 => (src_height, src_width),
        RotateAngle::Degrees180 => (src_width, src_height),
    };

    log::debug!("Source: {}x{}, Target: {}x{}", src_width, src_height, dst_width, dst_height);

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

    log::info!("Rotating {} frames...", frames.len());

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

    // Process and rotate frames
    for (idx, frame) in frames.iter().enumerate() {
        if idx % 30 == 0 {
            log::debug!("Rotating frame {}/{}", idx + 1, frames.len());
        }

        let rotated_data = rotate_frame_rgb24(&frame.data, frame.width, frame.height, config.angle);

        let frame_data = FrameData {
            width: dst_width,
            height: dst_height,
            data: rotated_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding rotated video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Rotation complete: {} -> {} ({}°)", config.input, config.output, config.angle.degrees());

    Ok(())
}

/// Flip a video
///
/// # Arguments
/// * `config` - Flip configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::transform::{flip_video, FlipConfig, FlipDirection};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Horizontal flip (mirror)
/// let config = FlipConfig::new("input.mp4", "output_h_flip.mp4", FlipDirection::Horizontal);
/// flip_video(config)?;
/// # Ok(())
/// # }
/// ```
pub fn flip_video(config: FlipConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    log::info!("Flipping video: {} ({:?})", config.input, config.direction);

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

    log::info!("Flipping {} frames...", frames.len());

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

    // Process and flip frames
    for (idx, frame) in frames.iter().enumerate() {
        if idx % 30 == 0 {
            log::debug!("Flipping frame {}/{}", idx + 1, frames.len());
        }

        let flipped_data = flip_frame_rgb24(&frame.data, frame.width, frame.height, config.direction);

        let frame_data = FrameData {
            width: frame.width,
            height: frame.height,
            data: flipped_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding flipped video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Flip complete: {} -> {} ({:?})", config.input, config.output, config.direction);

    Ok(())
}

/// Rotate RGB24 frame data
fn rotate_frame_rgb24(data: &[u8], width: u32, height: u32, angle: RotateAngle) -> Vec<u8> {
    let (new_width, new_height) = match angle {
        RotateAngle::Degrees90 | RotateAngle::Degrees270 => (height, width),
        RotateAngle::Degrees180 => (width, height),
    };

    let mut rotated = vec![0u8; (new_width * new_height * 3) as usize];

    match angle {
        RotateAngle::Degrees90 => {
            // Rotate 90° clockwise: (x,y) -> (y, width-1-x)
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * 3;
                    let dst_x = height - 1 - y;
                    let dst_y = x;
                    let dst_idx = (dst_y * new_width + dst_x) * 3;

                    rotated[dst_idx as usize] = data[src_idx as usize];
                    rotated[dst_idx as usize + 1] = data[src_idx as usize + 1];
                    rotated[dst_idx as usize + 2] = data[src_idx as usize + 2];
                }
            }
        }
        RotateAngle::Degrees180 => {
            // Rotate 180°: (x,y) -> (width-1-x, height-1-y)
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * 3;
                    let dst_x = width - 1 - x;
                    let dst_y = height - 1 - y;
                    let dst_idx = (dst_y * width + dst_x) * 3;

                    rotated[dst_idx as usize] = data[src_idx as usize];
                    rotated[dst_idx as usize + 1] = data[src_idx as usize + 1];
                    rotated[dst_idx as usize + 2] = data[src_idx as usize + 2];
                }
            }
        }
        RotateAngle::Degrees270 => {
            // Rotate 270° clockwise (90° counter-clockwise): (x,y) -> (height-1-y, x)
            for y in 0..height {
                for x in 0..width {
                    let src_idx = (y * width + x) * 3;
                    let dst_x = height - 1 - y;
                    let dst_y = x;
                    let dst_idx = (dst_y * new_width + dst_x) * 3;

                    rotated[dst_idx as usize] = data[src_idx as usize];
                    rotated[dst_idx as usize + 1] = data[src_idx as usize + 1];
                    rotated[dst_idx as usize + 2] = data[src_idx as usize + 2];
                }
            }
        }
    }

    rotated
}

/// Flip RGB24 frame data
fn flip_frame_rgb24(data: &[u8], width: u32, height: u32, direction: FlipDirection) -> Vec<u8> {
    let mut flipped = vec![0u8; data.len()];

    for y in 0..height {
        for x in 0..width {
            let (src_x, src_y) = match direction {
                FlipDirection::Horizontal => (width - 1 - x, y),
                FlipDirection::Vertical => (x, height - 1 - y),
            };

            let src_idx = (src_y * width + src_x) * 3;
            let dst_idx = (y * width + x) * 3;

            flipped[dst_idx as usize] = data[src_idx as usize];
            flipped[dst_idx as usize + 1] = data[src_idx as usize + 1];
            flipped[dst_idx as usize + 2] = data[src_idx as usize + 2];
        }
    }

    flipped
}

/// Convenience function to rotate video 90 degrees
pub fn rotate_90(input: &str, output: &str) -> Result<()> {
    let config = RotateConfig::new(input, output, RotateAngle::Degrees90);
    rotate_video(config)
}

/// Convenience function to rotate video 180 degrees
pub fn rotate_180(input: &str, output: &str) -> Result<()> {
    let config = RotateConfig::new(input, output, RotateAngle::Degrees180);
    rotate_video(config)
}

/// Convenience function to flip video horizontally
pub fn flip_horizontal(input: &str, output: &str) -> Result<()> {
    let config = FlipConfig::new(input, output, FlipDirection::Horizontal);
    flip_video(config)
}

/// Convenience function to flip video vertically
pub fn flip_vertical(input: &str, output: &str) -> Result<()> {
    let config = FlipConfig::new(input, output, FlipDirection::Vertical);
    flip_video(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_angle() {
        assert_eq!(RotateAngle::Degrees90.degrees(), 90);
        assert_eq!(RotateAngle::Degrees180.degrees(), 180);
        assert_eq!(RotateAngle::Degrees270.degrees(), 270);
    }

    #[test]
    fn test_rotate_90_frame() {
        // 2x3 frame, rotate 90°
        let data = vec![
            1, 2, 3,  4, 5, 6,  // (0,0)=1, (1,0)=2, (2,0)=3
            7, 8, 9, 10, 11, 12, // (0,1)=4, (1,1)=5, (2,1)=6
        ];

        let rotated = rotate_frame_rgb24(&data, 3, 2, RotateAngle::Degrees90);

        // Should become 3x2
        assert_eq!(rotated.len(), 3 * 2 * 3);

        // Top-left should be original bottom-left (7,8,9) after rotation
        assert_eq!(rotated[0], 7);
        assert_eq!(rotated[1], 8);
        assert_eq!(rotated[2], 9);
    }

    #[test]
    fn test_flip_horizontal() {
        // 3x2 frame
        let data = vec![
            1, 2, 3, 4, 5, 6,  // (0,0)=1, (2,0)=3
            7, 8, 9, 10, 11, 12, // (0,1)=4, (2,1)=6
        ];

        let flipped = flip_frame_rgb24(&data, 3, 2, FlipDirection::Horizontal);

        // Top-left (1,2,3) should become top-right (4,5,6)
        assert_eq!(flipped[0], 4);
        assert_eq!(flipped[1], 5);
        assert_eq!(flipped[2], 6);
    }

    #[test]
    fn test_flip_vertical() {
        // 3x2 frame
        let data = vec![
            1, 2, 3, 4, 5, 6,  // (0,0)=1, (1,1)=5
            7, 8, 9, 10, 11, 12, // (0,1)=4
        ];

        let flipped = flip_frame_rgb24(&data, 3, 2, FlipDirection::Vertical);

        // Top-left (1,2,3) should become bottom-left (7,8,9)
        assert_eq!(flipped[0], 7);
        assert_eq!(flipped[1], 8);
        assert_eq!(flipped[2], 9);
    }
}
