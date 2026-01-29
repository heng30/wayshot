//! Video crossfade transition filter
//!
//! This module provides crossfade transitions between two videos.

use crate::{Result, Error};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::Path;

/// Crossfade transition configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct CrossfadeConfig {
    /// First video file (will play first)
    #[derivative(Default(value = "String::new()"))]
    pub video1: String,
    /// Second video file (will play second)
    #[derivative(Default(value = "String::new()"))]
    pub video2: String,
    /// Output video file
    #[derivative(Default(value = "String::new()"))]
    pub output: String,
    /// Overlap duration in seconds (portion where both videos are blended)
    #[derivative(Default(value = "0.0"))]
    pub overlap_duration: f64,
}

/// Apply crossfade transition between two videos
///
/// The first video plays, then crossfades to the second video during the overlap period.
///
/// # Arguments
/// * `config` - Crossfade configuration
///
/// # Example
/// ```no_run
/// use video_utils::filters::crossfade::{crossfade_videos, CrossfadeConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Crossfade with 2 second overlap
/// let config = CrossfadeConfig::new("video1.mp4", "video2.mp4", "output.mp4", 2.0);
/// crossfade_videos(config)?;
/// # Ok(())
/// # }
/// ```
pub fn crossfade_videos(config: CrossfadeConfig) -> Result<()> {
    use crate::video_frame::extract_frames_interval;
    use crate::mp4_encoder::{MP4Encoder, MP4EncoderConfig, H264Config, H264Preset, AACConfig, FrameData};
    use ffmpeg_next as ffmpeg;

    log::info!("Creating crossfade: {} -> {} (overlap: {:.2}s)",
        config.video1, config.video2, config.overlap_duration);

    // Open both videos
    let input1_ctx = ffmpeg::format::input(&Path::new(&config.video1))
        .map_err(|e| Error::FFmpeg(format!("Failed to open video1: {}", e)))?;

    let input2_ctx = ffmpeg::format::input(&Path::new(&config.video2))
        .map_err(|e| Error::FFmpeg(format!("Failed to open video2: {}", e)))?;

    let video1_stream = input1_ctx.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found in video1".to_string()))?;

    let _video2_stream = input2_ctx.streams().best(ffmpeg::media::Type::Video)
        .ok_or_else(|| Error::FFmpeg("No video stream found in video2".to_string()))?;

    // Get video parameters (use video1 as reference)
    let frame_rate = video1_stream.avg_frame_rate();
    let fps = frame_rate.numerator() as f32 / frame_rate.denominator() as f32;

    let codec1_context = ffmpeg::codec::context::Context::from_parameters(video1_stream.parameters())
        .map_err(|e| Error::FFmpeg(format!("Failed to get codec context: {}", e)))?;
    let decoder1 = codec1_context.decoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to create decoder: {}", e)))?;

    let width = decoder1.width();
    let height = decoder1.height();

    // Get durations
    let duration1_secs = input1_ctx.duration() as f64 / 1_000_000.0;
    let duration2_secs = input2_ctx.duration() as f64 / 1_000_000.0;

    // Calculate overlap in frames
    let overlap_frames = (config.overlap_duration * fps as f64).round() as u32;
    let frame_interval = std::time::Duration::from_secs_f64(1.0 / fps as f64);

    // Extract frames from both videos
    log::info!("Extracting frames from video1...");
    let frames1 = extract_frames_interval(
        &config.video1,
        std::time::Duration::ZERO,
        std::time::Duration::from_secs_f64(duration1_secs),
        frame_interval,
    )?;

    log::info!("Extracting frames from video2...");
    let frames2 = extract_frames_interval(
        &config.video2,
        std::time::Duration::ZERO,
        std::time::Duration::from_secs_f64(duration2_secs),
        frame_interval,
    )?;

    let total_frames1 = frames1.len();
    let total_frames2 = frames2.len();

    // Validate overlap
    if overlap_frames as usize > total_frames1 || overlap_frames as usize > total_frames2 {
        return Err(Error::InvalidConfig(format!(
            "Overlap duration ({:.2}s = {} frames) exceeds video length (video1: {} frames, video2: {} frames)",
            config.overlap_duration, overlap_frames, total_frames1, total_frames2
        )));
    }

    log::info!("Processing crossfade: {} frames from video1, {} from video2, {} overlap frames",
        total_frames1, total_frames2, overlap_frames);

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

    // Calculate output frame count
    let output_frame_count = total_frames1 + total_frames2 - overlap_frames as usize;

    // Process frames with crossfade
    for out_frame_idx in 0..output_frame_count {
        if out_frame_idx % 30 == 0 {
            log::debug!("Processing frame {}/{}", out_frame_idx + 1, output_frame_count);
        }

        let (frame_data, timestamp) = if out_frame_idx < total_frames1 - overlap_frames as usize {
            // Before overlap: only video1
            let frame = &frames1[out_frame_idx];
            (frame.data.clone(), frame.pts)
        } else if out_frame_idx >= total_frames1 {
            // After overlap: only video2
            let frame_idx = out_frame_idx - total_frames1;
            let frame = &frames2[frame_idx];
            (frame.data.clone(), frame.pts)
        } else {
            // During overlap: blend video1 and video2
            let frame1_idx = out_frame_idx;
            let frame2_idx = out_frame_idx - (total_frames1 - overlap_frames as usize);

            let frame1 = &frames1[frame1_idx];
            let frame2 = &frames2[frame2_idx];

            // Calculate blend factor (0.0 = all video1, 1.0 = all video2)
            let overlap_position = out_frame_idx - (total_frames1 - overlap_frames as usize);
            let alpha = overlap_position as f32 / overlap_frames as f32;

            let blended = blend_frames_rgb24(
                &frame1.data,
                &frame2.data,
                width,
                height,
                alpha,
            );

            (blended, frame1.pts)
        };

        let frame_data = FrameData {
            width,
            height,
            data: frame_data,
            timestamp,
        };

        video_tx.send(frame_data)
            .map_err(|e| Error::FFmpeg(format!("Failed to send video frame: {}", e)))?;
    }

    log::info!("Encoding crossfade video...");

    // Drop senders and stop encoder
    drop(video_tx);
    drop(_audio_tx);

    encoder.stop()
        .map_err(|e| Error::FFmpeg(format!("Failed to stop encoder: {}", e)))?;

    log::info!("Crossfade complete: {} + {} -> {} (overlap: {:.2}s)",
        config.video1, config.video2, config.output, config.overlap_duration);

    Ok(())
}

