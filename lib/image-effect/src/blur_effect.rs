use crate::{Effect, ImageEffectResult};
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;

/// Gaussian blur configuration
/// Note: Using manual implementation to avoid photon-rs issues with large images
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GaussianBlurConfig {
    #[derivative(Default(value = "3"))]
    radius: i32,
}

impl GaussianBlurConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for GaussianBlurConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Simplified Gaussian blur using box blur approximation
        // Multiple box blur passes approximate Gaussian blur
        let radius = self.radius.max(1) as u32;
        let mut temp = image.clone();

        // Horizontal pass
        for y in 0..image.height() {
            for x in 0..image.width() {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;

                for dy in -(radius as i32)..=(radius as i32) {
                    for dx in -(radius as i32)..=(radius as i32) {
                        let nx = (x as i32 + dx).clamp(0, image.width() as i32 - 1) as u32;
                        let ny = (y as i32 + dy).clamp(0, image.height() as i32 - 1) as u32;
                        let pixel = image.get_pixel(nx, ny);
                        sum_r += pixel[0] as u32;
                        sum_g += pixel[1] as u32;
                        sum_b += pixel[2] as u32;
                        count += 1;
                    }
                }

                let pixel = temp.get_pixel_mut(x, y);
                pixel[0] = (sum_r / count) as u8;
                pixel[1] = (sum_g / count) as u8;
                pixel[2] = (sum_b / count) as u8;
            }
        }

        *image = temp;
        Ok(())
    }
}

/// Box blur configuration (using manual implementation)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct BoxBlurConfig {
    #[derivative(Default(value = "3"))]
    radius: u32,
}

impl BoxBlurConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for BoxBlurConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        let radius = self.radius.max(1);
        let mut temp = image.clone();

        for y in 0..image.height() {
            for x in 0..image.width() {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;

                // Simple box blur kernel
                for dy in 0..=(radius as i32) {
                    for dx in 0..=(radius as i32) {
                        let nx = (x as i32 + dx).clamp(0, image.width() as i32 - 1) as u32;
                        let ny = (y as i32 + dy).clamp(0, image.height() as i32 - 1) as u32;
                        let pixel = image.get_pixel(nx, ny);
                        sum_r += pixel[0] as u32;
                        sum_g += pixel[1] as u32;
                        sum_b += pixel[2] as u32;
                        count += 1;
                    }
                }

                let pixel = temp.get_pixel_mut(x, y);
                pixel[0] = (sum_r / count) as u8;
                pixel[1] = (sum_g / count) as u8;
                pixel[2] = (sum_b / count) as u8;
            }
        }

        *image = temp;
        Ok(())
    }
}

/// Median blur configuration (using imageproc)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct MedianBlurConfig {
    #[derivative(Default(value = "3"))]
    radius: u32,
}

impl MedianBlurConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for MedianBlurConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Use imageproc since it's more efficient
        let gray_image = image::imageops::grayscale(image);

        // Apply median filter using imageproc
        let filtered = imageproc::filter::median_filter(&gray_image, self.radius, self.radius);

        // Convert back to RGBA
        let mut result = image.clone();
        for (pixel, gray_pixel) in result.pixels_mut().zip(filtered.pixels()) {
            let gray = gray_pixel.0[0];
            *pixel = image::Rgba([gray, gray, gray, pixel[3]]);
        }

        *image = result;

        Ok(())
    }
}
