use crate::{Effect, ImageEffectResult};
use derivative::Derivative;
use derive_setters::Setters;
use image::RgbaImage;

/// Preset filter configuration
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
}

impl Effect for PresetFilterConfig {
    fn apply(&self, image: &mut RgbaImage) -> ImageEffectResult<()> {
        match self.filter {
            PresetFilter::Oceanic => apply_oceanic(image),
            PresetFilter::Islands => apply_islands(image),
            PresetFilter::Marine => apply_marine(image),
            PresetFilter::Seagreen => apply_seagreen(image),
            PresetFilter::Flagblue => apply_flagblue(image),
            PresetFilter::Liquid => apply_liquid(image),
            PresetFilter::Diamante => apply_diamante(image),
            PresetFilter::Radio => apply_radio(image),
            PresetFilter::Twenties => apply_twenties(image),
            PresetFilter::Rosetint => apply_rosetint(image),
            PresetFilter::Mauve => apply_mauve(image),
            PresetFilter::Bluechrome => apply_bluechrome(image),
            PresetFilter::Vintage => apply_vintage(image),
            PresetFilter::Perfume => apply_perfume(image),
            PresetFilter::Serenity => apply_serenity(image),
        }
    }
}

// Preset filter implementations

fn apply_oceanic(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Aquamarine-tinted hue
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.9).max(0.0) as u8; // Reduce red
        pixel[1] = (pixel[1] as f32 * 1.05).min(255.0) as u8; // Slight increase green
        pixel[2] = (pixel[2] as f32 * 1.2).min(255.0) as u8; // Increase blue
    }
    Ok(())
}

fn apply_islands(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Aquamarine tint
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.85).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 1.1).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.15).min(255.0) as u8;
    }
    Ok(())
}

fn apply_marine(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Green/blue mixed hue
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.8).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 1.1).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.1).min(255.0) as u8;
    }
    Ok(())
}

fn apply_seagreen(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Dark green hue with blue tones
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.75).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 1.15).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.05).min(255.0) as u8;
    }
    Ok(())
}

fn apply_flagblue(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Royal blue tint
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.7).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 0.9).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.2).min(255.0) as u8;
    }
    Ok(())
}

fn apply_liquid(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Blue-inspired tint
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.8).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 0.95).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.15).min(255.0) as u8;
    }
    Ok(())
}

fn apply_diamante(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Blue/turquoise tint
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.85).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 1.0).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.2).min(255.0) as u8;
    }
    Ok(())
}

fn apply_radio(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Fallout-style radio effect (greenish with decreased contrast)
    for pixel in image.pixels_mut() {
        let gray = ((pixel[0] as i32 * 3 + pixel[1] as i32 * 4 + pixel[2] as i32) / 8) as u8;
        pixel[0] = (gray as i32 - 20).max(0) as u8;
        pixel[1] = (gray as i32 + 10).min(255) as u8;
        pixel[2] = (gray as i32 - 30).max(0) as u8;
    }
    Ok(())
}

fn apply_twenties(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Slight-blue tinted historical effect
    for pixel in image.pixels_mut() {
        // Add slight blue tint
        pixel[0] = (pixel[0] as f32 * 0.95).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 0.98).max(0.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.05).min(255.0) as u8;

        // Decrease contrast slightly
        pixel[0] = ((pixel[0] as f32 - 128.0) * 0.9 + 128.0).clamp(0.0, 255.0) as u8;
        pixel[1] = ((pixel[1] as f32 - 128.0) * 0.9 + 128.0).clamp(0.0, 255.0) as u8;
        pixel[2] = ((pixel[2] as f32 - 128.0) * 0.9 + 128.0).clamp(0.0, 255.0) as u8;
    }
    Ok(())
}

fn apply_rosetint(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Rose-tinted filter
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 1.15).min(255.0) as u8; // Increase red
        pixel[1] = (pixel[1] as f32 * 0.9).max(0.0) as u8; // Slight decrease green
        pixel[2] = (pixel[2] as f32 * 0.95).max(0.0) as u8; // Slight decrease blue
    }
    Ok(())
}

fn apply_mauve(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Purple-infused filter
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 1.1).min(255.0) as u8;
        pixel[1] = (pixel[1] as f32 * 0.85).max(0.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.15).min(255.0) as u8;
    }
    Ok(())
}

fn apply_bluechrome(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Blue monochrome effect
    for pixel in image.pixels_mut() {
        let avg = ((pixel[0] as i32 + pixel[1] as i32 + pixel[2] as i32) / 3) as u8;
        // Tint towards blue
        pixel[0] = (avg as f32 * 0.7) as u8;
        pixel[1] = (avg as f32 * 0.8) as u8;
        pixel[2] = avg;
    }
    Ok(())
}

fn apply_vintage(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Vintage filter with red tint
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 1.1).min(255.0) as u8;
        pixel[1] = (pixel[1] as f32 * 0.95).max(0.0) as u8;
        pixel[2] = (pixel[2] as f32 * 0.8).max(0.0) as u8;

        // Decrease saturation
        let gray =
            (pixel[0] as f32 * 0.299 + pixel[1] as f32 * 0.587 + pixel[2] as f32 * 0.114) as u8;
        pixel[0] = ((pixel[0] as f32 + gray as f32) / 2.0) as u8;
        pixel[1] = ((pixel[1] as f32 + gray as f32) / 2.0) as u8;
        pixel[2] = ((pixel[2] as f32 + gray as f32) / 2.0) as u8;
    }
    Ok(())
}

fn apply_perfume(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Increase blue, moderate increases in red and green
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 1.05).min(255.0) as u8;
        pixel[1] = (pixel[1] as f32 * 1.05).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.2).min(255.0) as u8;
    }
    Ok(())
}

fn apply_serenity(image: &mut RgbaImage) -> ImageEffectResult<()> {
    // Increase blue channel's values
    for pixel in image.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.95).max(0.0) as u8;
        pixel[1] = (pixel[1] as f32 * 1.0).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * 1.25).min(255.0) as u8;
    }
    Ok(())
}
