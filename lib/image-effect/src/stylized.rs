use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, conv};

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct EdgeDetectionConfig {
    #[derivative(Default(value = "EdgeDetectionMode::Standard"))]
    mode: EdgeDetectionMode,
}

impl EdgeDetectionConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EdgeDetectionMode {
    Standard,
    SobelHorizontal,
    SobelVertical,
    SobelGlobal,
}

impl Effect for EdgeDetectionConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);

        match self.mode {
            EdgeDetectionMode::Standard => {
                conv::edge_detection(&mut photon_img);
            }
            EdgeDetectionMode::SobelHorizontal => {
                conv::sobel_horizontal(&mut photon_img);
            }
            EdgeDetectionMode::SobelVertical => {
                conv::sobel_vertical(&mut photon_img);
            }
            EdgeDetectionMode::SobelGlobal => {
                conv::sobel_global(&mut photon_img);
            }
        }

        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct EmbossConfig;

impl EmbossConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EmbossConfig {
    fn default() -> Self {
        Self
    }
}

impl Effect for EmbossConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        conv::emboss(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct SharpenConfig;

impl SharpenConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SharpenConfig {
    fn default() -> Self {
        Self
    }
}

impl Effect for SharpenConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        conv::sharpen(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        photon_rs::effects::pixelize(&mut photon_img, self.block_size as i32);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let levels = self.levels.max(2) as f32;
        let step = 255.0 / (levels - 1.0);

        let mut result = image.clone();

        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, y);
                let result_pixel = result.get_pixel_mut(x, y);
                // Posterize each channel
                result_pixel[0] = ((pixel[0] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
                result_pixel[1] = ((pixel[1] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
                result_pixel[2] = ((pixel[2] as f32 / step).round() * step).clamp(0.0, 255.0) as u8;
            }
        }

        Some(result)
    }
}
