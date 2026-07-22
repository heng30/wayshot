//! Animation generation and WebP export (requires `animation` feature).
//!
//! Generates a sequence of frames for the page flip animation
//! and exports them as an animated WebP or as a collection of PNG images.

#![cfg(feature = "animation")]

use image::RgbaImage;
use rgb::RGBA8;
use std::path::Path;

use crate::render::{FlipConfig, render_flip};

/// Errors that can occur during flip animation generation.
#[derive(Debug, thiserror::Error)]
pub enum FlipError {
    /// No frames were generated.
    #[error("no frames generated")]
    EmptyFrames,

    /// Failed to encode WebP animation.
    #[error("WebP encoding failed: {0}")]
    WebpEncode(String),

    /// Failed to write file to disk.
    #[error("failed to write file: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to save a PNG frame.
    #[error("failed to save PNG frame: {0}")]
    ImageSave(#[from] image::ImageError),
}

/// Generate all frames of a page flip animation.
///
/// - `front` = the current page (being flipped away)
/// - `back` = the page revealed behind the fold
/// - `config` = flip configuration (direction, duration, corner, etc.)
/// - `frame_count` = number of frames to generate
///
/// The `config.time_ms` field is ignored; frames are generated at evenly
/// spaced intervals across `config.duration_ms`.
pub fn generate_flip(
    front: &RgbaImage,
    back: &RgbaImage,
    config: &FlipConfig,
    frame_count: u32,
) -> Vec<RgbaImage> {
    let n = frame_count;
    let mut frames = Vec::with_capacity(n as usize);

    for i in 0..n {
        let time_ms = if n > 1 {
            (config.duration_ms as u64 * i as u64 / (n - 1) as u64) as u32
        } else {
            config.duration_ms
        };
        let cfg = FlipConfig {
            time_ms,
            ..config.clone()
        };
        frames.push(render_flip(front, back, &cfg));
    }

    frames
}

/// Generate a page flip animation and save as an animated WebP file.
///
/// # Arguments
/// * `front` - The current page image (being flipped away)
/// * `back` - The page revealed behind the fold
/// * `config` - Flip configuration
/// * `frame_count` - Number of frames in the animation
/// * `output_path` - Path to save the animated WebP file
pub fn generate_flip_to_webp(
    front: &RgbaImage,
    back: &RgbaImage,
    config: &FlipConfig,
    frame_count: u32,
    output_path: &Path,
) -> Result<(), FlipError> {
    let frames = generate_flip(front, back, config, frame_count);

    if frames.is_empty() {
        return Err(FlipError::EmptyFrames);
    }

    let width = frames[0].width();
    let height = frames[0].height();
    let n = frames.len() as u32;
    let frame_duration_ms = if n > 1 {
        config.duration_ms / (n - 1)
    } else {
        config.duration_ms
    };

    let mut encoder = webpx::AnimationEncoder::with_options(width, height, true, 0)
        .map_err(|e| FlipError::WebpEncode(e.to_string()))?;

    encoder.set_quality(90.0);
    encoder.set_preset(webpx::Preset::Photo);

    for (i, frame) in frames.iter().enumerate() {
        let rgba_data: Vec<RGBA8> = frame
            .pixels()
            .map(|p| RGBA8::new(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        let timestamp_ms = (i as i32) * (frame_duration_ms as i32);
        encoder
            .add_frame(&rgba_data, timestamp_ms)
            .map_err(|e| FlipError::WebpEncode(e.to_string()))?;
    }

    let total_duration_ms = config.duration_ms as i32;
    let webp_data = encoder
        .finish(total_duration_ms)
        .map_err(|e| FlipError::WebpEncode(e.to_string()))?;
    std::fs::write(output_path, &webp_data)?;

    Ok(())
}

/// Generate a page flip animation and save each frame as a PNG image.
///
/// Frames are saved as `frame_0000.png`, `frame_0001.png`, ... in `output_dir`.
///
/// # Arguments
/// * `front` - The current page image (being flipped away)
/// * `back` - The page revealed behind the fold
/// * `config` - Flip configuration
/// * `frame_count` - Number of frames in the animation
/// * `output_dir` - Directory to save the PNG frames into
pub fn generate_flip_to_pngs(
    front: &RgbaImage,
    back: &RgbaImage,
    config: &FlipConfig,
    frame_count: u32,
    output_dir: &Path,
) -> Result<(), FlipError> {
    let frames = generate_flip(front, back, config, frame_count);

    if frames.is_empty() {
        return Err(FlipError::EmptyFrames);
    }

    std::fs::create_dir_all(output_dir)?;

    let digits = format!("{}", frame_count - 1).len();

    for (i, frame) in frames.iter().enumerate() {
        let filename = format!("frame_{:0>width$}.png", i, width = digits);
        let path = output_dir.join(&filename);
        frame.save(&path)?;
    }

    Ok(())
}
