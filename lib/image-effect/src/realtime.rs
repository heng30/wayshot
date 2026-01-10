use crate::{
    Effect,
    channel::Invert,
    filter::{SepiaConfig, TemperatureConfig, VignetteConfig},
    monochrome::GrayscaleConfig,
    preset_filter::{PresetFilter, PresetFilterConfig},
    stylized::{PixelateConfig, PosterizeConfig},
};
use image::RgbaImage;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum RealTimeImageEffect {
    None = 0,
    Grayscale,
    Invert,
    Rosetint,
    Twenties,
    Mauve,
    Radio,
    Bluechrome,
    Dramatic,
    PastelPink,
    Obsidian,
    Pixelate,
    Posterize,
    Sepia,
    Vignette,
    Temperature,
}

impl RealTimeImageEffect {
    pub fn name(&self) -> &'static str {
        match self {
            RealTimeImageEffect::None => "None",
            RealTimeImageEffect::Grayscale => "Grayscale",
            RealTimeImageEffect::Invert => "Invert",
            RealTimeImageEffect::Rosetint => "Rosetint",
            RealTimeImageEffect::Twenties => "Twenties",
            RealTimeImageEffect::Mauve => "Mauve",
            RealTimeImageEffect::Radio => "Radio",
            RealTimeImageEffect::Bluechrome => "Bluechrome",
            RealTimeImageEffect::Dramatic => "Dramatic",
            RealTimeImageEffect::PastelPink => "Pastel Pink",
            RealTimeImageEffect::Obsidian => "Obsidian",
            RealTimeImageEffect::Pixelate => "Pixelate",
            RealTimeImageEffect::Posterize => "Posterize",
            RealTimeImageEffect::Sepia => "Sepia",
            RealTimeImageEffect::Vignette => "Vignette",
            RealTimeImageEffect::Temperature => "Temperature",
        }
    }

    pub fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        match self {
            RealTimeImageEffect::None => None,
            RealTimeImageEffect::Invert => Invert.apply(image),
            RealTimeImageEffect::Grayscale => GrayscaleConfig::new().apply(image),
            RealTimeImageEffect::Pixelate => PixelateConfig::new().with_block_size(8).apply(image),
            RealTimeImageEffect::Posterize => PosterizeConfig::new().with_levels(6).apply(image),
            RealTimeImageEffect::Sepia => SepiaConfig::new().with_intensity(0.3).apply(image),

            RealTimeImageEffect::Temperature => {
                TemperatureConfig::new().with_amount(0.15).apply(image)
            }

            RealTimeImageEffect::Vignette => VignetteConfig::new()
                .with_strength(0.25)
                .with_radius(0.55)
                .apply(image),

            RealTimeImageEffect::Rosetint => PresetFilterConfig::new()
                .with_filter(PresetFilter::Rosetint)
                .apply(image),

            RealTimeImageEffect::Twenties => PresetFilterConfig::new()
                .with_filter(PresetFilter::Twenties)
                .apply(image),

            RealTimeImageEffect::Mauve => PresetFilterConfig::new()
                .with_filter(PresetFilter::Mauve)
                .apply(image),

            RealTimeImageEffect::Radio => PresetFilterConfig::new()
                .with_filter(PresetFilter::Radio)
                .apply(image),

            RealTimeImageEffect::Bluechrome => PresetFilterConfig::new()
                .with_filter(PresetFilter::Bluechrome)
                .apply(image),

            RealTimeImageEffect::Dramatic => PresetFilterConfig::new()
                .with_filter(PresetFilter::Dramatic)
                .apply(image),

            RealTimeImageEffect::PastelPink => PresetFilterConfig::new()
                .with_filter(PresetFilter::PastelPink)
                .apply(image),

            RealTimeImageEffect::Obsidian => PresetFilterConfig::new()
                .with_filter(PresetFilter::Obsidian)
                .apply(image),
        }
    }

    pub fn all_effects() -> &'static [RealTimeImageEffect] {
        &[
            RealTimeImageEffect::Grayscale,
            RealTimeImageEffect::Invert,
            RealTimeImageEffect::Rosetint,
            RealTimeImageEffect::Twenties,
            RealTimeImageEffect::Mauve,
            RealTimeImageEffect::Radio,
            RealTimeImageEffect::Bluechrome,
            RealTimeImageEffect::Dramatic,
            RealTimeImageEffect::PastelPink,
            RealTimeImageEffect::Obsidian,
            RealTimeImageEffect::Pixelate,
            RealTimeImageEffect::Posterize,
            RealTimeImageEffect::Sepia,
            RealTimeImageEffect::Vignette,
            RealTimeImageEffect::Temperature,
        ]
    }
}
