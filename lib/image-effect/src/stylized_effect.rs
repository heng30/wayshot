use crate::{Effect, ImageEffectResult};
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;

/// Edge detection configuration (manual convolution implementation)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct EdgeDetectionConfig {
    #[derivative(Default(value = "EdgeDetectionMode::Sobel"))]
    mode: EdgeDetectionMode,
}

impl EdgeDetectionConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EdgeDetectionMode {
    Sobel,
    Laplacian,
}

impl Effect for EdgeDetectionConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        match self.mode {
            EdgeDetectionMode::Sobel => apply_sobel(image),
            EdgeDetectionMode::Laplacian => apply_laplacian(image),
        }
    }
}

fn apply_sobel(image: &mut RgbaImage) -> ImageEffectResult<()> {
    let mut result = image.clone();

    // Sobel operators
    let sobel_x: [i32; 9] = [-1, 0, 1, -2, 0, 2, -1, 0, 1];
    let sobel_y: [i32; 9] = [-1, -2, -1, 0, 0, 0, 1, 2, 1];

    for y in 1..image.height()-1 {
        for x in 1..image.width()-1 {
            let mut gx = 0i32;
            let mut gy = 0i32;

            // Apply Sobel kernels
            for ky in -1i32..=1 {
                for kx in -1i32..=1 {
                    let px = (x as i32 + kx) as u32;
                    let py = (y as i32 + ky) as u32;
                    let pixel = image.get_pixel(px, py);
                    let gray = (pixel[0] as i32 + pixel[1] as i32 + pixel[2] as i32) / 3;

                    let ki = ((ky + 1) * 3 + (kx + 1)) as usize;
                    gx += gray * sobel_x[ki];
                    gy += gray * sobel_y[ki];
                }
            }

            let magnitude = ((gx * gx + gy * gy) as f32).sqrt() as u8;
            let pixel = result.get_pixel_mut(x, y);
            pixel[0] = magnitude;
            pixel[1] = magnitude;
            pixel[2] = magnitude;
        }
    }

    *image = result;
    Ok(())
}

fn apply_laplacian(image: &mut RgbaImage) -> ImageEffectResult<()> {
    let mut result = image.clone();

    // Laplacian kernel
    let kernel: [i32; 9] = [0, 1, 0, 1, -4, 1, 0, 1, 0];

    for y in 1..image.height()-1 {
        for x in 1..image.width()-1 {
            let mut sum = 0i32;

            for ky in -1i32..=1 {
                for kx in -1i32..=1 {
                    let px = (x as i32 + kx) as u32;
                    let py = (y as i32 + ky) as u32;
                    let pixel = image.get_pixel(px, py);
                    let gray = (pixel[0] as i32 + pixel[1] as i32 + pixel[2] as i32) / 3;

                    let ki = ((ky + 1) * 3 + (kx + 1)) as usize;
                    sum += gray * kernel[ki];
                }
            }

            let val = sum.clamp(0, 255) as u8;
            let pixel = result.get_pixel_mut(x, y);
            pixel[0] = val;
            pixel[1] = val;
            pixel[2] = val;
        }
    }

    *image = result;
    Ok(())
}

/// Emboss effect configuration (manual convolution implementation)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct EmbossConfig {
    #[derivative(Default(value = "0.5"))]
    strength: f32,
}

impl EmbossConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for EmbossConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        let mut result = image.clone();

        // Emboss kernel
        let kernel: [i32; 9] = [-2, -1, 0, -1, 1, 1, 0, 1, 2];

        for y in 1..image.height()-1 {
            for x in 1..image.width()-1 {
                let mut sum = 0i32;

                for ky in -1i32..=1 {
                    for kx in -1i32..=1 {
                        let px = (x as i32 + kx) as u32;
                        let py = (y as i32 + ky) as u32;
                        let pixel = image.get_pixel(px, py);
                        let gray = (pixel[0] as i32 + pixel[1] as i32 + pixel[2] as i32) / 3;

                        let ki = ((ky + 1) * 3 + (kx + 1)) as usize;
                        sum += gray * kernel[ki];
                    }
                }

                let val = (sum as f32 * self.strength).clamp(0.0, 255.0) as u8;
                let pixel = result.get_pixel_mut(x, y);
                pixel[0] = val;
                pixel[1] = val;
                pixel[2] = val;
            }
        }

        *image = result;
        Ok(())
    }
}

