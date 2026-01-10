use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, colour_spaces};

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SaturationConfig {
    #[derivative(Default(value = "0.0"))]
    amount: f32, // [0, 1]
}

impl SaturationConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SaturationConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::saturate_hsl(&mut photon_img, self.amount);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HueRotateConfig {
    #[derivative(Default(value = "0"))]
    degrees: i32, // [0, 360]
}

impl HueRotateConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HueRotateConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::hue_rotate_hsl(&mut photon_img, self.degrees as f32 / 360.0);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GammaCorrectionConfig {
    #[derivative(Default(value = "2.2"))]
    red: f32,
    #[derivative(Default(value = "2.2"))]
    green: f32,
    #[derivative(Default(value = "2.2"))]
    blue: f32,
}

impl GammaCorrectionConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for GammaCorrectionConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::gamma_correction(&mut photon_img, self.red, self.green, self.blue);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HueRotateHslConfig {
    #[derivative(Default(value = "0.0"))]
    degrees: f32,
}

impl HueRotateHslConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HueRotateHslConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::hue_rotate_hsl(&mut photon_img, self.degrees);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HueRotateHsvConfig {
    #[derivative(Default(value = "0.0"))]
    degrees: f32,
}

impl HueRotateHsvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HueRotateHsvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::hue_rotate_hsv(&mut photon_img, self.degrees);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HueRotateLchConfig {
    #[derivative(Default(value = "0.0"))]
    degrees: f32,
}

impl HueRotateLchConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HueRotateLchConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::hue_rotate_lch(&mut photon_img, self.degrees);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct HueRotateHsluvConfig {
    #[derivative(Default(value = "0.0"))]
    degrees: f32,
}

impl HueRotateHsluvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for HueRotateHsluvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::hue_rotate_hsluv(&mut photon_img, self.degrees);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SaturateLchConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl SaturateLchConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SaturateLchConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::saturate_lch(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SaturateHsluvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl SaturateHsluvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SaturateHsluvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::saturate_hsluv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SaturateHsvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl SaturateHsvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SaturateHsvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::saturate_hsv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct LightenLchConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl LightenLchConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for LightenLchConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::lighten_lch(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct LightenHsluvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl LightenHsluvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for LightenHsluvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::lighten_hsluv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct LightenHsvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl LightenHsvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for LightenHsvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::lighten_hsv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DarkenLchConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl DarkenLchConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DarkenLchConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::darken_lch(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DarkenHsluvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl DarkenHsluvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DarkenHsluvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::darken_hsluv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DarkenHsvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl DarkenHsvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DarkenHsvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::darken_hsv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DesaturateHsvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl DesaturateHsvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DesaturateHsvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::desaturate_hsv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DesaturateLchConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl DesaturateLchConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DesaturateLchConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::desaturate_lch(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct DesaturateHsluvConfig {
    #[derivative(Default(value = "0.1"))]
    level: f32,
}

impl DesaturateHsluvConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for DesaturateHsluvConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        colour_spaces::desaturate_hsluv(&mut photon_img, self.level);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}
