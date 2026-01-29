//! Video cropping functionality
//!
//! Allows extracting rectangular regions from video frames.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;

/// Crop mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropMode {
    /// Crop from center
    Center,
    /// Crop from top-left corner
    TopLeft,
    /// Custom crop position
    Custom { x: u32, y: u32 },
}

/// Configuration for video cropping
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct CropConfig {
    /// Input video file
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Crop mode
    #[derivative(Default(value = "CropMode::Center"))]
    pub mode: CropMode,
    /// Crop width
    #[derivative(Default(value = "0"))]
    pub width: u32,
    /// Crop height
    #[derivative(Default(value = "0"))]
    pub height: u32,
}

impl CropConfig {
    /// Create a new crop config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, width: u32, height: u32) -> Self {
        Self::center(input, output, width, height)
    }

    /// Create a centered crop config
    pub fn center(input: impl Into<String>, output: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            mode: CropMode::Center,
            width,
            height,
        }
    }
}

/// Crop a video to specified dimensions
///
/// This function extracts a rectangular region from each video frame.
///
/// # Arguments
/// * `config` - Crop configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::crop::{crop_video, CropConfig, CropMode};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Crop center 640x360
/// let config = CropConfig::new("input.mp4", "output.mp4", 640, 360)
///     .with_mode(CropMode::Center);
///
/// crop_video(config)?;
/// # Ok(())
/// # }
/// ```
pub fn crop_video(config: CropConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;
    use std::path::Path;

    log::info!("Cropping video: {} -> {} ({}x{})", config.input, config.output, config.width, config.height);

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

    // Calculate crop position
    let (crop_x, crop_y) = match config.mode {
        CropMode::Center => {
            if config.width > src_width || config.height > src_height {
                return Err(Error::InvalidConfig(format!(
                    "Crop dimensions {}x{} exceed source {}x{}",
                    config.width, config.height, src_width, src_height
                )));
            }
            ((src_width - config.width) / 2, (src_height - config.height) / 2)
        }
        CropMode::TopLeft => (0, 0),
        CropMode::Custom { x, y } => {
            if x + config.width > src_width || y + config.height > src_height {
                return Err(Error::InvalidConfig(format!(
                    "Crop region ({}, {}) + ({}, {}) exceeds source {}x{}",
                    x, y, config.width, config.height, src_width, src_height
                )));
            }
            (x, y)
        }
    };

    log::debug!("Source: {}x{}, Crop: {}x{} at ({}, {})", src_width, src_height, config.width, config.height, crop_x, crop_y);

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

    log::info!("Cropping {} frames...", frames.len());

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

    // Process and crop frames
    for (idx, frame) in frames.iter().enumerate() {
        if idx % 30 == 0 {
            log::debug!("Cropping frame {}/{}", idx + 1, frames.len());
        }

        let cropped_data = crop_frame_rgb24(
            &frame.data,
            frame.width,
            frame.height,
            crop_x,
            crop_y,
            config.width,
            config.height,
        );

        let frame_data = FrameData {
            width: config.width,
            height: config.height,
            data: cropped_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding cropped video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Cropping complete: {} -> {}", config.input, config.output);

    Ok(())
}

/// Crop RGB24 frame data to extract rectangular region
fn crop_frame_rgb24(
    data: &[u8],
    src_width: u32,
    _src_height: u32,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
) -> Vec<u8> {
    let mut cropped = vec![0u8; (crop_width * crop_height * 3) as usize];

    for y in 0..crop_height {
        for x in 0..crop_width {
            let src_x = crop_x + x;
            let src_y = crop_y + y;

            let src_idx = (src_y * src_width + src_x) * 3;
            let dst_idx = (y * crop_width + x) * 3;

            if src_idx as usize + 2 < data.len() && dst_idx as usize + 2 < cropped.len() {
                cropped[dst_idx as usize] = data[src_idx as usize];
                cropped[dst_idx as usize + 1] = data[src_idx as usize + 1];
                cropped[dst_idx as usize + 2] = data[src_idx as usize + 2];
            }
        }
    }

    cropped
}

/// Convenience function to crop to center
pub fn crop_center(input: &str, output: &str, width: u32, height: u32) -> Result<()> {
    let config = CropConfig::new(input, output, width, height)
        .with_mode(CropMode::Center);
    crop_video(config)
}

/// Convenience function to crop to specific aspect ratio
pub fn crop_to_aspect(input: &str, output: &str, aspect_width: u32, aspect_height: u32) -> Result<()> {
    use ffmpeg_next as ffmpeg;
    use std::path::Path;

    // Get source dimensions
    let input_ctx = ffmpeg::format::input(&Path::new(input))
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    let video_stream = input_ctx.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found".to_string()))?;

    let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .map_err(|e| Error::FFmpeg(format!("Failed to get codec context: {}", e)))?;
    let decoder = codec_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

    let src_width = decoder.width();
    let src_height = decoder.height();

    // Calculate crop dimensions for target aspect ratio
    let (crop_width, crop_height) = calculate_crop_for_aspect(src_width, src_height, aspect_width, aspect_height);

    let config = CropConfig::new(input, output, crop_width, crop_height)
        .with_mode(CropMode::Center);

    crop_video(config)
}

/// Calculate crop dimensions to achieve target aspect ratio
fn calculate_crop_for_aspect(src_width: u32, src_height: u32, aspect_width: u32, aspect_height: u32) -> (u32, u32) {
    let target_aspect = aspect_width as f32 / aspect_height as f32;
    let src_aspect = src_width as f32 / src_height as f32;

    if src_aspect > target_aspect {
        // Source is wider - crop width
        let new_width = (src_height as f32 * target_aspect) as u32;
        (new_width, src_height)
    } else {
        // Source is taller - crop height
        let new_height = (src_width as f32 / target_aspect) as u32;
        (src_width, new_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_config() {
        let config = CropConfig::new("input.mp4", "output.mp4", 640, 360);
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 360);
        assert!(matches!(config.mode, CropMode::Center));
    }

    #[test]
    fn test_crop_aspect_calculation() {
        // 16:9 source, crop to 4:3
        let (w, h) = calculate_crop_for_aspect(1920, 1080, 4, 3);
        assert_eq!(w, 1440); // 1080 * 4/3 = 1440
        assert_eq!(h, 1080);

        // 4:3 source, crop to 16:9
        let (w, h) = calculate_crop_for_aspect(1280, 960, 16, 9);
        assert_eq!(w, 1280);
        assert_eq!(h, 720); // 1280 * 9/16 = 720
    }

    #[test]
    fn test_crop_frame() {
        let src = vec![
            255, 0, 0,  // (0,0) red
            0, 255, 0,  // (1,0) green
            0, 0, 255,  // (2,0) blue
            255, 255, 0, // (0,1) yellow
            0, 255, 255, // (1,1) cyan
            255, 0, 255, // (2,1) magenta
        ]; // 3x2 RGB

        // Crop to 2x1 (top-left corner)
        let cropped = crop_frame_rgb24(&src, 3, 2, 0, 0, 2, 1);

        assert_eq!(cropped.len(), 2 * 1 * 3);
        // First pixel should be red
        assert_eq!(cropped[0], 255);
        assert_eq!(cropped[1], 0);
        assert_eq!(cropped[2], 0);
        // Second pixel should be green
        assert_eq!(cropped[3], 0);
        assert_eq!(cropped[4], 255);
        assert_eq!(cropped[5], 0);
    }
}
