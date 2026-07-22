use image::{Rgba, RgbaImage};
use std::path::Path;
use video_editor::filters::{
    SubtitleFilter,
    subtitle::{
        renderer::render_text_to_image,
        style::{
            SubtitleStyle,
            alignment::AlignmentFilter,
            border::{BorderRadiusFilter, OutlineWidthFilter},
            colors::{BackgroundColorFilter, OutlineColorFilter, PrimaryColorFilter},
            font_path::FontPathFilter,
            font_size::FontSizeFilter,
            margin::{MarginHorizontalFilter, MarginVerticalFilter},
            padding::PaddingFilter,
        },
    },
};

fn main() {
    env_logger::init();

    log::info!("=== Subtitle Filters Demo - Using Filter apply() Functions ===");

    let width = 1280;
    let height = 720;
    let font_path = "../../wayshot/ui/fonts/SourceHanSansCN.otf".to_string();

    if !Path::new(&font_path).exists() {
        log::error!("Font file not found: {}", font_path);
        return;
    }

    log::info!("Using font: {}", font_path);

    // Demo 1: Full customization using all filters
    log::info!("\n=== Demo 1: Full customization with all filters ===");

    let mut style1 = SubtitleStyle::new();

    // Apply filters in sequence
    FontPathFilter::new(font_path.clone().into(), "SourceHanSansCN".to_string(), String::new()).apply(&mut style1);
    FontSizeFilter::new(36).apply(&mut style1);
    PrimaryColorFilter::from_rgba(255, 255, 0, 255).apply(&mut style1);
    OutlineColorFilter::from_rgba(0, 0, 0, 255).apply(&mut style1);
    OutlineWidthFilter::new(4).apply(&mut style1);
    BackgroundColorFilter::from_rgba(0, 0, 100, 180).apply(&mut style1);
    PaddingFilter::new(15).apply(&mut style1);
    BorderRadiusFilter::new(25).apply(&mut style1);
    AlignmentFilter::bottom_center().apply(&mut style1);
    MarginVerticalFilter::new(Some(80)).apply(&mut style1);
    MarginHorizontalFilter::new(50).apply(&mut style1);

    let mut img1 = RgbaImage::new(width, height);
    for (x, y, pixel) in img1.enumerate_pixels_mut() {
        let r = (x as f32 / width as f32 * 100.0) as u8;
        let g = (y as f32 / height as f32 * 100.0) as u8;
        *pixel = Rgba([r, g, 150, 255]);
    }

    match render_text_to_image(&mut img1, "使用 Filter 构建", &style1) {
        Ok(()) => {
            let _ = img1.save("tmp/subtitle_filters_full.png");
            log::info!("Saved: tmp/subtitle_filters_full.png");
        }
        Err(e) => {
            log::error!("Failed: {}", e);
        }
    }

    // Demo 2: Minimalist using helper methods
    log::info!("\n=== Demo 2: Minimalist ===");

    let mut style2 = SubtitleStyle::new();

    FontPathFilter::new(font_path.clone().into(), "SourceHanSansCN".to_string(), String::new()).apply(&mut style2);
    FontSizeFilter::new(28).apply(&mut style2);
    PrimaryColorFilter::from_rgba(255, 255, 255, 255).apply(&mut style2);
    OutlineColorFilter::from_rgba(0, 0, 0, 255).apply(&mut style2);
    OutlineWidthFilter::new(2).apply(&mut style2);
    AlignmentFilter::middle_center().apply(&mut style2);
    MarginVerticalFilter::new(Some(0)).apply(&mut style2);

    let mut img2 = RgbaImage::new(width, height);
    for pixel in img2.pixels_mut() {
        *pixel = Rgba([50, 50, 50, 255]);
    }

    match render_text_to_image(&mut img2, "极简风格", &style2) {
        Ok(()) => {
            let _ = img2.save("tmp/subtitle_filters_minimal.png");
            log::info!("Saved: tmp/subtitle_filters_minimal.png");
        }
        Err(e) => {
            log::error!("Failed: {}", e);
        }
    }

    // Demo 3: High contrast with opaque box
    log::info!("\n=== Demo 3: High contrast ===");

    let mut style3 = SubtitleStyle::new();

    FontPathFilter::new(font_path.clone().into(), "SourceHanSansCN".to_string(), String::new()).apply(&mut style3);
    FontSizeFilter::new(42).apply(&mut style3);
    PrimaryColorFilter::from_rgba(255, 255, 255, 255).apply(&mut style3);
    OutlineColorFilter::from_rgba(0, 0, 0, 0).apply(&mut style3);
    OutlineWidthFilter::new(3).apply(&mut style3);
    BackgroundColorFilter::from_rgba(0, 200, 0, 255).apply(&mut style3);
    PaddingFilter::new(20).apply(&mut style3);
    BorderRadiusFilter::new(10).apply(&mut style3);
    AlignmentFilter::top_center().apply(&mut style3);
    MarginVerticalFilter::new(Some(30)).apply(&mut style3);

    let mut img3 = RgbaImage::new(width, height);
    for pixel in img3.pixels_mut() {
        *pixel = Rgba([200, 100, 50, 255]);
    }

    match render_text_to_image(&mut img3, "高对比度 High contrast", &style3) {
        Ok(()) => {
            let _ = img3.save("tmp/subtitle_filters_contrast.png");
            log::info!("Saved: tmp/subtitle_filters_contrast.png");
        }
        Err(e) => {
            log::error!("Failed: {}", e);
        }
    }

    // Demo 4: Multi-line with cyan color
    log::info!("\n=== Demo 4: Multi-line ===");

    let mut style4 = SubtitleStyle::new();

    FontPathFilter::new(font_path.clone().into(), "SourceHanSansCN".to_string(), String::new()).apply(&mut style4);
    FontSizeFilter::new(32).apply(&mut style4);
    PrimaryColorFilter::from_rgba(0, 255, 255, 255).apply(&mut style4);
    OutlineColorFilter::from_rgba(0, 0, 0, 255).apply(&mut style4);
    OutlineWidthFilter::new(3).apply(&mut style4);
    BackgroundColorFilter::from_rgba(0, 0, 0, 150).apply(&mut style4);
    PaddingFilter::new(12).apply(&mut style4);
    BorderRadiusFilter::new(15).apply(&mut style4);
    AlignmentFilter::bottom_center().apply(&mut style4);
    MarginVerticalFilter::new(Some(100)).apply(&mut style4);

    let mut img4 = RgbaImage::new(width, height);
    for (x, _y, pixel) in img4.enumerate_pixels_mut() {
        let r = (x as f32 * 255.0 / width as f32) as u8;
        *pixel = Rgba([r, 0, 255 - r, 255]);
    }

    match render_text_to_image(
        &mut img4,
        "多行字幕演示\\N使用 Filter apply\\N每行独立样式",
        &style4,
    ) {
        Ok(()) => {
            let _ = img4.save("tmp/subtitle_filters_multiline.png");
            log::info!("Saved: tmp/subtitle_filters_multiline.png");
        }
        Err(e) => {
            log::error!("Failed: {}", e);
        }
    }

    // Demo 5: Demonstrate all alignment positions
    log::info!("\n=== Demo 5: All alignments ===");

    let alignments = vec![
        (AlignmentFilter::bottom_left(), "Bottom-Left"),
        (AlignmentFilter::bottom_center(), "Bottom-Center"),
        (AlignmentFilter::bottom_right(), "Bottom-Right"),
        (AlignmentFilter::middle_left(), "Middle-Left"),
        (AlignmentFilter::middle_center(), "Middle-Center"),
        (AlignmentFilter::middle_right(), "Middle-Right"),
        (AlignmentFilter::top_left(), "Top-Left"),
        (AlignmentFilter::top_center(), "Top-Center"),
        (AlignmentFilter::top_right(), "Top-Right"),
    ];

    for (align_filter, name) in alignments {
        let mut style = SubtitleStyle::new();

        FontPathFilter::new(font_path.clone().into(), "SourceHanSansCN".to_string(), String::new()).apply(&mut style);
        FontSizeFilter::new(28).apply(&mut style);
        PrimaryColorFilter::from_rgba(255, 255, 255, 255).apply(&mut style);
        OutlineColorFilter::from_rgba(0, 0, 0, 255).apply(&mut style);
        OutlineWidthFilter::new(2).apply(&mut style);
        BackgroundColorFilter::from_rgba(50, 50, 50, 200).apply(&mut style);
        PaddingFilter::new(10).apply(&mut style);
        align_filter.apply(&mut style);
        MarginVerticalFilter::new(Some(20)).apply(&mut style);
        MarginHorizontalFilter::new(20).apply(&mut style);

        let mut img = RgbaImage::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        let text = format!("对齐: {}", name);
        if render_text_to_image(&mut img, &text, &style).is_ok() {
            let align_num = style.alignment.unwrap_or(2);
            let _ = img.save(&format!("tmp/subtitle_filters_align_{}.png", align_num));
            log::info!(
                "Saved: tmp/subtitle_filters_align_{}.png - {}",
                align_num,
                name
            );
        }
    }

    log::info!("\n=== All demos completed ===");
    log::info!("All demos generated using filter apply() functions!");
}
