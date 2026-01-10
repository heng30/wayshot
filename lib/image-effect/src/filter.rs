use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, effects, monochrome};

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
