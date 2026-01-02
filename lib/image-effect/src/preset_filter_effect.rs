use crate::Effect;
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;
use photon_rs::{PhotonImage, filters};

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct PresetFilterConfig {
    #[derivative(Default(value = "PresetFilter::Oceanic"))]
    filter: PresetFilter,
}

impl PresetFilterConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PresetFilter {
    Oceanic,
    Islands,
    Marine,
    Seagreen,
    Flagblue,
    Liquid,
    Diamante,
    Radio,
    Twenties,
    Rosetint,
    Mauve,
    Bluechrome,
    Vintage,
    Perfume,
    Serenity,
    Golden,
    PastelPink,
    Cali,
    Dramatic,
    Firenze,
    Obsidian,
    Lofi,
}

impl Effect for PresetFilterConfig {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        let (width, height) = (image.width(), image.height());
        let mut photon_img = PhotonImage::new(image.into_raw(), width, height);

        let filter_name = match self.filter {
            PresetFilter::Oceanic => "oceanic",
            PresetFilter::Islands => "islands",
            PresetFilter::Marine => "marine",
            PresetFilter::Seagreen => "seagreen",
            PresetFilter::Flagblue => "flagblue",
            PresetFilter::Liquid => "liquid",
            PresetFilter::Diamante => "diamante",
            PresetFilter::Radio => "radio",
            PresetFilter::Twenties => "twenties",
            PresetFilter::Rosetint => "rosetint",
            PresetFilter::Mauve => "mauve",
            PresetFilter::Bluechrome => "bluechrome",
            PresetFilter::Vintage => "vintage",
            PresetFilter::Perfume => "perfume",
            PresetFilter::Serenity => "serenity",
            PresetFilter::Golden => "golden",
            PresetFilter::PastelPink => "pastel_pink",
            PresetFilter::Cali => "cali",
            PresetFilter::Dramatic => "dramatic",
            PresetFilter::Firenze => "firenze",
            PresetFilter::Obsidian => "obsidian",
            PresetFilter::Lofi => "lofi",
        };

        filters::filter(&mut photon_img, filter_name);

        RgbaImage::from_raw(width, height, photon_img.get_raw_pixels())
    }
}
