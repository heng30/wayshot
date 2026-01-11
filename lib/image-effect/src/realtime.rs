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
pub enum RealtimeImageEffect {
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

impl RealtimeImageEffect {
    pub fn name(&self) -> &'static str {
        match self {
            RealtimeImageEffect::None => "None",
            RealtimeImageEffect::Grayscale => "Grayscale",
            RealtimeImageEffect::Invert => "Invert",
            RealtimeImageEffect::Rosetint => "Rosetint",
            RealtimeImageEffect::Twenties => "Twenties",
            RealtimeImageEffect::Mauve => "Mauve",
            RealtimeImageEffect::Radio => "Radio",
            RealtimeImageEffect::Bluechrome => "Bluechrome",
            RealtimeImageEffect::Dramatic => "Dramatic",
            RealtimeImageEffect::PastelPink => "Pastel Pink",
            RealtimeImageEffect::Obsidian => "Obsidian",
            RealtimeImageEffect::Pixelate => "Pixelate",
            RealtimeImageEffect::Posterize => "Posterize",
            RealtimeImageEffect::Sepia => "Sepia",
            RealtimeImageEffect::Vignette => "Vignette",
            RealtimeImageEffect::Temperature => "Temperature",
        }
    }

    pub fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        match self {
            RealtimeImageEffect::None => None,
            RealtimeImageEffect::Invert => Invert.apply(image),
            RealtimeImageEffect::Grayscale => GrayscaleConfig::new().apply(image),
            RealtimeImageEffect::Pixelate => PixelateConfig::new().with_block_size(16).apply(image),
            RealtimeImageEffect::Posterize => PosterizeConfig::new().with_levels(6).apply(image),
            RealtimeImageEffect::Sepia => SepiaConfig::new().with_intensity(0.3).apply(image),

            RealtimeImageEffect::Temperature => {
                TemperatureConfig::new().with_amount(0.15).apply(image)
            }

            RealtimeImageEffect::Vignette => VignetteConfig::new()
                .with_strength(0.25)
                .with_radius(0.55)
                .apply(image),

            RealtimeImageEffect::Rosetint => PresetFilterConfig::new()
                .with_filter(PresetFilter::Rosetint)
                .apply(image),

            RealtimeImageEffect::Twenties => PresetFilterConfig::new()
                .with_filter(PresetFilter::Twenties)
                .apply(image),

            RealtimeImageEffect::Mauve => PresetFilterConfig::new()
                .with_filter(PresetFilter::Mauve)
                .apply(image),

            RealtimeImageEffect::Radio => PresetFilterConfig::new()
                .with_filter(PresetFilter::Radio)
                .apply(image),

            RealtimeImageEffect::Bluechrome => PresetFilterConfig::new()
                .with_filter(PresetFilter::Bluechrome)
                .apply(image),

            RealtimeImageEffect::Dramatic => PresetFilterConfig::new()
                .with_filter(PresetFilter::Dramatic)
                .apply(image),

            RealtimeImageEffect::PastelPink => PresetFilterConfig::new()
                .with_filter(PresetFilter::PastelPink)
                .apply(image),

            RealtimeImageEffect::Obsidian => PresetFilterConfig::new()
                .with_filter(PresetFilter::Obsidian)
                .apply(image),
        }
    }

    pub fn all_effects() -> &'static [RealtimeImageEffect] {
        &[
            RealtimeImageEffect::Grayscale,
            RealtimeImageEffect::Invert,
            RealtimeImageEffect::Rosetint,
            RealtimeImageEffect::Twenties,
            RealtimeImageEffect::Mauve,
            RealtimeImageEffect::Radio,
            RealtimeImageEffect::Bluechrome,
            RealtimeImageEffect::Dramatic,
            RealtimeImageEffect::PastelPink,
            RealtimeImageEffect::Obsidian,
            RealtimeImageEffect::Pixelate,
            RealtimeImageEffect::Posterize,
            RealtimeImageEffect::Sepia,
            RealtimeImageEffect::Vignette,
            RealtimeImageEffect::Temperature,
        ]
    }
}
