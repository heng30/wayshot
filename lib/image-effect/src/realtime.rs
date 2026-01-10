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
#[non_exhaustive]
#[repr(u8)]
pub enum RealTimeEffect {
    Grayscale = 0,
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

impl RealTimeEffect {
    pub fn name(&self) -> &'static str {
        match self {
            RealTimeEffect::Grayscale => "Grayscale",
            RealTimeEffect::Invert => "Invert",
            RealTimeEffect::Rosetint => "Rosetint",
            RealTimeEffect::Twenties => "Twenties",
            RealTimeEffect::Mauve => "Mauve",
            RealTimeEffect::Radio => "Radio",
            RealTimeEffect::Bluechrome => "Bluechrome",
            RealTimeEffect::Dramatic => "Dramatic",
            RealTimeEffect::PastelPink => "Pastel Pink",
            RealTimeEffect::Obsidian => "Obsidian",
            RealTimeEffect::Pixelate => "Pixelate",
            RealTimeEffect::Posterize => "Posterize",
            RealTimeEffect::Sepia => "Sepia",
            RealTimeEffect::Vignette => "Vignette",
            RealTimeEffect::Temperature => "Temperature",
        }
    }

    pub fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        match self {
            RealTimeEffect::Invert => Invert.apply(image),
            RealTimeEffect::Grayscale => GrayscaleConfig::new().apply(image),
            RealTimeEffect::Pixelate => PixelateConfig::new().with_block_size(8).apply(image),
            RealTimeEffect::Posterize => PosterizeConfig::new().with_levels(6).apply(image),
            RealTimeEffect::Sepia => SepiaConfig::new().with_intensity(0.3).apply(image),
            RealTimeEffect::Temperature => TemperatureConfig::new().with_amount(0.15).apply(image),

            RealTimeEffect::Vignette => VignetteConfig::new()
                .with_strength(0.25)
                .with_radius(0.55)
                .apply(image),

            RealTimeEffect::Rosetint => PresetFilterConfig::new()
                .with_filter(PresetFilter::Rosetint)
                .apply(image),

            RealTimeEffect::Twenties => PresetFilterConfig::new()
                .with_filter(PresetFilter::Twenties)
                .apply(image),

            RealTimeEffect::Mauve => PresetFilterConfig::new()
                .with_filter(PresetFilter::Mauve)
                .apply(image),

            RealTimeEffect::Radio => PresetFilterConfig::new()
                .with_filter(PresetFilter::Radio)
                .apply(image),

            RealTimeEffect::Bluechrome => PresetFilterConfig::new()
                .with_filter(PresetFilter::Bluechrome)
                .apply(image),

            RealTimeEffect::Dramatic => PresetFilterConfig::new()
                .with_filter(PresetFilter::Dramatic)
                .apply(image),

            RealTimeEffect::PastelPink => PresetFilterConfig::new()
                .with_filter(PresetFilter::PastelPink)
                .apply(image),

            RealTimeEffect::Obsidian => PresetFilterConfig::new()
                .with_filter(PresetFilter::Obsidian)
                .apply(image),
        }
    }

    pub fn all_effects() -> &'static [RealTimeEffect] {
        &[
            RealTimeEffect::Grayscale,
            RealTimeEffect::Invert,
            RealTimeEffect::Rosetint,
            RealTimeEffect::Twenties,
            RealTimeEffect::Mauve,
            RealTimeEffect::Radio,
            RealTimeEffect::Bluechrome,
            RealTimeEffect::Dramatic,
            RealTimeEffect::PastelPink,
            RealTimeEffect::Obsidian,
            RealTimeEffect::Pixelate,
            RealTimeEffect::Posterize,
            RealTimeEffect::Sepia,
            RealTimeEffect::Vignette,
            RealTimeEffect::Temperature,
        ]
    }
}
