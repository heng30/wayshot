pub mod base_effect;
pub mod blur_effect;
pub mod channel_effect;
pub mod colour_space_effect;
pub mod filter_effect;
pub mod monochrome_effect;
pub mod noise_effect;
pub mod preset_filter_effect;
pub mod special_effect;
pub mod stylized_effect;

use image::RgbaImage;

pub trait Effect {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage>;
}

#[derive(Debug, Clone)]
pub enum ImageEffect {
    // Base effects
    Invert,
    Grayscale(base_effect::GrayscaleConfig),
    Brightness(base_effect::BrightnessConfig),
    Contrast(base_effect::ContrastConfig),
    Saturation(base_effect::SaturationConfig),
    HueRotate(base_effect::HueRotateConfig),

    // Blur effects
    GaussianBlur(blur_effect::GaussianBlurConfig),
    BoxBlur(blur_effect::BoxBlurConfig),
    MedianBlur(blur_effect::MedianBlurConfig),

    // Noise effects
    GaussianNoise(noise_effect::GaussianNoiseConfig),
    PinkNoise(noise_effect::PinkNoiseConfig),

    // Channel effects
    AlterRedChannel(channel_effect::AlterRedChannelConfig),
    AlterGreenChannel(channel_effect::AlterGreenChannelConfig),
    AlterBlueChannel(channel_effect::AlterBlueChannelConfig),
    AlterTwoChannels(channel_effect::AlterTwoChannelsConfig),
    AlterChannels(channel_effect::AlterChannelsConfig),
    RemoveRedChannel(channel_effect::RemoveRedChannelConfig),
    RemoveGreenChannel(channel_effect::RemoveGreenChannelConfig),
    RemoveBlueChannel(channel_effect::RemoveBlueChannelConfig),
    SelectiveHueRotate(channel_effect::SelectiveHueRotateConfig),
    SelectiveLighten(channel_effect::SelectiveLightenConfig),
    SelectiveDesaturate(channel_effect::SelectiveDesaturateConfig),
    SelectiveSaturate(channel_effect::SelectiveSaturateConfig),
    SelectiveGrayscale(channel_effect::SelectiveGrayscaleConfig),

    // Colour space effects
    GammaCorrection(colour_space_effect::GammaCorrectionConfig),
    HueRotateHsl(colour_space_effect::HueRotateHslConfig),
    HueRotateHsv(colour_space_effect::HueRotateHsvConfig),
    HueRotateLch(colour_space_effect::HueRotateLchConfig),
    HueRotateHsluv(colour_space_effect::HueRotateHsluvConfig),
    SaturateLch(colour_space_effect::SaturateLchConfig),
    SaturateHsluv(colour_space_effect::SaturateHsluvConfig),
    SaturateHsv(colour_space_effect::SaturateHsvConfig),
    LightenLch(colour_space_effect::LightenLchConfig),
    LightenHsluv(colour_space_effect::LightenHsluvConfig),
    LightenHsv(colour_space_effect::LightenHsvConfig),
    DarkenLch(colour_space_effect::DarkenLchConfig),
    DarkenHsluv(colour_space_effect::DarkenHsluvConfig),
    DarkenHsv(colour_space_effect::DarkenHsvConfig),
    DesaturateHsv(colour_space_effect::DesaturateHsvConfig),
    DesaturateLch(colour_space_effect::DesaturateLchConfig),
    DesaturateHsluv(colour_space_effect::DesaturateHsluvConfig),

    // Special effects
    Offset(special_effect::OffsetConfig),
    OffsetRed(special_effect::OffsetRedConfig),
    OffsetGreen(special_effect::OffsetGreenConfig),
    OffsetBlue(special_effect::OffsetBlueConfig),
    MultipleOffsets(special_effect::MultipleOffsetsConfig),
    Halftone(special_effect::HalftoneConfig),
    Primary(special_effect::PrimaryConfig),
    Colorize(special_effect::ColorizeConfig),
    IncBrightness(special_effect::IncBrightnessConfig),
    DecBrightness(special_effect::DecBrightnessConfig),
    HorizontalStrips(special_effect::HorizontalStripsConfig),
    ColorHorizontalStrips(special_effect::ColorHorizontalStripsConfig),
    VerticalStrips(special_effect::VerticalStripsConfig),
    ColorVerticalStrips(special_effect::ColorVerticalStripsConfig),
    Oil(special_effect::OilConfig),
    FrostedGlass(special_effect::FrostedGlassConfig),
    Normalize(special_effect::NormalizeConfig),
    Dither(special_effect::DitherConfig),

    // Filter effects
    Sepia(filter_effect::SepiaConfig),
    WarmFilter(filter_effect::TemperatureConfig),
    CoolFilter(filter_effect::TemperatureConfig),
    ColorTint(filter_effect::ColorTintConfig),
    Vignette(filter_effect::VignetteConfig),

    // Preset filters (15 filters from photon-rs)
    PresetFilter(preset_filter_effect::PresetFilterConfig),

