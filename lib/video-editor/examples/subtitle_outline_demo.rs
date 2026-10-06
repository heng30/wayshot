//! Subtitle outline sharpness demo.
//!
//! Renders text with a thick outline at a large font size so the outline can be
//! inspected pixel by pixel, then writes magnified crops next to the source
//! images. Use it to verify that outlines are solid (crisp inner edge with a
//! single anti-aliased pixel at the outer edge) instead of soft/blurry.

use image::{Rgba, RgbaImage, imageops};
use std::path::Path;
use video_editor::filters::subtitle::{
    renderer::{create_text_image, render_text_to_image},
    style::SubtitleStyle,
};

fn find_font_file() -> String {
    for candidate in [
        "../../wayshot/ui/fonts/SourceHanSansCN.otf",
        "../../wayshot/ui/fonts/NotoSansMonoCJKsc-Regular.otf",
    ] {
        if Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }

    if let Ok(output) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}"])
        .output()
        && output.status.success()
    {
        let font_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !font_path.is_empty() && Path::new(&font_path).exists() && !font_path.ends_with(".ttc") {
            return font_path;
        }
    }

    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()
}

/// Magnify a region around the text so individual pixels are visible.
fn save_zoom(img: &RgbaImage, name: &str, scale: u32) {
    let path = format!("tmp/{}.png", name);
    if let Err(e) = img.save(&path) {
        log::error!("Failed to save {}: {}", path, e);
        return;
    }

    let zoom_path = format!("tmp/{}_zoom.png", name);
    let zoomed = imageops::resize(
        img,
        img.width() * scale,
        img.height() * scale,
        imageops::FilterType::Nearest,
    );
    match zoomed.save(&zoom_path) {
        Ok(()) => log::info!("Saved: {} ({:.0}x)", zoom_path, scale as f32),
        Err(e) => log::error!("Failed to save {}: {}", zoom_path, e),
    }
}

/// Magnify a sub-rectangle, keeping the output small enough to inspect by eye.
fn save_crop_zoom(img: &RgbaImage, name: &str, x: u32, y: u32, w: u32, h: u32, scale: u32) {
    let w = w.min(img.width().saturating_sub(x));
    let h = h.min(img.height().saturating_sub(y));
    let crop = imageops::crop_imm(img, x, y, w, h).to_image();
    save_zoom(&crop, name, scale);
}

fn main() {
    env_logger::init();

    if let Err(e) = std::fs::create_dir_all("tmp") {
        log::error!("Failed to create tmp directory: {}", e);
        return;
    }

    let font_path = find_font_file();
    if !Path::new(&font_path).exists() {
        log::error!("Font file not found: {}", font_path);
        return;
    }
    log::info!("Using font: {}", font_path);

    // 1. Thick outline on a large glyph: the outline ring should be uniformly
    //    black with a single anti-aliased pixel along the outer border.
    log::info!("\n=== Example 1: thick outline, solid fill ===");
    let style = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(160)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(10))
        .with_padding(Some(4));

    match create_text_image("凝实", &style) {
        Ok(img) => save_zoom(&img, "outline_thick", 3),
        Err(e) => log::error!("create_text_image failed: {}", e),
    }

    // 2. Thin outline: a 2px ring must not fade into a gray halo.
    log::info!("\n=== Example 2: thin outline ===");
    let style = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(120)
        .with_primary_color(Some(Rgba([255, 210, 0, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2))
        .with_padding(Some(2));

    match create_text_image("Outline", &style) {
        Ok(img) => save_zoom(&img, "outline_thin", 4),
        Err(e) => log::error!("create_text_image failed: {}", e),
    }

    // 3. Outline over a background box: the outline must stay opaque where it
    //    overlaps the semi-transparent background.
    log::info!("\n=== Example 3: outline with background box ===");
    let style = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(96)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([255, 0, 0, 255])))
        .with_outline_width(Some(6))
        .with_background_color(Some(Rgba([0, 0, 0, 160])))
        .with_padding(Some(12));

    match create_text_image("背景描边", &style) {
        Ok(img) => save_zoom(&img, "outline_background", 3),
        Err(e) => log::error!("create_text_image failed: {}", e),
    }

    // 4. Full-canvas rendering: outline must survive the scaled/composited path.
    log::info!("\n=== Example 4: render_text_to_image on a canvas ===");
    let width = 1280;
    let height = 1080;
    let style = SubtitleStyle::new()
        .with_font_path(font_path.into())
        .with_font_size(180)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(8))
        .with_alignment(Some(5));

    let mut canvas = RgbaImage::from_pixel(width, height, Rgba([60, 120, 60, 255]));
    match render_text_to_image(&mut canvas, "描边测试", &style) {
        Ok(()) => save_crop_zoom(&canvas, "outline_canvas", 280, 400, 720, 280, 2),
        Err(e) => log::error!("render_text_to_image failed: {}", e),
    }

    log::info!("\n=== Demo completed ===");
}
