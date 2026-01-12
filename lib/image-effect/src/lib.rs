pub mod blur;
pub mod channel;
pub mod colour_space;
pub mod filter;
pub mod monochrome;
pub mod noise;
pub mod preset_filter;
pub mod realtime;
pub mod special;
pub mod stylized;

use image::RgbaImage;

pub trait Effect {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage>;
}

#[derive(Debug, Clone)]
pub enum ImageEffect {
    // Blur effects
    GaussianBlur(blur::GaussianBlurConfig),
    BoxBlur(blur::BoxBlurConfig),
    MedianBlur(blur::MedianBlurConfig),

    // Noise effects
    GaussianNoise(noise::GaussianNoiseConfig),
    PinkNoise(noise::PinkNoiseConfig),
    SnowNoise(filter::SnowNoiseConfig),

    // Channel effects
    Invert,
    AlterRedChannel(channel::AlterRedChannelConfig),
    AlterGreenChannel(channel::AlterGreenChannelConfig),
    AlterBlueChannel(channel::AlterBlueChannelConfig),
    AlterTwoChannels(channel::AlterTwoChannelsConfig),
    AlterChannels(channel::AlterChannelsConfig),
    RemoveRedChannel(channel::RemoveRedChannelConfig),
    RemoveGreenChannel(channel::RemoveGreenChannelConfig),
    RemoveBlueChannel(channel::RemoveBlueChannelConfig),
    SelectiveHueRotate(channel::SelectiveHueRotateConfig),
    SelectiveLighten(channel::SelectiveLightenConfig),
    SelectiveDesaturate(channel::SelectiveDesaturateConfig),
    SelectiveSaturate(channel::SelectiveSaturateConfig),
    SelectiveGrayscale(channel::SelectiveGrayscaleConfig),

    // Colour space effects
    Saturation(colour_space::SaturationConfig),
    HueRotate(colour_space::HueRotateConfig),
    GammaCorrection(colour_space::GammaCorrectionConfig),
    HueRotateHsl(colour_space::HueRotateHslConfig),
    HueRotateHsv(colour_space::HueRotateHsvConfig),
    HueRotateLch(colour_space::HueRotateLchConfig),
    HueRotateHsluv(colour_space::HueRotateHsluvConfig),
    SaturateLch(colour_space::SaturateLchConfig),
    SaturateHsluv(colour_space::SaturateHsluvConfig),
    SaturateHsv(colour_space::SaturateHsvConfig),
    LightenLch(colour_space::LightenLchConfig),
    LightenHsluv(colour_space::LightenHsluvConfig),
    LightenHsv(colour_space::LightenHsvConfig),
    DarkenLch(colour_space::DarkenLchConfig),
    DarkenHsluv(colour_space::DarkenHsluvConfig),
    DarkenHsv(colour_space::DarkenHsvConfig),
    DesaturateHsv(colour_space::DesaturateHsvConfig),
    DesaturateLch(colour_space::DesaturateLchConfig),
    DesaturateHsluv(colour_space::DesaturateHsluvConfig),

    // Special effects
    Brightness(special::BrightnessConfig),
    Contrast(special::ContrastConfig),
    Offset(special::OffsetConfig),
    OffsetRed(special::OffsetRedConfig),
    OffsetGreen(special::OffsetGreenConfig),
    OffsetBlue(special::OffsetBlueConfig),
    MultipleOffsets(special::MultipleOffsetsConfig),
    Halftone(special::HalftoneConfig),
    Primary(special::PrimaryConfig),
    Colorize(special::ColorizeConfig),
    IncBrightness(special::IncBrightnessConfig),
    DecBrightness(special::DecBrightnessConfig),
    HorizontalStrips(special::HorizontalStripsConfig),
    ColorHorizontalStrips(special::ColorHorizontalStripsConfig),
    VerticalStrips(special::VerticalStripsConfig),
    ColorVerticalStrips(special::ColorVerticalStripsConfig),
    Oil(special::OilConfig),
    FrostedGlass(special::FrostedGlassConfig),
    Normalize(special::NormalizeConfig),
    Dither(special::DitherConfig),

    // Preset filters (15 filters from photon-rs)
    PresetFilter(preset_filter::PresetFilterConfig),

    // Filter effects
    Sepia(filter::SepiaConfig),
    WarmFilter(filter::TemperatureConfig),
    CoolFilter(filter::TemperatureConfig),
    ColorTint(filter::ColorTintConfig),
    Vignette(filter::VignetteConfig),

    // Stylized effects
    EdgeDetection(stylized::EdgeDetectionConfig),
    Emboss(stylized::EmbossConfig),
    Sharpen(stylized::SharpenConfig),
    Pixelate(stylized::PixelateConfig),
    Posterize(stylized::PosterizeConfig),

    // Monochrome effects
    Grayscale(monochrome::GrayscaleConfig),
    Duotone(monochrome::DuotoneConfig),
    Solarization(monochrome::SolarizationConfig),
    Threshold(monochrome::ThresholdConfig),
    Level(monochrome::LevelConfig),
    ColorBalance(monochrome::ColorBalanceConfig),
}

impl Effect for ImageEffect {
    fn apply(&self, image: RgbaImage) -> Option<RgbaImage> {
        match self {
            // Blur effects
            ImageEffect::GaussianBlur(config) => config.apply(image),
            ImageEffect::BoxBlur(config) => config.apply(image),
            ImageEffect::MedianBlur(config) => config.apply(image),

            // Noise effects
            ImageEffect::GaussianNoise(config) => config.apply(image),
            ImageEffect::PinkNoise(config) => config.apply(image),
            ImageEffect::SnowNoise(config) => config.apply(image),

            // Channel effects
            ImageEffect::Invert => channel::Invert.apply(image),
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
            ImageEffect::Saturation(config) => config.apply(image),
            ImageEffect::HueRotate(config) => config.apply(image),
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
            ImageEffect::Brightness(config) => config.apply(image),
            ImageEffect::Contrast(config) => config.apply(image),
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
            ImageEffect::Grayscale(config) => config.apply(image),
            ImageEffect::Duotone(config) => config.apply(image),
            ImageEffect::Solarization(config) => config.apply(image),
            ImageEffect::Threshold(config) => config.apply(image),
            ImageEffect::Level(config) => config.apply(image),
            ImageEffect::ColorBalance(config) => config.apply(image),
        }
    }
}
