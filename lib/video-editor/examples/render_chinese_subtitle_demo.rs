use image::{Rgba, RgbaImage};
use video_editor::filters::subtitle::{renderer::render_text_to_image, style::SubtitleStyle};

fn main() {
    env_logger::init();

    log::info!("=== 中文字幕渲染测试 ===");

    let width = 1280;
    let height = 720;
    let font_path = "../../wayshot/ui/fonts/SourceHanSansCN.otf".to_string();

    if !std::path::Path::new(&font_path).exists() {
        log::error!("Font file not found: {}", font_path);
        return;
    }

    log::info!("Using font: {}", font_path);

    // Test 1: 简单中文字幕
    log::info!("\n=== 测试 1: 简单中文字幕 ===");
    let style1 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(48)
        .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(3))
        .with_alignment(Some(2)) // Bottom-center
        .with_margin_vertical(Some(50));

    let mut img1 = RgbaImage::from_pixel(width, height, Rgba([0, 255, 0, 255]));

    match render_text_to_image(&mut img1, "你好，世界！", &style1) {
        Ok(()) => {
            log::info!("Successfully rendered: {}x{}", img1.width(), img1.height());
            let _ = img1.save("tmp/chinese_test1.png");
            log::info!("Saved to: tmp/chinese_test1.png");
        }
        Err(e) => {
            log::error!("Failed to render: {}", e);
        }
    }

    // Test 2: 中文字幕带背景
    log::info!("\n=== 测试 2: 中文字幕带背景 ===");
    let style2 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(40)
        .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(4))
        .with_background_color(Some(Rgba([200, 200, 0, 200]))) // Semi-transparent green
        .with_padding(Some(10))
        .with_border_radius(Some(20))
        .with_alignment(Some(2))
        .with_margin_vertical(Some(100));

    let mut img2 = RgbaImage::from_pixel(width, height, Rgba([255, 0, 0, 255]));
    match render_text_to_image(&mut img2, "这是一个中文字幕测试", &style2) {
        Ok(()) => {
            log::info!("Successfully rendered: {}x{}", img2.width(), img2.height());
            let _ = img2.save("tmp/chinese_test2.png");
            log::info!("Saved to: tmp/chinese_test2.png");
        }
        Err(e) => {
            log::error!("Failed to render: {}", e);
        }
    }

    // Test 3: 多行中文
    log::info!("\n=== 测试 3: 多行中文字幕 ===");
    let style3 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(36)
        .with_primary_color(Some(Rgba([0, 255, 255, 255]))) // Cyan
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(2))
        .with_alignment(Some(5)) // Middle-center
        .with_margin_vertical(Some(0));

    let mut img3 = RgbaImage::from_pixel(width, height, Rgba([255, 0, 0, 255]));
    match render_text_to_image(&mut img3, "第一行字幕\\N第二行字幕\\N第三行字幕", &style3)
    {
        Ok(()) => {
            log::info!("Successfully rendered: {}x{}", img3.width(), img3.height());
            let _ = img3.save("tmp/chinese_test3.png");
            log::info!("Saved to: tmp/chinese_test3.png");
        }
        Err(e) => {
            log::error!("Failed to render: {}", e);
        }
    }

    log::info!("\n=== 测试完成 ===");
}
