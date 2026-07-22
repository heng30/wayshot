use crate::{Error, Result};
use fast_image_resize::{PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image};
use image::{ImageBuffer, Rgb, RgbImage, Rgba, RgbaImage, buffer::ConvertBuffer};
use std::time::Duration;

// 精度安全的时间到帧转换（使用整数运算避免浮点误差）
// 将 Duration 转换为帧索引，使用整数运算保证精度
fn duration_to_frame(duration: Duration, fps: f32) -> usize {
    let nanos = duration.as_nanos() as u128;
    let fps_scaled = (fps * 1000.0) as u128; // 保留 3 位小数精度

    // frame = nanos * fps / 1_000_000_000_000 = (nanos * fps_scaled / 1000) / 1_000_000_000
    let frame = (nanos * fps_scaled + 500_000_000_000) / 1_000_000_000_000; // 四舍五入
    frame as usize
}

pub fn rgb_into_rgba(rgb_image: RgbImage) -> RgbaImage {
    let (width, height) = rgb_image.dimensions();
    let rgb_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb_image.into_raw())
            .expect("Failed to create RGB image buffer");
    rgb_buffer.convert()
}

pub fn rgb_to_rgba(rgb_image: &RgbImage) -> RgbaImage {
    let mut rgba = RgbaImage::new(rgb_image.width(), rgb_image.height());
    for (x, y, pixel) in rgb_image.enumerate_pixels() {
        let [r, g, b] = pixel.0;
        rgba.put_pixel(x, y, Rgba([r, g, b, 255]));
    }
    rgba
}

pub fn rgba_into_rgb(rgba_image: RgbaImage) -> RgbImage {
    let (width, height) = rgba_image.dimensions();
    let rgba_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_image.into_raw())
            .expect("Failed to create RGBA image buffer");
    rgba_buffer.convert()
}

pub fn rgba_to_rgb(rgba: &RgbaImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    ImageBuffer::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y);
        Rgb([pixel[0], pixel[1], pixel[2]])
    })
}

pub fn resample_video_frames<T: Clone>(
    all_frames: Vec<(usize, T)>,
    duration: Duration,
    source_fps: f32,
    output_fps: f32,
) -> Vec<T> {
    let target_frame_count = duration_to_frame(duration, output_fps);
    if source_fps == output_fps {
        // 帧率相同，但也需要根据duration限制帧数
        let result: Vec<T> = all_frames
            .into_iter()
            .take(target_frame_count)
            .map(|(_, frame)| frame)
            .collect();

        return result;
    }

    // 从第一个元素获取起始帧索引
    let start_frame_index = all_frames.first().map(|(idx, _)| *idx).unwrap_or(0);

    // 帧率不同，需要重采样
    let output_frame_interval = Duration::from_secs_f64(1.0 / output_fps as f64);
    let mut sampled_frames = Vec::new();
    let mut target_time = Duration::ZERO;

    while target_time < duration && sampled_frames.len() < target_frame_count {
        let elapsed_frames = duration_to_frame(target_time, source_fps);
        let source_frame_index = start_frame_index + elapsed_frames;

        let pos = all_frames.partition_point(|(idx, _)| *idx < source_frame_index);

        // 检查是否找到精确匹配
        let frame = if pos < all_frames.len() && all_frames[pos].0 == source_frame_index {
            // 精确匹配
            Some(&all_frames[pos].1)
        } else if pos > 0 && pos < all_frames.len() {
            // 在中间位置，比较前后两个取更近的
            let prev_diff = (source_frame_index as f64 - all_frames[pos - 1].0 as f64).abs();
            let curr_diff = (all_frames[pos].0 as f64 - source_frame_index as f64).abs();
            if prev_diff < curr_diff {
                Some(&all_frames[pos - 1].1)
            } else {
                Some(&all_frames[pos].1)
            }
        } else if pos == 0 {
            // 目标小于所有帧，取第一个
            all_frames.first().map(|(_, frame)| frame)
        } else {
            // pos >= all_frames.len()，目标大于所有帧，取最后一个
            all_frames.last().map(|(_, frame)| frame)
        };

        if let Some(frame) = frame {
            sampled_frames.push(frame.clone());
        }

        target_time += output_frame_interval;
    }

    sampled_frames
}

