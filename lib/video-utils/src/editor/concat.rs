//! Video concatenation functionality
//!
//! Allows joining multiple video files end-to-end into a single output.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Configuration for video concatenation
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ConcatConfig {
    /// List of input video files (in order)
    #[derivative(Default(value = "Vec::new()"))]
    pub inputs: Vec<String>,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Target width (0 to use first video's width)
    #[derivative(Default(value = "0"))]
    pub target_width: u32,
    /// Target height (0 to use first video's height)
    #[derivative(Default(value = "0"))]
    pub target_height: u32,
    /// Whether to scale all inputs to match target resolution
    #[derivative(Default(value = "false"))]
    pub normalize_resolution: bool,
    /// Video bitrate (bps)
    #[derivative(Default(value = "None"))]
    pub video_bitrate: Option<usize>,
    /// Audio bitrate (bps)
    #[derivative(Default(value = "None"))]
    pub audio_bitrate: Option<usize>,
}

impl ConcatConfig {
    /// Create a new concat config (convenience method)
    pub fn new(inputs: Vec<String>, output: impl Into<String>) -> Self {
        Self::default()
            .with_inputs(inputs)
            .with_output(output.into())
    }

    /// Create a concat config from list of inputs
    pub fn from_list(inputs: Vec<String>, output: impl Into<String>) -> Self {
        Self {
            inputs,
            output: output.into(),
            ..Default::default()
        }
    }

    /// Set target resolution and enable normalization
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.target_width = width;
        self.target_height = height;
        self.normalize_resolution = true;
        self
    }
}

/// Concatenate multiple videos end-to-end
///
/// This function joins multiple video files into a single output video.
/// Videos are processed sequentially and their frames are encoded into the output.
///
/// # Arguments
/// * `config` - Concatenation configuration
///
/// # Example
/// ```no_run
/// use video_utils::editor::concat::{concat_videos, ConcatConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ConcatConfig::new(
///     vec!["clip1.mp4".to_string(), "clip2.mp4".to_string(), "clip3.mp4".to_string()],
///     "output.mp4".to_string(),
/// );
///
/// concat_videos(config)?;
/// # Ok(())
/// # }
/// ```
pub fn concat_videos(config: ConcatConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    if config.inputs.is_empty() {
        return Err(Error::InvalidConfig("No input files provided".to_string()));
    }

    log::info!("Concatenating {} video files into {}", config.inputs.len(), config.output);

    // Validate all input files exist
    for input in &config.inputs {
        if !Path::new(input).exists() {
            return Err(Error::IO(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Input file not found: {}", input),
            )));
        }
    }

    // Get metadata from first video to determine output parameters
    let first_input = ffmpeg::format::input(&Path::new(&config.inputs[0]))
        .map_err(|e| Error::FFmpeg(format!("Failed to open first input: {}", e)))?;

    let video_stream = first_input.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found in first input".to_string()))?;

    let frame_rate = video_stream.avg_frame_rate();
    let fps = frame_rate.numerator() as f32 / frame_rate.denominator() as f32;

    // Determine output resolution
    let (output_width, output_height) = if config.normalize_resolution && config.target_width > 0 && config.target_height > 0 {
        (config.target_width, config.target_height)
    } else {
        // Use first video's resolution
        let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
            .map_err(|e| Error::FFmpeg(format!("Failed to get codec context: {}", e)))?;
        let decoder = codec_context.decoder().video()
            .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;
        (decoder.width(), decoder.height())
    };

    // Setup encoder
    let encoder_config = MP4EncoderConfig {
        output_path: std::path::PathBuf::from(&config.output),
        frame_rate: fps as u32,
        h264: H264Config {
            bitrate: config.video_bitrate.unwrap_or(2_000_000) as u32,
            preset: H264Preset::Medium,
            crf: Some(23),
        },
        aac: AACConfig {
            bitrate: config.audio_bitrate.unwrap_or(128_000) as u32,
            sample_rate: 48000,
            channels: 2,
        },
    };

    let (encoder, video_tx, _audio_tx) = MP4Encoder::start(encoder_config)
        .map_err(|e| Error::FFmpeg(format!("Failed to start encoder: {}", e)))?;

    let mut frame_timestamp = std::time::Duration::ZERO;
    let mut total_frames = 0;

    // Process each input video
    for (idx, input_path) in config.inputs.iter().enumerate() {
        log::info!("Processing video {}/{}: {}", idx + 1, config.inputs.len(), input_path);

        // Get duration of this input
        let input_ctx = ffmpeg::format::input(&Path::new(input_path))
            .map_err(|e| Error::FFmpeg(format!("Failed to open input {}: {}", input_path, e)))?;

        let duration_secs = input_ctx.duration() as f64 / 1_000_000.0;
        let _video_stream_idx = input_ctx.streams().best(ffmpeg::media::Type::Video)
            .map(|s| s.index())
            .ok_or_else(|| Error::FFmpeg(format!("No video stream in {}", input_path)))?;

        // Calculate frame interval
        let frame_interval = std::time::Duration::from_secs_f64(1.0 / fps as f64);

        // Extract all frames from this video
        let frames = extract_frames_interval(
            input_path,
            std::time::Duration::ZERO,
            std::time::Duration::from_secs_f64(duration_secs),
            frame_interval,
        )?;

        let frame_count = frames.len();
        log::debug!("Extracted {} frames from {}", frame_count, input_path);

        // Send frames to encoder with adjusted timestamps
        for frame in frames {
            let mut frame_data = FrameData {
                width: frame.width,
                height: frame.height,
                data: frame.data,
                timestamp: frame_timestamp,
            };

            // Scale if needed
            if config.normalize_resolution && (frame.width != output_width || frame.height != output_height) {
                frame_data.data = scale_frame_rgb(&frame_data.data, frame.width, frame.height, output_width, output_height);
                frame_data.width = output_width;
                frame_data.height = output_height;
            }

            video_tx.send(frame_data)
                .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;

            frame_timestamp += frame_interval;
            total_frames += 1;
        }

        log::debug!("Processed {} frames, total so far: {}", frame_count, total_frames);
    }

    log::info!("Concatenation complete: {} total frames", total_frames);

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    Ok(())
}

