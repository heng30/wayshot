//! Inpainting trait and LaMa implementation.
//!
//! Provides the `Inpainter` trait and the LaMa single-frame ONNX backend.

mod lama;

pub use lama::LamaInpainter;

use crate::mask::InpaintArea;
use image::RgbImage;
use ndarray::Array2;

/// A binary mask for inpainting (255 = inpaint, 0 = keep).
pub type Mask = Array2<u8>;

/// Trait for video inpainting backends.
pub trait Inpainter {
    /// Inpaint the given frames using the mask.
    ///
    /// `frames` — RGB frames to process
    /// `mask` — binary mask (255 = area to inpaint)
    /// `area` — the sub-region of the frame being processed (ymin, ymax, xmin, xmax)
    ///
    /// Returns the inpainted frames.
    fn inpaint(
        &mut self,
        frames: &[RgbImage],
        mask: &Mask,
        area: &InpaintArea,
    ) -> Result<Vec<RgbImage>, InpaintError>;
}

/// Errors that can occur during inpainting.
#[derive(Debug, thiserror::Error)]
pub enum InpaintError {
    #[error("ONNX inference error: {0}")]
    Onnx(#[from] ort::Error),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Helper: crop a frame and mask to the inpaint area.
pub fn crop_to_area(
    frame: &RgbImage,
    mask: &Mask,
    area: &InpaintArea,
) -> (image::RgbImage, Vec<u8>) {
    let (ymin, ymax, xmin, xmax) = *area;
    let width = (xmax - xmin) as u32;
    let height = (ymax - ymin) as u32;

    let mut cropped = image::RgbImage::new(width, height);
    let mut cropped_mask = vec![0u8; (width * height) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_y = (y as i32 + ymin) as u32;
            let src_x = (x as i32 + xmin) as u32;
            if src_y < frame.height() && src_x < frame.width() {
                cropped.put_pixel(x, y, *frame.get_pixel(src_x, src_y));
            }
            let my = (y as i32 + ymin) as usize;
            let mx = (x as i32 + xmin) as usize;
            if my < mask.nrows() && mx < mask.ncols() {
                cropped_mask[(y * width + x) as usize] = mask[[my, mx]];
            }
        }
    }

    (cropped, cropped_mask)
}

/// Helper: convert RgbImage to ndarray [3, H, W] normalized to [0, 1].
pub(crate) fn rgb_image_to_chw(img: &RgbImage) -> ndarray::Array3<f32> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut arr = ndarray::Array3::zeros((3, h, w));
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32);
            arr[[0, y, x]] = p[0] as f32 / 255.0;
            arr[[1, y, x]] = p[1] as f32 / 255.0;
            arr[[2, y, x]] = p[2] as f32 / 255.0;
        }
    }
    arr
}

/// Helper: convert ndarray [3, H, W] in [0,1] to RgbImage.
pub(crate) fn chw_to_rgb_image(arr: &ndarray::Array3<f32>) -> RgbImage {
    let h = arr.shape()[1];
    let w = arr.shape()[2];
    let mut img = RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let r = (arr[[0, y, x]].clamp(0.0, 1.0) * 255.0) as u8;
            let g = (arr[[1, y, x]].clamp(0.0, 1.0) * 255.0) as u8;
            let b = (arr[[2, y, x]].clamp(0.0, 1.0) * 255.0) as u8;
            img.put_pixel(x as u32, y as u32, image::Rgb([r, g, b]));
        }
    }
    img
}

/// Pad dimensions to be a multiple of `mod_val`.
pub(crate) fn pad_to_multiple(dim: usize, mod_val: usize) -> usize {
    if mod_val == 0 {
        return dim;
    }
    let remainder = dim % mod_val;
    if remainder == 0 {
        dim
    } else {
        dim + mod_val - remainder
    }
}

/// Composite inpainted region back into the original frame.
///
/// For pixels where the mask is 0 (keep original), copy from the original frame;
/// for pixels near the mask boundary, blend inpainted and original using a
/// feathered transition to avoid visible seams.
pub fn composite_back(result: &mut RgbImage, original: &RgbImage, mask: &Mask, area: &InpaintArea) {
    let (ymin, ymax, xmin, xmax) = *area;
    let feather = 5;

    for y in 0..(ymax - ymin) as u32 {
        let src_y = y + ymin as u32;
        if src_y >= result.height() {
            continue;
        }
        for x in 0..(xmax - xmin) as u32 {
            let src_x = x + xmin as u32;
            if src_x >= result.width() {
                continue;
            }
            let my = src_y as usize;
            let mx = src_x as usize;
            if my >= mask.nrows() || mx >= mask.ncols() {
                continue;
            }

            let mask_val = mask[[my, mx]];
            if mask_val == 0 {
                if src_y < original.height() && src_x < original.width() {
                    result.put_pixel(src_x, src_y, *original.get_pixel(src_x, src_y));
                }
            } else {
                let dist = mask_dist_to_boundary(mask, my, mx);
                if dist >= feather {
                    // Far from boundary: keep inpainted result (already in result)
                } else if dist == 0 {
                    // On boundary: use original pixel entirely
                    if src_y < original.height() && src_x < original.width() {
                        result.put_pixel(src_x, src_y, *original.get_pixel(src_x, src_y));
                    }
                } else {
                    let alpha = dist as f32 / feather as f32;
                    if src_y < original.height() && src_x < original.width() {
                        let inpainted = result.get_pixel(src_x, src_y);
                        let orig = original.get_pixel(src_x, src_y);
                        let blended = image::Rgb([
                            (orig[0] as f32 * (1.0 - alpha) + inpainted[0] as f32 * alpha) as u8,
                            (orig[1] as f32 * (1.0 - alpha) + inpainted[1] as f32 * alpha) as u8,
                            (orig[2] as f32 * (1.0 - alpha) + inpainted[2] as f32 * alpha) as u8,
                        ]);
                        result.put_pixel(src_x, src_y, blended);
                    }
                }
            }
        }
    }
}

/// Find minimum Chebyshev distance from (row, col) to a mask boundary.
fn mask_dist_to_boundary(mask: &Mask, row: usize, col: usize) -> usize {
    let rows = mask.nrows();
    let cols = mask.ncols();
    let mut min_dist = usize::MAX;
    for dy in -8i32..=8 {
        for dx in -8i32..=8 {
            let nr = row as i32 + dy;
            let nc = col as i32 + dx;
            if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                min_dist = min_dist.min(dy.abs().max(dx.abs()) as usize);
            } else if mask[[nr as usize, nc as usize]] != mask[[row, col]] {
                min_dist = min_dist.min(dy.abs().max(dx.abs()) as usize);
            }
        }
    }
    if min_dist == usize::MAX { 9 } else { min_dist }
}
