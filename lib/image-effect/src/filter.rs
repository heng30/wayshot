use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, effects, monochrome};
use rand::Rng;
use rayon::prelude::*;

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
        monochrome::sepia(&mut photon_img);
        let sepia_pixels = photon_img.get_raw_pixels();

        // If intensity is not 1.0, blend with original
        if self.intensity < 1.0 {
            let original_pixels = image.to_vec();
            let mut blended_pixels = Vec::with_capacity(sepia_pixels.len());

            for (original, sepia) in original_pixels.chunks(4).zip(sepia_pixels.chunks(4)) {
                blended_pixels.push(
                    (original[0] as f32 * (1.0 - self.intensity) + sepia[0] as f32 * self.intensity)
                        as u8,
                );
                blended_pixels.push(
                    (original[1] as f32 * (1.0 - self.intensity) + sepia[1] as f32 * self.intensity)
                        as u8,
                );
                blended_pixels.push(
                    (original[2] as f32 * (1.0 - self.intensity) + sepia[2] as f32 * self.intensity)
                        as u8,
                );
                blended_pixels.push(original[3]); // Keep alpha
            }

            RgbaImage::from_raw(width, height, blended_pixels)
        } else {
            RgbaImage::from_raw(width, height, sepia_pixels)
        }
    }
}

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
}

impl Effect for TemperatureConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);

        // Positive amount = warm (more red/yellow)
        // Negative amount = cool (more blue)
        if self.amount > 0.0 {
            // Warm: increase red, decrease blue
            effects::tint(
                &mut photon_img,
                (self.amount * 20.0) as u32,
                (self.amount * 10.0) as u32,
                0,
            );
        } else {
            // Cool: decrease red, increase blue
            let cool_amount = -self.amount;
            effects::tint(&mut photon_img, 0, 0, (cool_amount * 20.0) as u32);
        }

        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);

        effects::tint(
            &mut photon_img,
            self.r as u32 / 3,
            self.g as u32 / 3,
            self.b as u32 / 3,
        );

        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let width = image.width();
        let height = image.height();
        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;
        let max_distance = (center_x * center_x + center_y * center_y).sqrt();

        let mut pixels = image.to_vec();

        for (i, chunk) in pixels.chunks_mut(4).enumerate() {
            // Calculate position from pixel index
            let x = (i % width as usize) as f32;
            let y = (i / width as usize) as f32;

            let dx = x - center_x;
            let dy = y - center_y;
            let distance = (dx * dx + dy * dy).sqrt();

            // Calculate vignette factor based on radius and strength
            let normalized_distance = distance / (max_distance * self.radius);
            let vignette_factor = 1.0
                - (normalized_distance * self.strength)
                    .min(self.strength)
                    .max(0.0);

            // Apply vignette to RGB channels
            chunk[0] = (chunk[0] as f32 * vignette_factor) as u8;
            chunk[1] = (chunk[1] as f32 * vignette_factor) as u8;
            chunk[2] = (chunk[2] as f32 * vignette_factor) as u8;
        }

        RgbaImage::from_raw(width, height, pixels)
    }
}

/// Black and white TV snow noise effect (static/white noise)
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SnowNoiseConfig {
    /// Noise intensity (0.0 - 1.0), controls what fraction of pixels get noise
    #[derivative(Default(value = "0.3"))]
    intensity: f32,

    /// Whether to convert to grayscale first for authentic B&W TV look
    #[derivative(Default(value = "true"))]
    grayscale: bool,

    /// Minimum noise brightness value (0-255)
    #[derivative(Default(value = "100"))]
    min_brightness: u8,

    /// Maximum noise brightness value (0-255)
    #[derivative(Default(value = "255"))]
    max_brightness: u8,
}

impl SnowNoiseConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SnowNoiseConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let width = image.width() as usize;
        let height = image.height() as usize;
        let mut pixels = image.into_raw();

        let intensity = self.intensity.clamp(0.0, 1.0);

        if self.grayscale {
            pixels.par_chunks_exact_mut(4).for_each(|pixel| {
                let r = pixel[0] as u32;
                let g = pixel[1] as u32;
                let b = pixel[2] as u32;
                let gray = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
                pixel[0] = gray;
                pixel[1] = gray;
                pixel[2] = gray;
            });
        }

        // Apply noise by processing each row in parallel
        let intensity_clamped = intensity.clamp(0.0, 1.0);
        let min_brightness = self.min_brightness;
        let max_brightness = self.max_brightness.max(min_brightness);
        let row_stride = width * 4;

        pixels
            .par_chunks_mut(row_stride)
            .enumerate()
            .for_each(|(_row_idx, row)| {
                let mut rng = rand::rng();
                let noise_count = ((width as f64 * intensity_clamped as f64) as usize).max(1);

                for _ in 0..noise_count {
                    // Random grain size (1 to 8 pixels) - only horizontal
                    let x = rng.random_range(0usize..width);
                    let grain_w = rng.random_range(1usize..=8.min(width - x));
                    let gray_val = rng.random_range(min_brightness..=max_brightness);

                    // Draw noise grain (horizontal only within this row)
                    for dx in 0..grain_w {
                        let px = x + dx;
                        if px < width {
                            let pixel_idx = px * 4;
                            if pixel_idx + 4 <= row.len() {
                                row[pixel_idx] = gray_val;
                                row[pixel_idx + 1] = gray_val;
                                row[pixel_idx + 2] = gray_val;
                            }
                        }
                    }
                }
            });

        RgbaImage::from_raw(width as u32, height as u32, pixels)
    }
}