/// Sharpen effect configuration (manual convolution implementation)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SharpenConfig {
    #[derivative(Default(value = "1.0"))]
    amount: f32,
}

impl SharpenConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SharpenConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        let mut result = image.clone();

        // Sharpen kernel
        let kernel: [f32; 9] = [
            0.0, -1.0 * self.amount, 0.0,
            -1.0 * self.amount, 1.0 + 4.0 * self.amount, -1.0 * self.amount,
            0.0, -1.0 * self.amount, 0.0,
        ];

        for y in 1..image.height()-1 {
            for x in 1..image.width()-1 {
                let mut sum_r = 0.0;
                let mut sum_g = 0.0;
                let mut sum_b = 0.0;

                for ky in -1i32..=1 {
                    for kx in -1i32..=1 {
                        let px = (x as i32 + kx) as u32;
                        let py = (y as i32 + ky) as u32;
                        let pixel = image.get_pixel(px, py);

                        let ki = ((ky + 1) * 3 + (kx + 1)) as usize;
                        sum_r += pixel[0] as f32 * kernel[ki];
                        sum_g += pixel[1] as f32 * kernel[ki];
                        sum_b += pixel[2] as f32 * kernel[ki];
                    }
                }

                let pixel = result.get_pixel_mut(x, y);
                pixel[0] = sum_r.clamp(0.0, 255.0) as u8;
                pixel[1] = sum_g.clamp(0.0, 255.0) as u8;
                pixel[2] = sum_b.clamp(0.0, 255.0) as u8;
            }
        }

        *image = result;
        Ok(())
    }
}

/// Pixelate effect configuration
/// Note: Manual implementation
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct PixelateConfig {
    #[derivative(Default(value = "10"))]
    block_size: u32,
}

impl PixelateConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for PixelateConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        let width = image.width();
        let height = image.height();
        let block_size = self.block_size.max(1);

        let mut result = image.clone();

        for y in (0..height).step_by(block_size as usize) {
            for x in (0..width).step_by(block_size as usize) {
                // Calculate average color for the block
                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;
                let mut count = 0u32;

                let y_end = (y + block_size).min(height);
                let x_end = (x + block_size).min(width);

                for by in y..y_end {
                    for bx in x..x_end {
                        let pixel = image.get_pixel(bx, by);
                        r_sum += pixel[0] as u32;
                        g_sum += pixel[1] as u32;
                        b_sum += pixel[2] as u32;
                        count += 1;
                    }
                }

                let avg_r = (r_sum / count) as u8;
                let avg_g = (g_sum / count) as u8;
                let avg_b = (b_sum / count) as u8;

                // Fill the block with average color
                for by in y..y_end {
                    for bx in x..x_end {
                        let pixel = result.get_pixel_mut(bx, by);
                        pixel[0] = avg_r;
                        pixel[1] = avg_g;
                        pixel[2] = avg_b;
                    }
                }
            }
        }

        *image = result;
        Ok(())
    }
}

/// Posterize effect configuration
/// Note: Manual implementation
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct PosterizeConfig {
    #[derivative(Default(value = "4"))]
    levels: u8,
}

impl PosterizeConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for PosterizeConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        let levels = self.levels.max(2) as f32;
        let step = 255.0 / (levels - 1.0);

        for pixel in image.pixels_mut() {
            // Posterize each channel
            pixel[0] = ((pixel[0] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
            pixel[1] = ((pixel[1] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
            pixel[2] = ((pixel[2] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
        }

        Ok(())
    }
}
