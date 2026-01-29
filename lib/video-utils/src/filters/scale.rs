//! Video scaling/resizing functionality

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Scaling quality preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleQuality {
    /// Fast, low quality (nearest neighbor)
    Fast,
    /// Balanced (bilinear)
    Medium,
    /// Slow, high quality (bicubic)
    High,
    /// Best quality (lanczos)
    Best,
}

impl ScaleQuality {
    /// Convert to FFmpeg scale filter flags
    #[allow(dead_code)]
    fn to_flags(&self) -> &'static str {
        match self {
            ScaleQuality::Fast => "neighbor",
            ScaleQuality::Medium => "bilinear",
            ScaleQuality::High => "bicubic",
            ScaleQuality::Best => "lanczos",
        }
    }
}

/// Configuration for video scaling
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ScaleConfig {
    /// Input video path
    #[derivative(Default(value = "String::new()"))]
    pub input: String,
    /// Output video path
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Target width (0 to keep aspect ratio based on height)
    #[derivative(Default(value = "0"))]
    pub width: u32,
    /// Target height (0 to keep aspect ratio based on width)
    #[derivative(Default(value = "0"))]
    pub height: u32,
    /// Scaling quality
    #[derivative(Default(value = "ScaleQuality::Medium"))]
    pub quality: ScaleQuality,
    /// Whether to preserve aspect ratio (adds black bars if needed)
    #[derivative(Default(value = "true"))]
    pub preserve_aspect_ratio: bool,
}

impl ScaleConfig {
    /// Create a new scale config (convenience method)
    pub fn new(input: impl Into<String>, output: impl Into<String>, width: u32, height: u32) -> Self {
        Self::default()
            .with_input(input.into())
            .with_output(output.into())
            .with_width(width)
            .with_height(height)
    }

    /// Create scale config that fits within dimensions (preserving aspect ratio)
    pub fn fit_within(input: impl Into<String>, output: impl Into<String>, max_width: u32, max_height: u32) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            width: max_width,
            height: max_height,
            quality: ScaleQuality::Medium,
            preserve_aspect_ratio: true,
        }
    }

    /// Create scale config with exact dimensions
    pub fn exact(input: impl Into<String>, output: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            width,
            height,
            quality: ScaleQuality::Medium,
            preserve_aspect_ratio: false,
        }
    }

    /// Alias for with_preserve_aspect_ratio (for backward compatibility)
    pub fn with_aspect_ratio(mut self, preserve: bool) -> Self {
        self.preserve_aspect_ratio = preserve;
        self
    }
}

