use std::path::Path;

use anyhow::{Result, anyhow};
use image_effect::{Effect, ImageEffect};
use serde_json::{Value, json};

use crate::util;

const FILTERS: &[&str] = &[
    "invert",
    "grayscale",
    "sepia",
    "blur",
    "box-blur",
    "median-blur",
    "brightness",
    "contrast",
    "saturation",
    "desaturate",
    "hue-rotate",
    "gamma",
    "vignette",
    "sharpen",
    "emboss",
    "edge-detect",
    "pixelate",
    "posterize",
    "threshold",
    "oil",
    "normalize",
    "warm",
    "cool",
    "tint",
    "duotone",
    "preset",
];
pub(super) fn list_filters() -> Value {
    json!({
        "filters": FILTERS,
        "presets": [
            "oceanic", "islands", "marine", "seagreen", "flagblue", "liquid", "diamante",
            "radio", "twenties", "rosetint", "mauve", "bluechrome", "vintage", "perfume",
            "serenity", "golden", "pastel-pink", "cali", "dramatic", "firenze", "obsidian", "lofi"
        ],
    })
}
pub(super) fn effect(
    input: &Path,
    output: &Path,
    name: &str,
    value: Option<f64>,
    value2: Option<f64>,
    value3: Option<f64>,
    text: Option<&str>,
) -> Result<Value> {
    use image_effect::{blur, colour_space, filter, monochrome, preset_filter, special, stylized};

    let v = value.unwrap_or(0.0);

    let effect = match name {
        "invert" => ImageEffect::Invert,
        "grayscale" => ImageEffect::Grayscale(monochrome::GrayscaleConfig::new()),
        "sepia" => ImageEffect::Sepia(filter::SepiaConfig::new().with_intensity(v as f32)),
        "blur" => ImageEffect::GaussianBlur(blur::GaussianBlurConfig::new().with_radius(v as i32)),
        "box-blur" => ImageEffect::BoxBlur(blur::BoxBlurConfig::new().with_radius(v as u32)),
        "median-blur" => {
            ImageEffect::MedianBlur(blur::MedianBlurConfig::new().with_radius(v as u32))
        }
        "brightness" => {
            ImageEffect::Brightness(special::BrightnessConfig::new().with_brightness(v as i32))
        }
        "contrast" => ImageEffect::Contrast(special::ContrastConfig::new().with_contrast(v as f32)),
        "saturation" => {
            ImageEffect::Saturation(colour_space::SaturationConfig::new().with_amount(v as f32))
        }
        "desaturate" => ImageEffect::DesaturateHsv(
            colour_space::DesaturateHsvConfig::new().with_level(v as f32),
        ),
        "hue-rotate" => {
            ImageEffect::HueRotate(colour_space::HueRotateConfig::new().with_degrees(v as i32))
        }
        "gamma" => ImageEffect::GammaCorrection(
            colour_space::GammaCorrectionConfig::new()
                .with_red(v as f32)
                .with_green(value2.unwrap_or(v) as f32)
                .with_blue(value3.unwrap_or(v) as f32),
        ),
        "vignette" => ImageEffect::Vignette(
            filter::VignetteConfig::new()
                .with_strength(value.unwrap_or(0.5) as f32)
                .with_radius(value2.unwrap_or(0.7) as f32),
        ),
        "sharpen" => ImageEffect::Sharpen(stylized::SharpenConfig::new()),
        "emboss" => ImageEffect::Emboss(stylized::EmbossConfig::new()),
        "edge-detect" => ImageEffect::EdgeDetection(stylized::EdgeDetectionConfig::new()),
        "pixelate" => {
            ImageEffect::Pixelate(stylized::PixelateConfig::new().with_block_size(v as u32))
        }
        "posterize" => {
            ImageEffect::Posterize(stylized::PosterizeConfig::new().with_levels(v as u8))
        }
        "threshold" => {
            ImageEffect::Threshold(monochrome::ThresholdConfig::new().with_threshold(v as u8))
        }
        "oil" => {
            ImageEffect::Oil(special::OilConfig::new().with_radius(value.unwrap_or(4.0) as i32))
        }
        "normalize" => ImageEffect::Normalize(special::NormalizeConfig::new()),
        "warm" => ImageEffect::WarmFilter(filter::TemperatureConfig::new().with_amount(v as f32)),
        "cool" => ImageEffect::CoolFilter(filter::TemperatureConfig::new().with_amount(v as f32)),
        "tint" => ImageEffect::ColorTint(
            filter::ColorTintConfig::new()
                .with_r(v as u8)
                .with_g(value2.unwrap_or(v) as u8)
                .with_b(value3.unwrap_or(v) as u8),
        ),
        "duotone" => ImageEffect::Duotone(
            monochrome::DuotoneConfig::new()
                .with_primary_r(v as u8)
                .with_primary_g(value2.unwrap_or(0.0) as u8)
                .with_primary_b(value3.unwrap_or(0.0) as u8),
        ),
        "preset" => {
            let preset =
                parse_preset(text.ok_or_else(|| anyhow!("preset 需要 --text 指定预设名"))?)?;
            ImageEffect::PresetFilter(preset_filter::PresetFilterConfig::new().with_filter(preset))
        }
        other => {
            return Err(anyhow!("未知特效: {other}，可用特效见 `image filters`"));
        }
    };

    let img = util::load_rgba(input)?;
    let img = effect
        .apply(img)
        .ok_or_else(|| anyhow!("特效执行失败: {name}"))?;
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn parse_preset(name: &str) -> Result<image_effect::preset_filter::PresetFilter> {
    use image_effect::preset_filter::PresetFilter::*;
    Ok(match name.to_lowercase().replace('_', "-").as_str() {
        "oceanic" => Oceanic,
        "islands" => Islands,
        "marine" => Marine,
        "seagreen" => Seagreen,
        "flagblue" => Flagblue,
        "liquid" => Liquid,
        "diamante" => Diamante,
        "radio" => Radio,
        "twenties" => Twenties,
        "rosetint" => Rosetint,
        "mauve" => Mauve,
        "bluechrome" => Bluechrome,
        "vintage" => Vintage,
        "perfume" => Perfume,
        "serenity" => Serenity,
        "golden" => Golden,
        "pastel-pink" => PastelPink,
        "cali" => Cali,
        "dramatic" => Dramatic,
        "firenze" => Firenze,
        "obsidian" => Obsidian,
        "lofi" => Lofi,
        other => return Err(anyhow!("未知预设: {other}")),
    })
}

pub(super) fn catalog() -> Value {
    list_filters()
}
