use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, channels};

pub struct Invert;

impl Effect for Invert {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::invert(&mut photon_img);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AlterRedChannelConfig {
    #[derivative(Default(value = "20"))]
    amount: i16,
}

impl AlterRedChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for AlterRedChannelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::alter_red_channel(&mut photon_img, self.amount);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AlterGreenChannelConfig {
    #[derivative(Default(value = "20"))]
    amount: i16,
}

impl AlterGreenChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for AlterGreenChannelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::alter_green_channel(&mut photon_img, self.amount);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AlterBlueChannelConfig {
    #[derivative(Default(value = "20"))]
    amount: i16,
}

impl AlterBlueChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for AlterBlueChannelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::alter_blue_channel(&mut photon_img, self.amount);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AlterTwoChannelsConfig {
    #[derivative(Default(value = "1"))]
    channel1: usize,
    #[derivative(Default(value = "30"))]
    amt1: i16,
    #[derivative(Default(value = "2"))]
    channel2: usize,
    #[derivative(Default(value = "30"))]
    amt2: i16,
}

impl AlterTwoChannelsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for AlterTwoChannelsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::alter_two_channels(
            &mut photon_img,
            self.channel1.clamp(0, 2),
            self.amt1,
            self.channel2.clamp(0, 2),
            self.amt2,
        );
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AlterChannelsConfig {
    #[derivative(Default(value = "20"))]
    r_amt: i16,
    #[derivative(Default(value = "20"))]
    g_amt: i16,
    #[derivative(Default(value = "20"))]
    b_amt: i16,
}

impl AlterChannelsConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for AlterChannelsConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::alter_channels(&mut photon_img, self.r_amt, self.g_amt, self.b_amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct RemoveRedChannelConfig {
    #[derivative(Default(value = "255"))]
    min_filter: u8,
}

impl RemoveRedChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for RemoveRedChannelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::remove_red_channel(&mut photon_img, self.min_filter);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct RemoveGreenChannelConfig {
    #[derivative(Default(value = "255"))]
    min_filter: u8,
}

impl RemoveGreenChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for RemoveGreenChannelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::remove_green_channel(&mut photon_img, self.min_filter);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct RemoveBlueChannelConfig {
    #[derivative(Default(value = "255"))]
    min_filter: u8,
}

impl RemoveBlueChannelConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for RemoveBlueChannelConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        channels::remove_blue_channel(&mut photon_img, self.min_filter);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SelectiveHueRotateConfig {
    #[derivative(Default(value = "255"))]
    ref_r: u8,
    #[derivative(Default(value = "255"))]
    ref_g: u8,
    #[derivative(Default(value = "255"))]
    ref_b: u8,
    #[derivative(Default(value = "0.0"))]
    degrees: f32,
}

impl SelectiveHueRotateConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SelectiveHueRotateConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        let ref_color = photon_rs::Rgb::new(self.ref_r, self.ref_g, self.ref_b);
        channels::selective_hue_rotate(&mut photon_img, ref_color, self.degrees);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SelectiveLightenConfig {
    #[derivative(Default(value = "255"))]
    ref_r: u8,
    #[derivative(Default(value = "255"))]
    ref_g: u8,
    #[derivative(Default(value = "255"))]
    ref_b: u8,
    #[derivative(Default(value = "0.2"))]
    amt: f32, // [0, 1]
}

impl SelectiveLightenConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SelectiveLightenConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        let ref_color = photon_rs::Rgb::new(self.ref_r, self.ref_g, self.ref_b);
        channels::selective_lighten(&mut photon_img, ref_color, self.amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SelectiveDesaturateConfig {
    #[derivative(Default(value = "255"))]
    ref_r: u8,
    #[derivative(Default(value = "255"))]
    ref_g: u8,
    #[derivative(Default(value = "255"))]
    ref_b: u8,
    #[derivative(Default(value = "0.2"))]
    amt: f32, // [0, 1]
}

impl SelectiveDesaturateConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SelectiveDesaturateConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        let ref_color = photon_rs::Rgb::new(self.ref_r, self.ref_g, self.ref_b);
        channels::selective_desaturate(&mut photon_img, ref_color, self.amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SelectiveSaturateConfig {
    #[derivative(Default(value = "255"))]
    ref_r: u8,
    #[derivative(Default(value = "255"))]
    ref_g: u8,
    #[derivative(Default(value = "255"))]
    ref_b: u8,
    #[derivative(Default(value = "0.2"))]
    amt: f32, // [0, 1]
}

impl SelectiveSaturateConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SelectiveSaturateConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);
        let ref_color = photon_rs::Rgb::new(self.ref_r, self.ref_g, self.ref_b);
        channels::selective_saturate(&mut photon_img, ref_color, self.amt);
        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SelectiveGrayscaleConfig {
    #[derivative(Default(value = "255"))]
    ref_r: u8,
    #[derivative(Default(value = "255"))]
    ref_g: u8,
    #[derivative(Default(value = "255"))]
    ref_b: u8,
}

impl SelectiveGrayscaleConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Effect for SelectiveGrayscaleConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let raw_pixels = image.into_raw();
        let photon_img = PhotonImage::new(raw_pixels.clone(), width, height);
        let ref_color = photon_rs::Rgb::new(self.ref_r, self.ref_g, self.ref_b);
        channels::selective_greyscale(photon_img, ref_color);
        RgbaImage::from_raw(width, height, raw_pixels)
    }
}
