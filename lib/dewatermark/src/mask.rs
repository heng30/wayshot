use image::{GrayImage, imageops};
use ndarray::Array4;

#[derive(Debug, Clone, Copy)]
pub struct WatermarkRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub enum MaskInput {
    Rect(WatermarkRegion),
    Pixels(GrayImage),
}

impl From<WatermarkRegion> for MaskInput {
    fn from(region: WatermarkRegion) -> Self {
        MaskInput::Rect(region)
    }
}

/// Generate a binary mask tensor [1, 1, H, W].
///
/// 1.0 inside the watermark region, 0.0 outside.
pub fn generate_mask(w: u32, h: u32, region: &WatermarkRegion) -> Array4<f32> {
    let mut mask = Array4::<f32>::zeros((1, 1, h as usize, w as usize));

    let y_end = (region.y + region.height).min(h);
    let x_end = (region.x + region.width).min(w);

    for y in region.y..y_end {
        for x in region.x..x_end {
            mask[[0, 0, y as usize, x as usize]] = 1.0;
        }
    }

    mask
}

/// Generate a binary mask tensor [1, 1, H, W] from a [`MaskInput`].
///
/// For `Rect`, the region coordinates are assumed to already be in the
/// target `(w, h)` coordinate space (callers must scale from original
/// image coordinates before calling this function).
pub fn generate_mask_from_input(w: u32, h: u32, input: &MaskInput) -> Array4<f32> {
    match input {
        MaskInput::Rect(region) => generate_mask(w, h, region),
        MaskInput::Pixels(gray) => {
            let resized = imageops::resize(gray, w, h, imageops::FilterType::Nearest);
            let mut mask = Array4::<f32>::zeros((1, 1, h as usize, w as usize));
            for y in 0..h {
                for x in 0..w {
                    if resized.get_pixel(x, y).0[0] > 0 {
                        mask[[0, 0, y as usize, x as usize]] = 1.0;
                    }
                }
            }
            mask
        }
    }
}