/// Blend two frames using alpha blending
fn blend_frames_rgb24(frame1: &[u8], frame2: &[u8], width: u32, height: u32, alpha: f32) -> Vec<u8> {
    let mut blended = vec![0u8; frame1.len()];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;

            // Linear interpolation between the two frames
            let r1 = frame1[idx as usize] as f32;
            let g1 = frame1[idx as usize + 1] as f32;
            let b1 = frame1[idx as usize + 2] as f32;

            let r2 = frame2[idx as usize] as f32;
            let g2 = frame2[idx as usize + 1] as f32;
            let b2 = frame2[idx as usize + 2] as f32;

            blended[idx as usize] = (r1 * (1.0 - alpha) + r2 * alpha).clamp(0.0, 255.0) as u8;
            blended[idx as usize + 1] = (g1 * (1.0 - alpha) + g2 * alpha).clamp(0.0, 255.0) as u8;
            blended[idx as usize + 2] = (b1 * (1.0 - alpha) + b2 * alpha).clamp(0.0, 255.0) as u8;
        }
    }

    blended
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crossfade_config() {
        let config = CrossfadeConfig::new("v1.mp4", "v2.mp4", "out.mp4", 2.5);
        assert_eq!(config.video1, "v1.mp4");
        assert_eq!(config.video2, "v2.mp4");
        assert_eq!(config.overlap_duration, 2.5);
    }

    #[test]
    fn test_blend_no_op() {
        // Alpha = 0 should return frame1 unchanged
        let frame1 = vec![100, 150, 200, 50, 75, 100];
        let frame2 = vec![0, 0, 0, 255, 255, 255];
        let blended = blend_frames_rgb24(&frame1, &frame2, 2, 1, 0.0);

        assert_eq!(blended[0], 100);
        assert_eq!(blended[1], 150);
        assert_eq!(blended[2], 200);
    }

    #[test]
    fn test_blend_full() {
        // Alpha = 1 should return frame2 unchanged
        let frame1 = vec![100, 150, 200, 50, 75, 100];
        let frame2 = vec![0, 0, 0, 255, 255, 255];
        let blended = blend_frames_rgb24(&frame1, &frame2, 2, 1, 1.0);

        assert_eq!(blended[0], 0);
        assert_eq!(blended[1], 0);
        assert_eq!(blended[2], 0);
    }

    #[test]
    fn test_blend_half() {
        // Alpha = 0.5 should average the frames
        let frame1 = vec![100, 100, 100];
        let frame2 = vec![200, 200, 200];
        let blended = blend_frames_rgb24(&frame1, &frame2, 1, 1, 0.5);

        assert_eq!(blended[0], 150);
        assert_eq!(blended[1], 150);
        assert_eq!(blended[2], 150);
    }
}
