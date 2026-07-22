use image::{Rgba, RgbaImage, imageops};
use video_editor::filters::subtitle::{renderer::{create_text_image, render_text_to_image}, style::SubtitleStyle};
use std::path::Path;

fn main() {
    env_logger::init();

    log::info!("=== Font Comparison for Chinese Punctuation ===");

    let width = 1280;
    let height = 720;

    let sans_font = "../../wayshot/ui/fonts/SourceHanSansCN.otf";
    let serif_font = "../../wayshot/ui/fonts/SourceHanSerifCN.ttf";

    // Check fonts exist
    if !Path::new(sans_font).exists() {
        log::error!("Sans font not found: {}", sans_font);
        return;
    }
    if !Path::new(serif_font).exists() {
        log::error!("Serif font not found: {}", serif_font);
        return;
    }

    log::info!("Sans font: {}", sans_font);
    log::info!("Serif font: {}", serif_font);

    // Test text with Chinese punctuation
    let test_text = "你好，世界。测试、验证";

    // Create tmp directory
    std::fs::create_dir_all("tmp").ok();

    // Test with create_text_image (auto-sized background)
    log::info!("\n=== Test with create_text_image ===");

    for (name, font_path) in [("sans", sans_font), ("serif", serif_font)] {
        let style = SubtitleStyle::new()
            .with_font_path(font_path.into())
            .with_font_size(48)
            .with_primary_color(Some(Rgba([255, 255, 255, 255])))
            .with_outline_color(Some(Rgba([0, 0, 0, 255])))
            .with_outline_width(Some(2))
            .with_background_color(Some(Rgba([0, 0, 0, 150])))
            .with_padding(Some(10));

        match create_text_image(test_text, &style) {
            Ok(img) => {
                log::info!("Created {} image: {}x{}", name, img.width(), img.height());
                img.save(format!("tmp/create_text_{}.png", name)).unwrap();
                log::info!("Saved: tmp/create_text_{}.png", name);
            }
            Err(e) => log::error!("create_text_image failed: {}", e),
        }
    }

    // Test with render_text_to_image (positioned on canvas)
    log::info!("\n=== Test with render_text_to_image ===");

    // Test with SourceHanSansCN (problematic)
    log::info!("\n=== Test 1: SourceHanSansCN.otf ===");
    let style_sans = SubtitleStyle::new()
        .with_font_path(sans_font.into())
        .with_font_size(48)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2))
        .with_background_color(Some(Rgba([0, 0, 0, 150])))
        .with_padding(Some(10))
        .with_alignment(Some(2))
        .with_margin_vertical(Some(50));

    let mut img_sans = RgbaImage::from_pixel(width, height, Rgba([50, 50, 50, 255]));
    match render_text_to_image(&mut img_sans, test_text, &style_sans) {
        Ok(()) => {
            img_sans.save("tmp/font_comparison_sans.png").unwrap();
            log::info!("Saved: tmp/font_comparison_sans.png");
        }
        Err(e) => log::error!("Sans render failed: {}", e),
    }

    // Test with SourceHanSerifCN (working)
    log::info!("\n=== Test 2: SourceHanSerifCN.ttf ===");
    let style_serif = SubtitleStyle::new()
        .with_font_path(serif_font.into())
        .with_font_size(48)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2))
        .with_background_color(Some(Rgba([0, 0, 0, 150])))
        .with_padding(Some(10))
        .with_alignment(Some(5)) // Middle-center for comparison
        .with_margin_vertical(Some(0));

    let mut img_serif = RgbaImage::from_pixel(width, height, Rgba([50, 50, 50, 255]));
    match render_text_to_image(&mut img_serif, test_text, &style_serif) {
        Ok(()) => {
            img_serif.save("tmp/font_comparison_serif.png").unwrap();
            log::info!("Saved: tmp/font_comparison_serif.png");
        }
        Err(e) => log::error!("Serif render failed: {}", e),
    }

    // Combined comparison image - side by side instead of stacked
    let mut img_combined = RgbaImage::from_pixel(width * 2 + 20, height, Rgba([50, 50, 50, 255]));

    // Copy sans result to left side
    imageops::overlay(&mut img_combined, &img_sans, 0, 0);

    // Copy serif result to right side
    imageops::overlay(&mut img_combined, &img_serif, width as i64 + 20, 0);

    img_combined.save("tmp/font_comparison_combined.png").unwrap();
    log::info!("Saved combined comparison: tmp/font_comparison_combined.png");
    log::info!("Left: SourceHanSansCN.otf, Right: SourceHanSerifCN.ttf");

    log::info!("\n=== Test Complete ===");
    log::info!("Compare the punctuation marks (，。、) position in both fonts");
    log::info!("Punctuation should appear at text bottom, not centered");
}