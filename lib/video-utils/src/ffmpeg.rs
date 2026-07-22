//! FFmpeg video/audio frame conversion utilities.
//!
//! Provides helpers for converting decoded FFmpeg frames into standard Rust
//! image and audio types. Only available when the `ffmpeg` feature is enabled.

use crate::convert::rgb_into_rgba;
use ffmpeg_next as ffmpeg;
use image::{RgbImage, RgbaImage};
use std::time::Duration;
use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix, yuv420_to_rgba};

/// Convert a decoded FFmpeg video frame to an RGBA image.
///
/// Supports RGB24, RGBA, and YUV420P pixel formats.
/// For RGB24 input, alpha is set to 255.
pub fn frame_to_rgba(frame: &ffmpeg::frame::Video) -> crate::Result<RgbaImage> {
    let format = frame.format();

    match format {
        ffmpeg::format::Pixel::RGB24 => {
            let rgb_img = extract_rgb24(frame)?;
            Ok(rgb_into_rgba(rgb_img))
        }
        ffmpeg::format::Pixel::RGBA => extract_rgba(frame),
        ffmpeg::format::Pixel::YUV420P => yuv420p_to_rgba(frame),
        _ => Err(crate::Error::FFmpeg(format!(
            "Unsupported pixel format: {:?}",
            format
        ))),
    }
}

/// Convert a decoded FFmpeg video frame to an RGB image.
///
/// Supports RGB24, RGBA, and YUV420P pixel formats.
/// For RGBA input, alpha is discarded.
pub fn frame_to_rgb(frame: &ffmpeg::frame::Video) -> crate::Result<RgbImage> {
    use crate::convert::rgba_to_rgb;

    let format = frame.format();

    match format {
        ffmpeg::format::Pixel::RGB24 => extract_rgb24(frame),
        ffmpeg::format::Pixel::RGBA => {
            let rgba = extract_rgba(frame)?;
            Ok(rgba_to_rgb(&rgba))
        }
        ffmpeg::format::Pixel::YUV420P => {
            let rgba = yuv420p_to_rgba(frame)?;
            Ok(rgba_to_rgb(&rgba))
        }
        _ => Err(crate::Error::FFmpeg(format!(
            "Unsupported pixel format: {:?}",
            format
        ))),
    }
}

/// Convert a decoded FFmpeg audio frame to interleaved f32 samples.
///
/// Supports F32, I16, and I32 sample formats in both packed and planar layouts.
/// Integer samples are normalized to the [-1.0, 1.0] range.
pub fn audio_frame_to_f32(frame: &ffmpeg::frame::Audio) -> Vec<f32> {
    let channels = frame.channels() as usize;
    let samples = frame.samples();
    if channels == 0 || samples == 0 {
        return Vec::new();
    }

    let format = frame.format();
    let mut output = Vec::with_capacity(channels * samples);

    match format {
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed) => {
            let data = frame.data(0);
            let slice = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const f32, samples * channels)
            };
            output.extend_from_slice(slice);
        }
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar) => {
            for i in 0..samples {
                for ch in 0..channels {
                    let plane = frame.data(ch);
                    let channel_data = unsafe {
                        std::slice::from_raw_parts(plane.as_ptr() as *const f32, samples)
                    };
                    output.push(channel_data[i]);
                }
            }
        }
        ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed) => {
            let data = frame.data(0);
            let slice = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const i16, samples * channels)
            };
            const SCALE: f32 = 1.0 / (i16::MAX as f32 + 1.0);
            output.extend(slice.iter().map(|&s| s as f32 * SCALE));
        }
        ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Planar) => {
            const SCALE: f32 = 1.0 / (i16::MAX as f32 + 1.0);
            for i in 0..samples {
                for ch in 0..channels {
                    let plane = frame.data(ch);
                    let channel_data = unsafe {
                        std::slice::from_raw_parts(plane.as_ptr() as *const i16, samples)
                    };
                    output.push(channel_data[i] as f32 * SCALE);
                }
            }
        }
        ffmpeg::format::Sample::I32(ffmpeg::format::sample::Type::Packed) => {
            let data = frame.data(0);
            let slice = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const i32, samples * channels)
            };
            const SCALE: f32 = 1.0 / (i32::MAX as f32 + 1.0);
            output.extend(slice.iter().map(|&s| s as f32 * SCALE));
        }
        ffmpeg::format::Sample::I32(ffmpeg::format::sample::Type::Planar) => {
            const SCALE: f32 = 1.0 / (i32::MAX as f32 + 1.0);
            for i in 0..samples {
                for ch in 0..channels {
                    let plane = frame.data(ch);
                    let channel_data = unsafe {
                        std::slice::from_raw_parts(plane.as_ptr() as *const i32, samples)
                    };
                    output.push(channel_data[i] as f32 * SCALE);
                }
            }
        }
        _ => {
            log::warn!("Unsupported audio format: {:?}, skipping", format);
        }
    }

    output
}

