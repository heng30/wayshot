use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, channels, colour_spaces::hue_rotate_hsl, monochrome};

pub struct Invert;

impl Effect for Invert {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::invert(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
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

impl Effect for GrayscaleConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        match self.mode {
            GrayscaleMode::Luminance => {
                let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
                monochrome::grayscale_human_corrected(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::Average => {
                let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
                monochrome::grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::RedChannel => {
                let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
                monochrome::r_grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::GreenChannel => {
                let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
                monochrome::g_grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::BlueChannel => {
                let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
                monochrome::b_grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
        }
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
        photon_rs::effects::adjust_brightness(&mut photon_img, self.brightness as i16);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ContrastConfig {
    #[derivative(Default(value = "10.0"))]
    contrast: f32, // [-255.0, 255.0]
}

impl ContrastConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for ContrastConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
        photon_rs::effects::adjust_contrast(&mut photon_img, self.contrast);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
        photon_rs::colour_spaces::saturate_hsl(&mut photon_img, self.amount);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

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
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.to_vec(), width, height);
        hue_rotate_hsl(&mut photon_img, self.degrees as f32 / 360.0);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}
