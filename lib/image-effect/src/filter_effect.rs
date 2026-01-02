use crate::{Effect, ImageEffectResult};
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;

/// Sepia tone configuration (using photon-rs)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SepiaConfig {
    #[derivative(Default(value = "0.5"))]
    intensity: f32,
}

impl SepiaConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SepiaConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Manual sepia implementation to avoid photon-rs issues
        for pixel in image.pixels_mut() {
            let r = pixel[0] as f32;
            let g = pixel[1] as f32;
            let b = pixel[2] as f32;

            // Sepia tone transformation
            let tr = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0);
            let tg = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0);
            let tb = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0);

            // Blend with original based on intensity
            let intensity = self.intensity.clamp(0.0, 1.0);
            pixel[0] = (r * (1.0 - intensity) + tr * intensity) as u8;
            pixel[1] = (g * (1.0 - intensity) + tg * intensity) as u8;
            pixel[2] = (b * (1.0 - intensity) + tb * intensity) as u8;
        }

        Ok(())
    }
}

/// Color temperature configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TemperatureConfig {
    #[derivative(Default(value = "0.0"))]
    amount: f32,
}

impl TemperatureConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_warm(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Adjust color temperature towards warm (increase red, decrease blue)
        for pixel in image.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * (1.0 + self.amount * 0.1)).min(255.0) as u8;
            pixel[2] = (pixel[2] as f32 * (1.0 - self.amount * 0.1)).max(0.0) as u8;
        }

        Ok(())
    }

    pub fn apply_cool(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Adjust color temperature towards cool (increase blue, decrease red)
        for pixel in image.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * (1.0 - self.amount * 0.1)).max(0.0) as u8;
            pixel[2] = (pixel[2] as f32 * (1.0 + self.amount * 0.1)).min(255.0) as u8;
        }

        Ok(())
    }
}

/// Color tint configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ColorTintConfig {
    #[derivative(Default(value = "255"))]
    r: u8,

    #[derivative(Default(value = "0"))]
    g: u8,

    #[derivative(Default(value = "0"))]
    b: u8,
}

impl ColorTintConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl Effect for ColorTintConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        // Apply color tint by blending
        for pixel in image.pixels_mut() {
            // Blend the tint color with the original pixel
            let blend_ratio = 0.3; // 30% tint
            pixel[0] = (pixel[0] as f32 * (1.0 - blend_ratio) + self.r as f32 * blend_ratio) as u8;
            pixel[1] = (pixel[1] as f32 * (1.0 - blend_ratio) + self.g as f32 * blend_ratio) as u8;
            pixel[2] = (pixel[2] as f32 * (1.0 - blend_ratio) + self.b as f32 * blend_ratio) as u8;
        }

        Ok(())
    }
}

/// Vignette configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct VignetteConfig {
    #[derivative(Default(value = "0.3"))]
    strength: f32,

    #[derivative(Default(value = "0.5"))]
    radius: f32,
}

impl VignetteConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for VignetteConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        let width = image.width();
        let height = image.height();
        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;
        let max_distance = (center_x * center_x + center_y * center_y).sqrt();

        for (y, row) in image.rows_mut().enumerate() {
            for (x, pixel) in row.enumerate() {
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let distance = (dx * dx + dy * dy).sqrt();

                // Calculate vignette factor
                let normalized_distance = distance / max_distance;
                let vignette_factor = 1.0 - (normalized_distance * self.strength).min(self.strength);

                // Apply vignette
                pixel[0] = (pixel[0] as f32 * vignette_factor) as u8;
                pixel[1] = (pixel[1] as f32 * vignette_factor) as u8;
                pixel[2] = (pixel[2] as f32 * vignette_factor) as u8;
            }
        }

        Ok(())
    }
}