/// Convenience function to concatenate videos with default settings
///
/// # Arguments
/// * `inputs` - List of input video paths
/// * `output` - Output video path
pub fn concat_videos_simple(inputs: Vec<String>, output: impl Into<String>) -> Result<()> {
    let config = ConcatConfig::new(inputs, output.into());
    concat_videos(config)
}

/// Scale RGB frame data to new dimensions
/// This is a simple bilinear interpolation implementation
fn scale_frame_rgb(data: &[u8], src_width: u32, src_height: u32, dst_width: u32, dst_height: u32) -> Vec<u8> {
    let mut scaled = vec![0u8; (dst_width * dst_height * 3) as usize];

    let x_ratio = src_width as f32 / dst_width as f32;
    let y_ratio = src_height as f32 / dst_height as f32;

    for y in 0..dst_height {
        for x in 0..dst_width {
            let src_x = (x as f32 * x_ratio) as u32;
            let src_y = (y as f32 * y_ratio) as u32;

            let src_idx = (src_y * src_width + src_x) * 3;
            let dst_idx = (y * dst_width + x) * 3;

            if src_idx as usize + 2 < data.len() && dst_idx as usize + 2 < scaled.len() {
                scaled[dst_idx as usize] = data[src_idx as usize];
                scaled[dst_idx as usize + 1] = data[src_idx as usize + 1];
                scaled[dst_idx as usize + 2] = data[src_idx as usize + 2];
            }
        }
    }

    scaled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concat_config_creation() {
        let config = ConcatConfig::new(
            vec!["a.mp4".to_string(), "b.mp4".to_string()],
            "out.mp4".to_string(),
        );

        assert_eq!(config.inputs.len(), 2);
        assert_eq!(config.output, "out.mp4");
        assert!(!config.normalize_resolution);
    }

    #[test]
    fn test_concat_config_with_resolution() {
        let config = ConcatConfig::new(
            vec!["a.mp4".to_string()],
            "out.mp4".to_string(),
        )
        .with_resolution(1920, 1080);

        assert_eq!(config.target_width, 1920);
        assert_eq!(config.target_height, 1080);
        assert!(config.normalize_resolution);
    }

    #[test]
    fn test_scale_frame_rgb() {
        let src_data = vec![255u8, 0, 0, 0, 255, 0]; // 2x1 RGB: red, green
        let scaled = scale_frame_rgb(&src_data, 2, 1, 4, 1);

        assert_eq!(scaled.len(), 4 * 1 * 3);
        // First pixel should be red
        assert_eq!(scaled[0], 255);
        assert_eq!(scaled[1], 0);
        assert_eq!(scaled[2], 0);
    }
}