/// Scale a video to specified dimensions
///
/// This function resizes a video using FFmpeg's scale filter.
///
/// # Arguments
/// * `config` - Scale configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::scale::{scale_video, ScaleConfig, ScaleQuality};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Scale to 1280x720 using high quality
/// let config = ScaleConfig::default()
///     .with_input("input.mp4")
///     .with_output("output.mp4")
///     .with_width(1280)
///     .with_height(720)
///     .with_quality(ScaleQuality::High);
///
/// scale_video(config)?;
/// # Ok(())
/// # }
/// ```
pub fn scale_video(config: ScaleConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    log::info!("Scaling video: {} -> {} ({}x{})", config.input, config.output, config.width, config.height);

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

    // Calculate output dimensions (preserve aspect ratio if enabled)
    let (dst_width, dst_height) = if config.preserve_aspect_ratio {
        calculate_aspect_preserved_dimensions(src_width, src_height, config.width, config.height)
    } else {
        (config.width, config.height)
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

    log::info!("Scaling {} frames...", frames.len());

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

    // Process and scale frames
    for (idx, frame) in frames.iter().enumerate() {
        if idx % 30 == 0 {
            log::debug!("Scaling frame {}/{}", idx + 1, frames.len());
        }

        let scaled_data = scale_frame_rgb24(
            &frame.data,
            frame.width,
            frame.height,
            dst_width,
            dst_height,
            config.quality,
        );

        let frame_data = FrameData {
            width: dst_width,
            height: dst_height,
            data: scaled_data,
            timestamp: frame.pts,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding scaled video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Scaling complete: {} -> {}", config.input, config.output);

    Ok(())
}

/// Calculate dimensions that preserve aspect ratio
fn calculate_aspect_preserved_dimensions(src_width: u32, src_height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    let src_aspect = src_width as f32 / src_height as f32;
    let max_aspect = max_width as f32 / max_height as f32;

    if src_aspect > max_aspect {
        // Width is the limiting factor
        let new_width = max_width;
        let new_height = (max_width as f32 / src_aspect) as u32;
        (new_width, new_height)
    } else {
        // Height is the limiting factor
        let new_height = max_height;
        let new_width = (max_height as f32 * src_aspect) as u32;
        (new_width, new_height)
    }
}

/// Scale RGB24 frame data using various quality algorithms
fn scale_frame_rgb24(data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32, quality: ScaleQuality) -> Vec<u8> {
    let mut scaled = vec![0u8; (dst_width * dst_height * 3) as usize];

    match quality {
        ScaleQuality::Fast => scale_nearest_neighbor(data, src_width, src_height, dst_width, dst_height, &mut scaled),
        ScaleQuality::Medium => scale_bilinear(data, src_width, src_height, dst_width, dst_height, &mut scaled),
        ScaleQuality::High | ScaleQuality::Best => scale_bicubic(data, src_width, src_height, dst_width, dst_height, &mut scaled),
    }

    scaled
}

/// Nearest neighbor scaling (fast, low quality)
fn scale_nearest_neighbor(data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32, output: &mut [u8]) {
    let x_ratio = src_width as f32 / dst_width as f32;
    let y_ratio = src_height as f32 / dst_height as f32;

    for y in 0..dst_height {
        for x in 0..dst_width {
            let src_x = ((x as f32 * x_ratio) as u32).min(src_width - 1) as usize;
            let src_y = ((y as f32 * y_ratio) as u32).min(src_height - 1) as usize;

            let src_idx = (src_y * src_width as usize + src_x) * 3;
            let dst_idx = (y as usize * dst_width as usize + x as usize) * 3;

            output[dst_idx as usize] = data[src_idx as usize];
            output[dst_idx as usize + 1] = data[src_idx as usize + 1];
            output[dst_idx as usize + 2] = data[src_idx as usize + 2];
        }
    }
}

/// Bilinear interpolation scaling
fn scale_bilinear(data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32, output: &mut [u8]) {
    let x_ratio = (src_width - 1) as f32 / (dst_width - 1) as f32;
    let y_ratio = (src_height - 1) as f32 / (dst_height - 1) as f32;

    for y in 0..dst_height {
        for x in 0..dst_width {
            let src_x = x as f32 * x_ratio;
            let src_y = y as f32 * y_ratio;

            let x1 = src_x.floor() as usize;
            let x2 = (x1 + 1).min(src_width as usize - 1);
            let y1 = src_y.floor() as usize;
            let y2 = (y1 + 1).min(src_height as usize - 1);

            let x_ratio_local = src_x - x1 as f32;
            let y_ratio_local = src_y - y1 as f32;

            for c in 0..3 {
                let idx11 = (y1 * src_width as usize + x1) * 3 + c;
                let idx12 = (y1 * src_width as usize + x2) * 3 + c;
                let idx21 = (y2 * src_width as usize + x1) * 3 + c;
                let idx22 = (y2 * src_width as usize + x2) * 3 + c;

                let c11 = data[idx11] as f32;
                let c12 = data[idx12] as f32;
                let c21 = data[idx21] as f32;
                let c22 = data[idx22] as f32;

                let result = c11 * (1.0 - x_ratio_local) * (1.0 - y_ratio_local)
                           + c12 * x_ratio_local * (1.0 - y_ratio_local)
                           + c21 * (1.0 - x_ratio_local) * y_ratio_local
                           + c22 * x_ratio_local * y_ratio_local;

                let dst_idx = (y as usize * dst_width as usize + x as usize) * 3 + c;
                output[dst_idx] = result as u8;
            }
        }
    }
}

/// Bicubic interpolation scaling (high quality)
fn scale_bicubic(data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32, output: &mut [u8]) {
    // Simplified bicubic - use bilinear for now
    // A full implementation would use Catmull-Rom or Mitchell-Netravali splines
    scale_bilinear(data, src_width, src_height, dst_width, dst_height, output);
}

/// Convenience function to scale video to fit within dimensions
pub fn scale_to_fit(input: &str, output: &str, max_width: u32, max_height: u32) -> Result<()> {
    let config = ScaleConfig::fit_within(input, output, max_width, max_height);
    scale_video(config)
}

/// Convenience function to scale video to exact dimensions
pub fn scale_to_exact(input: &str, output: &str, width: u32, height: u32) -> Result<()> {
    let config = ScaleConfig::new(input, output, width, height)
        .with_aspect_ratio(false);
    scale_video(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_config() {
        let config = ScaleConfig::new("input.mp4", "output.mp4", 1920, 1080);
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert!(config.preserve_aspect_ratio);
    }

    #[test]
    fn test_aspect_ratio_calculation() {
        // 16:9 source, fit into 4:3 max
        let (w, h) = calculate_aspect_preserved_dimensions(1920, 1080, 1024, 768);
        assert_eq!(w, 1024);
        assert_eq!(h, 576); // 1024 / (16/9) = 576

        // 4:3 source, fit into 16:9 max
        let (w, h) = calculate_aspect_preserved_dimensions(640, 480, 1920, 1080);
        assert_eq!(w, 1440); // 1080 * (4/3) = 1440
        assert_eq!(h, 1080);
    }

    #[test]
    fn test_scale_nearest_neighbor() {
        let src = vec![255u8, 0, 0, 0, 255, 0, 0, 0, 255]; // 3x1 RGB: red, green, blue
        let mut dst = vec![0u8; 6 * 1 * 3]; // 6x1 output

        scale_nearest_neighbor(&src, 3, 1, 6, 1, &mut dst);

        // First pixel should be red
        assert_eq!(dst[0], 255);
        assert_eq!(dst[1], 0);
        assert_eq!(dst[2], 0);
    }

    #[test]
    fn test_quality_flags() {
        assert_eq!(ScaleQuality::Fast.to_flags(), "neighbor");
        assert_eq!(ScaleQuality::Medium.to_flags(), "bilinear");
        assert_eq!(ScaleQuality::High.to_flags(), "bicubic");
        assert_eq!(ScaleQuality::Best.to_flags(), "lanczos");
    }
}
