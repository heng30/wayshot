pub mod base_effect;
pub mod blur_effect;
pub mod filter_effect;
pub mod monochrome_effect;
pub mod preset_filter_effect;
pub mod stylized_effect;

use image::RgbaImage;

pub type ImageEffectResult<T> = Result<T, ImageEffectError>;

#[derive(thiserror::Error, Debug)]
pub enum ImageEffectError {
    #[error("Photon error: {0}")]
    Photon(String),
    #[error("Image processing error: {0}")]
    ImageProc(String),
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
}

pub trait Effect {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()>;
}

#[derive(Debug, Clone)]
pub enum ImageEffect {
    // Base effects
    Grayscale(base_effect::GrayscaleConfig),
    Invert,
    Brightness(base_effect::BrightnessConfig),
    Contrast(base_effect::ContrastConfig),
    Saturation(base_effect::SaturationConfig),
    HueRotate(base_effect::HueRotateConfig),

    // Blur effects
    GaussianBlur(blur_effect::GaussianBlurConfig),
    BoxBlur(blur_effect::BoxBlurConfig),
    MedianBlur(blur_effect::MedianBlurConfig),

    // Filter effects
    Sepia(filter_effect::SepiaConfig),
    WarmFilter(filter_effect::TemperatureConfig),
    CoolFilter(filter_effect::TemperatureConfig),
    ColorTint(filter_effect::ColorTintConfig),
    Vignette(filter_effect::VignetteConfig),

    // Stylized effects
    EdgeDetection(stylized_effect::EdgeDetectionConfig),
    Emboss(stylized_effect::EmbossConfig),
    Sharpen(stylized_effect::SharpenConfig),
    Pixelate(stylized_effect::PixelateConfig),
    Posterize(stylized_effect::PosterizeConfig),

    // Preset filters (15 filters from photon-rs)
    PresetFilter(preset_filter_effect::PresetFilterConfig),

    // Monochrome effects
    Duotone(monochrome_effect::DuotoneConfig),
    Solarization(monochrome_effect::SolarizationConfig),
    Threshold(monochrome_effect::ThresholdConfig),
    Level(monochrome_effect::LevelConfig),
    ColorBalance(monochrome_effect::ColorBalanceConfig),
}

impl Effect for ImageEffect {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        match self {
            // Base effects
            ImageEffect::Grayscale(config) => config.apply(image),
            ImageEffect::Invert => base_effect::invert(image),
            ImageEffect::Brightness(config) => config.apply(image),
            ImageEffect::Contrast(config) => config.apply(image),
            ImageEffect::Saturation(config) => config.apply(image),
            ImageEffect::HueRotate(config) => config.apply(image),

            // Blur effects
            ImageEffect::GaussianBlur(config) => config.apply(image),
            ImageEffect::BoxBlur(config) => config.apply(image),
            ImageEffect::MedianBlur(config) => config.apply(image),

            // Filter effects
            ImageEffect::Sepia(config) => config.apply(image),
            ImageEffect::WarmFilter(config) => config.apply_warm(image),
            ImageEffect::CoolFilter(config) => config.apply_cool(image),
            ImageEffect::ColorTint(config) => config.apply(image),
            ImageEffect::Vignette(config) => config.apply(image),

            // Stylized effects
            ImageEffect::EdgeDetection(config) => config.apply(image),
            ImageEffect::Emboss(config) => config.apply(image),
            ImageEffect::Sharpen(config) => config.apply(image),
            ImageEffect::Pixelate(config) => config.apply(image),
            ImageEffect::Posterize(config) => config.apply(image),

            // Preset filters
            ImageEffect::PresetFilter(config) => config.apply(image),

            // Monochrome effects
            ImageEffect::Duotone(config) => config.apply(image),
            ImageEffect::Solarization(config) => config.apply(image),
            ImageEffect::Threshold(config) => config.apply(image),
            ImageEffect::Level(config) => config.apply(image),
            ImageEffect::ColorBalance(config) => config.apply(image),
        }
    }
}
