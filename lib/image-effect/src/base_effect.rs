use crate::{Effect, ImageEffectResult};
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;

/// Invert the colors of an image
pub fn invert(image: &mut RgbaImage) -> ImageEffectResult<()> {
    for pixel in image.pixels_mut() {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
    }

    Ok(())
}

/// Grayscale effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GrayscaleConfig {
    #[derivative(Default(value = "GrayscaleMode::Luminance"))]
    mode: GrayscaleMode,
}

impl GrayscaleConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GrayscaleMode {
    Average,
    Luminance,
    RedChannel,
    GreenChannel,
    BlueChannel,
}

impl Effect for GrayscaleConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        for pixel in image.pixels_mut() {
            let gray = match self.mode {
                GrayscaleMode::Average => {
                    ((pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32) / 3) as u8
                }
                GrayscaleMode::Luminance => {
                    // Human perception: 0.299*R + 0.587*G + 0.114*B
                    (0.299 * pixel[0] as f32
                        + 0.587 * pixel[1] as f32
                        + 0.114 * pixel[2] as f32) as u8
                }
                GrayscaleMode::RedChannel => pixel[0],
                GrayscaleMode::GreenChannel => pixel[1],
                GrayscaleMode::BlueChannel => pixel[2],
            };

            pixel[0] = gray;
            pixel[1] = gray;
            pixel[2] = gray;
        }

        Ok(())
    }
}

/// Brightness adjustment configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct BrightnessConfig {
    #[derivative(Default(value = "10"))]
    brightness: i32,
}

impl BrightnessConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for BrightnessConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        for pixel in image.pixels_mut() {
            for i in 0..3 {
                let new_val = pixel[i] as i32 + self.brightness;
                pixel[i] = new_val.clamp(0, 255) as u8;
            }
        }

        Ok(())
    }
}

/// Contrast adjustment configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ContrastConfig {
    #[derivative(Default(value = "10.0"))]
    contrast: f32,
}

impl ContrastConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for ContrastConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Manual contrast adjustment to avoid photon-rs issues with large images
        // Contrast formula: new_color = (old_color - 128) * factor + 128
        // where factor = (259 * (contrast + 255)) / (255 * (259 - contrast))
        // For simplicity, we use: new_color = (old_color - 128) * (1 + contrast/100) + 128

        let factor = 1.0 + (self.contrast / 100.0);

        for pixel in image.pixels_mut() {
            for i in 0..3 {
                let val = pixel[i] as f32;
                let new_val = (val - 128.0) * factor + 128.0;
                pixel[i] = new_val.clamp(0.0, 255.0) as u8;
            }
        }

        Ok(())
    }
}

/// Saturation adjustment configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SaturationConfig {
    #[derivative(Default(value = "0.0"))]
    amount: f32,
}

impl SaturationConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SaturationConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Saturation adjustment using HSL-like approach
        // amount > 0 increases saturation, amount < 0 decreases it
        // amount is in range roughly -1.0 to 1.0

        let adjustment = 1.0 + self.amount;

        for pixel in image.pixels_mut() {
            // Convert to grayscale for luminance
            let gray = (0.299 * pixel[0] as f32
                + 0.587 * pixel[1] as f32
                + 0.114 * pixel[2] as f32) as u8;

            // Interpolate between gray and color based on adjustment
            for i in 0..3 {
                let val = pixel[i] as f32;
                let gray_val = gray as f32;
                let new_val = gray_val + (val - gray_val) * adjustment;
                pixel[i] = new_val.clamp(0.0, 255.0) as u8;
            }
        }

        Ok(())
    }
}

/// Hue rotation configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HueRotateConfig {
    #[derivative(Default(value = "0"))]
    degrees: i32,
}

impl HueRotateConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HueRotateConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Hue rotation using RGB to HSL conversion, rotate H, convert back
        let degrees = (self.degrees % 360 + 360) % 360; // Normalize to 0-360

        for pixel in image.pixels_mut() {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;

            // Convert RGB to HSL
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let delta = max - min;

            let lightness = (max + min) / 2.0;
            let saturation = if delta == 0.0 {
                0.0
            } else {
                delta / (1.0 - (2.0 * lightness - 1.0).abs())
            };

            let mut hue = if delta == 0.0 {
                0.0
            } else if max == r {
                60.0 * (((g - b) / delta) % 6.0)
            } else if max == g {
                60.0 * (((b - r) / delta + 2.0) % 6.0)
            } else {
                60.0 * (((r - g) / delta + 4.0) % 6.0)
            };

            if hue < 0.0 {
                hue += 360.0;
            }

            // Rotate hue
            let hue = (hue + degrees as f32) % 360.0;

            // Convert HSL back to RGB
            let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
            let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
            let m = lightness - c / 2.0;

            let (r_new, g_new, b_new) = if hue < 60.0 {
                (c, x, 0.0)
            } else if hue < 120.0 {
                (x, c, 0.0)
            } else if hue < 180.0 {
                (0.0, c, x)
            } else if hue < 240.0 {
                (0.0, x, c)
            } else if hue < 300.0 {
                (x, 0.0, c)
            } else {
                (c, 0.0, x)
            };

            pixel[0] = ((r_new + m) * 255.0).clamp(0.0, 255.0) as u8;
            pixel[1] = ((g_new + m) * 255.0).clamp(0.0, 255.0) as u8;
            pixel[2] = ((b_new + m) * 255.0).clamp(0.0, 255.0) as u8;
        }

        Ok(())
    }
}