/// Seek the input context to a given timestamp.
pub fn seek_to_time(input_ctx: &mut ffmpeg::format::context::Input, target_time: Duration) {
    let seek_timestamp = (target_time.as_secs_f64() * ffmpeg::sys::AV_TIME_BASE as f64) as i64;
    if seek_timestamp > 0
        && let Err(e) = input_ctx.seek(seek_timestamp, ..)
    {
        log::warn!("Seek to {:.3}s failed: {}", target_time.as_secs_f64(), e);
    }
}

/// Extract RGB24 pixel data from a decoded FFmpeg video frame.
pub fn extract_rgb24(frame: &ffmpeg::frame::Video) -> crate::Result<RgbImage> {
    let width = frame.width();
    let height = frame.height();
    let stride = frame.stride(0);
    let data = frame.data(0);
    let expected_len = width as usize * height as usize * 3;

    if data.len() == expected_len {
        RgbImage::from_raw(width, height, data[..expected_len].to_vec())
            .ok_or_else(|| crate::Error::FFmpeg("Failed to create RGB image".into()))
    } else {
        let mut pixel_data = Vec::with_capacity(expected_len);
        let row_size = width as usize * 3;
        for y in 0..height as usize {
            let row_start = y * stride;
            let row_end = row_start + row_size;
            if row_end <= data.len() {
                pixel_data.extend_from_slice(&data[row_start..row_end]);
            }
        }
        RgbImage::from_raw(width, height, pixel_data)
            .ok_or_else(|| crate::Error::FFmpeg("Failed to create RGB image from stride".into()))
    }
}

/// Extract RGBA pixel data from a decoded FFmpeg video frame.
pub fn extract_rgba(frame: &ffmpeg::frame::Video) -> crate::Result<RgbaImage> {
    let width = frame.width();
    let height = frame.height();
    let stride = frame.stride(0);
    let data = frame.data(0);
    let expected_len = width as usize * height as usize * 4;

    if data.len() == expected_len {
        let pixel_data: Vec<u8> = data[..expected_len].to_vec();
        RgbaImage::from_raw(width, height, pixel_data)
            .ok_or_else(|| crate::Error::FFmpeg("Failed to create RGBA image".into()))
    } else {
        let mut pixel_data = Vec::with_capacity(expected_len);
        let row_size = width as usize * 4;
        for y in 0..height as usize {
            let row_start = y * stride;
            let row_end = row_start + row_size;
            if row_end <= data.len() {
                pixel_data.extend_from_slice(&data[row_start..row_end]);
            }
        }
        RgbaImage::from_raw(width, height, pixel_data)
            .ok_or_else(|| crate::Error::FFmpeg("Failed to create RGBA image from stride".into()))
    }
}

/// Convert a YUV420P FFmpeg frame to RGBA.
pub fn yuv420p_to_rgba(frame: &ffmpeg::frame::Video) -> crate::Result<RgbaImage> {
    let width = frame.width();
    let height = frame.height();

    let yuv_planar_image = YuvPlanarImage {
        y_plane: frame.data(0),
        y_stride: frame.stride(0) as u32,
        u_plane: frame.data(1),
        u_stride: frame.stride(1) as u32,
        v_plane: frame.data(2),
        v_stride: frame.stride(2) as u32,
        width,
        height,
    };

    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    yuv420_to_rgba(
        &yuv_planar_image,
        &mut rgba_data,
        width * 4,
        YuvRange::Limited,
        YuvStandardMatrix::Bt601,
    )
    .map_err(|e| crate::Error::FFmpeg(format!("YUV to RGBA conversion failed: {:?}", e)))?;

    RgbaImage::from_raw(width, height, rgba_data)
        .ok_or_else(|| crate::Error::FFmpeg("Failed to create RGBA image from YUV".into()))
}
