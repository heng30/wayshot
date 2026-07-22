//! Image processing utilities for VL models.

use std::collections::HashSet;

use crate::util::{ceil_by_factor, floor_by_factor, round_by_factor};
use crate::error::{LfmError, Result};
use candle_core::{DType, Device, Tensor};
use image::DynamicImage;

/// Convert a `DynamicImage` to a normalized tensor `(C, H, W)`.
pub fn img_transform(
    img: &DynamicImage,
    mean: &Tensor,
    std: &Tensor,
    device: &Device,
    dtype: DType,
) -> Result<Tensor> {
    let img_h = img.height();
    let img_w = img.width();
    let img_vec = img.to_rgb8().into_raw();
    let img_tensor = Tensor::from_slice(&img_vec, (img_h as usize, img_w as usize, 3), device)?
        .permute((2, 0, 1))?
        .to_dtype(DType::F32)?;
    let img_tensor = img_tensor.affine(1.0 / 255.0, 0.0)?;
    let img_tensor = img_tensor
        .broadcast_sub(&mean.to_dtype(DType::F32)?)?
        .broadcast_div(&std.to_dtype(DType::F32)?)?
        .to_dtype(dtype)?;
    Ok(img_tensor)
}

/// Smart-resize: compute target (height, width) for an image.
pub fn img_smart_resize(
    img_h: u32,
    img_w: u32,
    factor: u32,
    min_pixels: u32,
    max_pixels: u32,
) -> Result<(u32, u32)> {
    if std::cmp::max(img_h, img_w) / std::cmp::min(img_h, img_w) > 200 {
        return Err(LfmError::ImageProcessing(format!(
            "absolute aspect ratio must be smaller than 200, got {}",
            std::cmp::max(img_h, img_w) / std::cmp::min(img_h, img_w)
        )));
    }
    let mut h_bar = std::cmp::max(factor, round_by_factor(img_h, factor));
    let mut w_bar = std::cmp::max(factor, round_by_factor(img_w, factor));

    if h_bar * w_bar > max_pixels {
        let beta = ((img_h * img_w) as f32 / max_pixels as f32).sqrt();
        h_bar = std::cmp::max(factor, floor_by_factor(img_h as f32 / beta, factor));
        w_bar = std::cmp::max(factor, floor_by_factor(img_w as f32 / beta, factor));
    } else if h_bar * w_bar < min_pixels {
        let beta = (min_pixels as f32 / (img_h * img_w) as f32).sqrt();
        h_bar = ceil_by_factor(img_h as f32 * beta, factor);
        w_bar = ceil_by_factor(img_w as f32 * beta, factor);
    }
    Ok((h_bar, w_bar))
}

/// Generate sorted target aspect ratios for tiling.
pub fn generate_target_ratios_sorted(min_num: u32, max_num: u32) -> Vec<(u32, u32)> {
    let mut target_ratios = HashSet::new();
    for n in min_num..=max_num {
        for i in 1..=n {
            for j in 1..=n {
                let product = i * j;
                if product <= max_num && product >= min_num {
                    target_ratios.insert((i, j));
                }
            }
        }
    }
    let mut sorted: Vec<(u32, u32)> = target_ratios.into_iter().collect();
    sorted.sort_by_key(|&(i, j)| i * j);
    sorted
}

/// Find the closest aspect ratio from a list of target ratios.
pub fn find_closest_aspect_ratio(
    aspect_ratio: f64,
    target_ratios: &[(u32, u32)],
    width: u32,
    height: u32,
    image_size: u32,
) -> (u32, u32) {
    let mut best_ratio_diff = f64::INFINITY;
    let mut best_ratio = (1, 1);
    let area = width * height;
    for &ratio in target_ratios {
        let target_ar = ratio.0 as f64 / ratio.1 as f64;
        let diff = (aspect_ratio - target_ar).abs();
        if diff < best_ratio_diff {
            best_ratio_diff = diff;
            best_ratio = ratio;
        } else if (diff - best_ratio_diff).abs() < 1e-10 {
            let target_area = 0.5 * (image_size as f64).powi(2) * (ratio.0 * ratio.1) as f64;
            if area as f64 > target_area {
                best_ratio = ratio;
            }
        }
    }
    best_ratio
}

/// Crop an image into a grid of tiles.
pub fn crop_img(
    image: &DynamicImage,
    grid_height: u32,
    grid_width: u32,
    image_size: u32,
) -> Vec<DynamicImage> {
    let target_width = image_size * grid_width;
    let target_height = image_size * grid_height;
    let blocks = grid_width * grid_height;
    let mut resized = image.resize_exact(
        target_width,
        target_height,
        image::imageops::FilterType::CatmullRom,
    );
    let mut processed = Vec::new();
    for i in 0..blocks {
        let x1 = (i % grid_width) * image_size;
        let y1 = (i / grid_width) * image_size;
        let split = resized.crop(x1, y1, image_size, image_size);
        processed.push(split);
    }
    assert_eq!(processed.len() as u32, blocks);
    processed
}
