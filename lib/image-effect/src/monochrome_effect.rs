use crate::Effect;
use image::RgbaImage;
use photon_rs::{effects, monochrome, Rgb, PhotonImage};

/// Duotone effect configuration
#[derive(Debug, Clone, Copy)]
pub struct DuotoneConfig {
    pub primary_r: u8,
    pub primary_g: u8,
    pub primary_b: u8,
    pub secondary_r: u8,
    pub secondary_g: u8,
    pub secondary_b: u8,
}

impl Default for DuotoneConfig {
    fn default() -> Self {
        Self {
            primary_r: 0,
            primary_g: 0,
            primary_b: 255,
            secondary_r: 128,
            secondary_g: 128,
            secondary_b: 128,
        }
    }
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
        let mut photon_img = PhotonImage::new(image.to_vec(), image.width(), image.height());

        let color_a = Rgb::new(self.primary_r, self.primary_g, self.primary_b);
        let color_b = Rgb::new(self.secondary_r, self.secondary_g, self.secondary_b);

        effects::duotone(&mut photon_img, color_a, color_b);

        RgbaImage::from_raw(image.width(), image.height(), photon_img.get_raw_pixels())
    }
}

/// Solarization effect configuration
#[derive(Debug, Clone, Copy)]
pub struct SolarizationConfig {
    pub mode: SolarizationMode,
    pub threshold: u8,
}

impl Default for SolarizationConfig {
    fn default() -> Self {
        Self {
            mode: SolarizationMode::RGB,
            threshold: 128,
        }
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

impl SolarizationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mode(mut self, mode: SolarizationMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_threshold(mut self, threshold: u8) -> Self {
        self.threshold = threshold;
        self
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
                    if pixel[0] > self.threshold { pixel[0] = 255 - pixel[0]; }
                    if pixel[1] > self.threshold { pixel[1] = 255 - pixel[1]; }
                }
                SolarizationMode::RB => {
                    if pixel[0] > self.threshold { pixel[0] = 255 - pixel[0]; }
                    if pixel[2] > self.threshold { pixel[2] = 255 - pixel[2]; }
                }
                SolarizationMode::GB => {
                    if pixel[1] > self.threshold { pixel[1] = 255 - pixel[1]; }
                    if pixel[2] > self.threshold { pixel[2] = 255 - pixel[2]; }
                }
                SolarizationMode::RGB => {
                    if pixel[0] > self.threshold { pixel[0] = 255 - pixel[0]; }
                    if pixel[1] > self.threshold { pixel[1] = 255 - pixel[1]; }
                    if pixel[2] > self.threshold { pixel[2] = 255 - pixel[2]; }
                }
            }
        }
        Some(result)
    }
}

/// Threshold effect configuration
#[derive(Debug, Clone, Copy)]
pub struct ThresholdConfig {
    pub threshold: u8,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self { threshold: 128 }
    }
}

impl ThresholdConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, threshold: u8) -> Self {
        self.threshold = threshold;
        self
    }
}

impl Effect for ThresholdConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let mut photon_img = PhotonImage::new(image.to_vec(), image.width(), image.height());
        monochrome::threshold(&mut photon_img, self.threshold as u32);
        RgbaImage::from_raw(image.width(), image.height(), photon_img.get_raw_pixels())
    }
}

/// Level adjustment configuration
#[derive(Debug, Clone, Copy)]
pub struct LevelConfig {
    pub input_black: i32,
    pub input_white: i32,
    pub output_black: i32,
    pub output_white: i32,
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self {
            input_black: 0,
            input_white: 255,
            output_black: 0,
            output_white: 255,
        }
    }
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
                let adjusted = ((val - self.input_black as f32) / input_range * output_range + self.output_black as f32).clamp(0.0, 255.0) as u8;
                pixel[i] = adjusted;
            }
        }
        Some(result)
    }
}

/// Color balance configuration
#[derive(Debug, Clone, Copy)]
pub struct ColorBalanceConfig {
    pub red_shift: i32,
    pub green_shift: i32,
    pub blue_shift: i32,
}

impl Default for ColorBalanceConfig {
    fn default() -> Self {
        Self {
            red_shift: 0,
            green_shift: 0,
            blue_shift: 0,
        }
    }
}

impl ColorBalanceConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_red_shift(mut self, shift: i32) -> Self {
        self.red_shift = shift;
        self
    }

    pub fn with_green_shift(mut self, shift: i32) -> Self {
        self.green_shift = shift;
        self
    }

    pub fn with_blue_shift(mut self, shift: i32) -> Self {
        self.blue_shift = shift;
        self
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
