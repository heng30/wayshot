use crate::Effect;
use image::RgbaImage;
use photon_rs::{PhotonImage, noise};

#[derive(Debug, Clone, Copy)]
pub struct GaussianNoiseConfig;

impl GaussianNoiseConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for GaussianNoiseConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        noise::add_noise_rand(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PinkNoiseConfig;

impl PinkNoiseConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for PinkNoiseConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        noise::pink_noise(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}