// 线性重采样（最近邻算法）
pub fn linear_resample<T: Clone>(frames: Vec<T>, target_count: usize) -> Vec<T> {
    if frames.is_empty() {
        return Vec::new();
    }

    let source_count = frames.len();
    if target_count == source_count {
        return frames;
    }

    let mut resampled = Vec::with_capacity(target_count);

    for target_idx in 0..target_count {
        // 计算对应的源帧索引（使用最近邻算法）
        let src_idx = (target_idx * source_count) / target_count;
        let src_idx = src_idx.min(source_count - 1);

        if let Some(frame) = frames.get(src_idx) {
            resampled.push(frame.clone());
        } else {
            resampled.push(frames.last().unwrap().clone());
        }
    }

    resampled
}

pub fn resize_rgba_image(
    img: RgbaImage,
    target_width: u32,
    target_height: u32,
) -> Result<RgbaImage> {
    let width = img.width();
    let height = img.height();

    if width == target_width && height == target_height {
        return Ok(img);
    }

    let src_image = Image::from_vec_u8(width, height, img.clone().into_raw(), PixelType::U8x4)?;
    let mut dst_image = Image::new(target_width, target_height, PixelType::U8x4);

    Resizer::new().resize(
        &src_image,
        &mut dst_image,
        &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(
            fast_image_resize::FilterType::Lanczos3,
        )),
    )?;

    Ok(
        image::RgbaImage::from_raw(target_width, target_height, dst_image.into_vec()).ok_or_else(
            || Error::ImageFrom {
                expected: (width, height),
                actual: (target_width, target_height),
            },
        )?,
    )
}

pub fn resize_rgb_image(img: RgbImage, target_width: u32, target_height: u32) -> Result<RgbImage> {
    let width = img.width();
    let height = img.height();

    if width == target_width && height == target_height {
        return Ok(img);
    }

    let src_image = Image::from_vec_u8(width, height, img.clone().into_raw(), PixelType::U8x3)?;
    let mut dst_image = Image::new(target_width, target_height, PixelType::U8x3);

    Resizer::new().resize(
        &src_image,
        &mut dst_image,
        &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(
            fast_image_resize::FilterType::Lanczos3,
        )),
    )?;

    Ok(
        image::RgbImage::from_raw(target_width, target_height, dst_image.into_vec()).ok_or_else(
            || Error::ImageFrom {
                expected: (width, height),
                actual: (target_width, target_height),
            },
        )?,
    )
}

pub fn resize_rgba_image_contain(
    img: RgbaImage,
    target_width: u32,
    target_height: u32,
    padding: bool,
) -> Result<RgbaImage> {
    let src_width = img.width();
    let src_height = img.height();

    if src_width == target_width && src_height == target_height {
        return Ok(img);
    }

    let width_ratio = target_width as f64 / src_width as f64;
    let height_ratio = target_height as f64 / src_height as f64;
    let scale = width_ratio.min(height_ratio);

    let scaled_width = (src_width as f64 * scale).round() as u32;
    let scaled_height = (src_height as f64 * scale).round() as u32;
    let scaled_width = scaled_width.max(1);
    let scaled_height = scaled_height.max(1);

    let scaled_image = if scaled_width != src_width || scaled_height != src_height {
        resize_rgba_image(img, scaled_width, scaled_height)?
    } else {
        img
    };

    if padding {
        let mut canvas = RgbaImage::new(target_width, target_height);
        let x_offset = (target_width - scaled_width) / 2;
        let y_offset = (target_height - scaled_height) / 2;

        for y in 0..scaled_height {
            for x in 0..scaled_width {
                if let Some(pixel) = scaled_image.get_pixel_checked(x, y) {
                    canvas.put_pixel(x_offset + x, y_offset + y, *pixel);
                }
            }
        }

        Ok(canvas)
    } else {
        Ok(scaled_image)
    }
}