    // Stylized effects
    EdgeDetection(stylized_effect::EdgeDetectionConfig),
    Emboss(stylized_effect::EmbossConfig),
    Sharpen(stylized_effect::SharpenConfig),
    Pixelate(stylized_effect::PixelateConfig),
    Posterize(stylized_effect::PosterizeConfig),

    // Monochrome effects
    Duotone(monochrome_effect::DuotoneConfig),
    Solarization(monochrome_effect::SolarizationConfig),
    Threshold(monochrome_effect::ThresholdConfig),
    Level(monochrome_effect::LevelConfig),
    ColorBalance(monochrome_effect::ColorBalanceConfig),
}

impl Effect for ImageEffect {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        match self {
            // Base effects
            ImageEffect::Invert => base_effect::Invert.apply(image),
            ImageEffect::Grayscale(config) => config.apply(image),
            ImageEffect::Brightness(config) => config.apply(image),
            ImageEffect::Contrast(config) => config.apply(image),
            ImageEffect::Saturation(config) => config.apply(image),
            ImageEffect::HueRotate(config) => config.apply(image),

            // Blur effects
            ImageEffect::GaussianBlur(config) => config.apply(image),
            ImageEffect::BoxBlur(config) => config.apply(image),
            ImageEffect::MedianBlur(config) => config.apply(image),

            // Noise effects
            ImageEffect::GaussianNoise(config) => config.apply(image),
            ImageEffect::PinkNoise(config) => config.apply(image),

            // Channel effects
            ImageEffect::AlterRedChannel(config) => config.apply(image),
            ImageEffect::AlterGreenChannel(config) => config.apply(image),
            ImageEffect::AlterBlueChannel(config) => config.apply(image),
            ImageEffect::AlterTwoChannels(config) => config.apply(image),
            ImageEffect::AlterChannels(config) => config.apply(image),
            ImageEffect::RemoveRedChannel(config) => config.apply(image),
            ImageEffect::RemoveGreenChannel(config) => config.apply(image),
            ImageEffect::RemoveBlueChannel(config) => config.apply(image),
            ImageEffect::SelectiveHueRotate(config) => config.apply(image),
            ImageEffect::SelectiveLighten(config) => config.apply(image),
            ImageEffect::SelectiveDesaturate(config) => config.apply(image),
            ImageEffect::SelectiveSaturate(config) => config.apply(image),
            ImageEffect::SelectiveGrayscale(config) => config.apply(image),

            // Colour space effects
            ImageEffect::GammaCorrection(config) => config.apply(image),
            ImageEffect::HueRotateHsl(config) => config.apply(image),
            ImageEffect::HueRotateHsv(config) => config.apply(image),
            ImageEffect::HueRotateLch(config) => config.apply(image),
            ImageEffect::HueRotateHsluv(config) => config.apply(image),
            ImageEffect::SaturateLch(config) => config.apply(image),
            ImageEffect::SaturateHsluv(config) => config.apply(image),
            ImageEffect::SaturateHsv(config) => config.apply(image),
            ImageEffect::LightenLch(config) => config.apply(image),
            ImageEffect::LightenHsluv(config) => config.apply(image),
            ImageEffect::LightenHsv(config) => config.apply(image),
            ImageEffect::DarkenLch(config) => config.apply(image),
            ImageEffect::DarkenHsluv(config) => config.apply(image),
            ImageEffect::DarkenHsv(config) => config.apply(image),
            ImageEffect::DesaturateHsv(config) => config.apply(image),
            ImageEffect::DesaturateLch(config) => config.apply(image),
            ImageEffect::DesaturateHsluv(config) => config.apply(image),

            // Special effects
            ImageEffect::Offset(config) => config.apply(image),
            ImageEffect::OffsetRed(config) => config.apply(image),
            ImageEffect::OffsetGreen(config) => config.apply(image),
            ImageEffect::OffsetBlue(config) => config.apply(image),
            ImageEffect::MultipleOffsets(config) => config.apply(image),
            ImageEffect::Halftone(config) => config.apply(image),
            ImageEffect::Primary(config) => config.apply(image),
            ImageEffect::Colorize(config) => config.apply(image),
            ImageEffect::IncBrightness(config) => config.apply(image),
            ImageEffect::DecBrightness(config) => config.apply(image),
            ImageEffect::HorizontalStrips(config) => config.apply(image),
            ImageEffect::ColorHorizontalStrips(config) => config.apply(image),
            ImageEffect::VerticalStrips(config) => config.apply(image),
            ImageEffect::ColorVerticalStrips(config) => config.apply(image),
            ImageEffect::Oil(config) => config.apply(image),
            ImageEffect::FrostedGlass(config) => config.apply(image),
            ImageEffect::Normalize(config) => config.apply(image),
            ImageEffect::Dither(config) => config.apply(image),

            // Filter effects
            ImageEffect::Sepia(config) => config.apply(image),
            ImageEffect::WarmFilter(config) => config.apply(image),
            ImageEffect::CoolFilter(config) => config.apply(image),
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
