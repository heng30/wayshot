use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, Rgb, effects, monochrome};

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
                let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
                monochrome::grayscale_human_corrected(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::Average => {
                let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
                monochrome::grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::RedChannel => {
                let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
                monochrome::r_grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::GreenChannel => {
                let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
                monochrome::g_grayscale(&mut photon_img);
                RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
            }
            GrayscaleMode::BlueChannel => {
                let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
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
pub struct DuotoneConfig {
    #[derivative(Default(value = "0"))]
    primary_r: u8,
    #[derivative(Default(value = "0"))]
    primary_g: u8,
    #[derivative(Default(value = "255"))]
    primary_b: u8,
    #[derivative(Default(value = "128"))]
    secondary_r: u8,
    #[derivative(Default(value = "128"))]
    secondary_g: u8,
    #[derivative(Default(value = "128"))]
    secondary_b: u8,
}

impl DuotoneConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_primary_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            primary_r: r,
            primary_g: g,
            primary_b: b,
            ..Self::default()
        }
    }

    pub fn with_secondary_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        self.secondary_r = r;
        self.secondary_g = g;
        self.secondary_b = b;
        self
    }
}

impl Effect for DuotoneConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);

        let color_a = Rgb::new(self.primary_r, self.primary_g, self.primary_b);
        let color_b = Rgb::new(self.secondary_r, self.secondary_g, self.secondary_b);

        effects::duotone(&mut photon_img, color_a, color_b);

        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SolarizationMode {
    Red,
    Green,
    Blue,
    RG,
    RB,
    GB,
    RGB,
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SolarizationConfig {
    #[derivative(Default(value = "SolarizationMode::RGB"))]
    mode: SolarizationMode,
    #[derivative(Default(value = "128"))]
    threshold: u8,
}

impl SolarizationConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SolarizationConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        // photon-rs solarize only affects red channel with fixed threshold
        // For more control, we implement custom solarization
        let mut result = image.clone();

        for pixel in result.pixels_mut() {
            match self.mode {
                SolarizationMode::Red => {
                    if pixel[0] > self.threshold {
                        pixel[0] = 255 - pixel[0];
                    }
                }
                SolarizationMode::Green => {
                    if pixel[1] > self.threshold {
                        pixel[1] = 255 - pixel[1];
                    }
                }
                SolarizationMode::Blue => {
                    if pixel[2] > self.threshold {
                        pixel[2] = 255 - pixel[2];
                    }
                }
                SolarizationMode::RG => {
                    if pixel[0] > self.threshold {
                        pixel[0] = 255 - pixel[0];
                    }
                    if pixel[1] > self.threshold {
                        pixel[1] = 255 - pixel[1];
                    }
                }
                SolarizationMode::RB => {
                    if pixel[0] > self.threshold {
                        pixel[0] = 255 - pixel[0];
                    }
                    if pixel[2] > self.threshold {
                        pixel[2] = 255 - pixel[2];
                    }
                }
                SolarizationMode::GB => {
                    if pixel[1] > self.threshold {
                        pixel[1] = 255 - pixel[1];
                    }
                    if pixel[2] > self.threshold {
                        pixel[2] = 255 - pixel[2];
                    }
                }
                SolarizationMode::RGB => {
                    if pixel[0] > self.threshold {
                        pixel[0] = 255 - pixel[0];
                    }
                    if pixel[1] > self.threshold {
                        pixel[1] = 255 - pixel[1];
                    }
                    if pixel[2] > self.threshold {
                        pixel[2] = 255 - pixel[2];
                    }
                }
            }
        }
        Some(result)
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ThresholdConfig {
    #[derivative(Default(value = "128"))]
    threshold: u8,
}

impl ThresholdConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for ThresholdConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        monochrome::threshold(&mut photon_img, self.threshold as u32);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct LevelConfig {
    #[derivative(Default(value = "0"))]
    input_black: i32,
    #[derivative(Default(value = "255"))]
    input_white: i32,
    #[derivative(Default(value = "0"))]
    output_black: i32,
    #[derivative(Default(value = "255"))]
    output_white: i32,
}

impl LevelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for LevelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let mut result = image.clone();
        let input_range = (self.input_white - self.input_black) as f32;
        let output_range = (self.output_white - self.output_black) as f32;

        for pixel in result.pixels_mut() {
            for i in 0..3 {
                let val = pixel[i] as f32;
                let adjusted = ((val - self.input_black as f32) / input_range * output_range
                    + self.output_black as f32)
                    .clamp(0.0, 255.0) as u8;
                pixel[i] = adjusted;
            }
        }
        Some(result)
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ColorBalanceConfig {
    #[derivative(Default(value = "0"))]
    red_shift: i32,
    #[derivative(Default(value = "0"))]
    green_shift: i32,
    #[derivative(Default(value = "0"))]
    blue_shift: i32,
}

impl ColorBalanceConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for ColorBalanceConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let mut result = image.clone();

        for pixel in result.pixels_mut() {
            pixel[0] = (pixel[0] as i32 + self.red_shift).clamp(0, 255) as u8;
            pixel[1] = (pixel[1] as i32 + self.green_shift).clamp(0, 255) as u8;
            pixel[2] = (pixel[2] as i32 + self.blue_shift).clamp(0, 255) as u8;
        }
        Some(result)
    }
}
