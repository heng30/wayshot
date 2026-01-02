use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, effects};

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct BrightnessConfig {
    #[derivative(Default(value = "10"))]
    brightness: i32, // [-255, 255]
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
        effects::adjust_brightness(&mut photon_img, self.brightness as i16);
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
        effects::adjust_contrast(&mut photon_img, self.contrast);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Offset effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct OffsetConfig {
    #[derivative(Default(value = "0"))]
    channel_index: usize,
    #[derivative(Default(value = "30"))]
    offset: u32,
}

impl OffsetConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for OffsetConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::offset(&mut photon_img, self.channel_index, self.offset);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Offset red channel effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct OffsetRedConfig {
    #[derivative(Default(value = "30"))]
    offset_amt: u32,
}

impl OffsetRedConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for OffsetRedConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::offset_red(&mut photon_img, self.offset_amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Offset green channel effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct OffsetGreenConfig {
    #[derivative(Default(value = "30"))]
    offset_amt: u32,
}

impl OffsetGreenConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for OffsetGreenConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::offset_green(&mut photon_img, self.offset_amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Offset blue channel effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct OffsetBlueConfig {
    #[derivative(Default(value = "30"))]
    offset_amt: u32,
}

impl OffsetBlueConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for OffsetBlueConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::offset_blue(&mut photon_img, self.offset_amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Multiple offsets effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct MultipleOffsetsConfig {
    #[derivative(Default(value = "30"))]
    offset: u32,
    #[derivative(Default(value = "0"))]
    channel_index: usize,
    #[derivative(Default(value = "2"))]
    channel_index2: usize,
}

impl MultipleOffsetsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for MultipleOffsetsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::multiple_offsets(
            &mut photon_img,
            self.offset,
            self.channel_index,
            self.channel_index2,
        );
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

// ============================================================================
// Artistic Effects
// ============================================================================

/// Halftone effect configuration
#[derive(Debug, Clone, Copy)]
pub struct HalftoneConfig;

impl HalftoneConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for HalftoneConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::halftone(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Primary color effect configuration
#[derive(Debug, Clone, Copy)]
pub struct PrimaryConfig;

impl PrimaryConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for PrimaryConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::primary(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Colorize effect configuration
#[derive(Debug, Clone, Copy)]
pub struct ColorizeConfig;

impl ColorizeConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for ColorizeConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::colorize(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

// ============================================================================
// Brightness Effects
// ============================================================================

/// Increase brightness effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct IncBrightnessConfig {
    #[derivative(Default(value = "10"))]
    brightness: u8,
}

impl IncBrightnessConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for IncBrightnessConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::inc_brightness(&mut photon_img, self.brightness);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Decrease brightness effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DecBrightnessConfig {
    #[derivative(Default(value = "10"))]
    brightness: u8,
}

impl DecBrightnessConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DecBrightnessConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::dec_brightness(&mut photon_img, self.brightness);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

// ============================================================================
// Strips Effects
// ============================================================================

/// Horizontal strips effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HorizontalStripsConfig {
    #[derivative(Default(value = "8"))]
    num_strips: u8,
}

impl HorizontalStripsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HorizontalStripsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::horizontal_strips(&mut photon_img, self.num_strips);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Color horizontal strips effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ColorHorizontalStripsConfig {
    #[derivative(Default(value = "8"))]
    num_strips: u8,
    #[derivative(Default(value = "255"))]
    r: u8,
    #[derivative(Default(value = "0"))]
    g: u8,
    #[derivative(Default(value = "0"))]
    b: u8,
}

impl ColorHorizontalStripsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for ColorHorizontalStripsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        let color = photon_rs::Rgb::new(self.r, self.g, self.b);
        effects::color_horizontal_strips(&mut photon_img, self.num_strips, color);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Vertical strips effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct VerticalStripsConfig {
    #[derivative(Default(value = "8"))]
    num_strips: u8,
}

impl VerticalStripsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for VerticalStripsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::vertical_strips(&mut photon_img, self.num_strips);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Color vertical strips effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ColorVerticalStripsConfig {
    #[derivative(Default(value = "8"))]
    num_strips: u8,
    #[derivative(Default(value = "255"))]
    r: u8,
    #[derivative(Default(value = "0"))]
    g: u8,
    #[derivative(Default(value = "0"))]
    b: u8,
}

impl ColorVerticalStripsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for ColorVerticalStripsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        let color = photon_rs::Rgb::new(self.r, self.g, self.b);
        effects::color_vertical_strips(&mut photon_img, self.num_strips, color);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

// ============================================================================
// Special Effects
// ============================================================================

/// Oil painting effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct OilConfig {
    #[derivative(Default(value = "4"))]
    radius: i32,
    #[derivative(Default(value = "55.0"))]
    intensity: f64,
}

impl OilConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for OilConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::oil(&mut photon_img, self.radius, self.intensity);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Frosted glass effect configuration
#[derive(Debug, Clone, Copy)]
pub struct FrostedGlassConfig;

impl FrostedGlassConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for FrostedGlassConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::frosted_glass(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Normalize effect configuration
#[derive(Debug, Clone, Copy)]
pub struct NormalizeConfig;

impl NormalizeConfig {
    pub fn new() -> Self {
        Self
    }
}

impl Effect for NormalizeConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::normalize(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

/// Dither effect configuration
#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DitherConfig {
    #[derivative(Default(value = "1"))]
    depth: u32,
}

impl DitherConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DitherConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        effects::dither(&mut photon_img, self.depth);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}
