use image::{Rgba, RgbaImage};
use std::path::Path;
use video_editor::filters::subtitle::{renderer::render_text_to_image, style::SubtitleStyle};

fn find_font_file() -> String {
    // First, try using fc-match to find a font, but skip .ttc files
    if let Ok(output) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}"])
        .output()
    {
        if output.status.success() {
            let font_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Prefer .ttf files over .ttc collections
            if !font_path.is_empty()
                && Path::new(&font_path).exists()
                && !font_path.ends_with(".ttc")
            {
                log::info!("Found font using fc-match: {}", font_path);
                return font_path;
            }
        }
    }

    // Try common TTF fonts first
    let common_fonts = vec![
        // DejaVu fonts (most common on Linux)
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        // Liberation fonts
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        // Noto fonts
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/opentype/noto/NotoSans-Regular.ttf",
        // Ubuntu fonts
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        // FreeSans
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        // Nix store paths (for NixOS systems)
        "/nix/store/lzqwng8ywfxxswj8g2f1bcwv2048iwva-dejavu-fonts-minimal-2.37/share/fonts/truetype/DejaVuSans.ttf",
    ];

    for font_path in common_fonts {
        if Path::new(font_path).exists() && !font_path.ends_with(".ttc") {
            return font_path.to_string();
        }
    }

    // If no common fonts found, try to find any .ttf file (skip .ttc files)
    let search_paths = vec![
        "/usr/share/fonts/truetype",
        "/usr/share/fonts",
        "~/.local/share/fonts",
        "~/.fonts",
    ];

    for base_path in search_paths {
        let expanded_path = base_path.replace("~", &std::env::var("HOME").unwrap_or_default());
        if let Ok(entries) = std::fs::read_dir(&expanded_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                    log::info!("Found font: {}", path.display());
                    return path.to_string_lossy().to_string();
                }
            }
        }
    }

    // Ultimate fallback
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()
}

fn main() {
    env_logger::init();

    log::info!("=== Subtitle Rendering Demo ===");

    let width = 640;
    let height = 480;
    let font_path = find_font_file();

    // Check if font file exists
    if !std::path::Path::new(&font_path).exists() {
        log::error!("Font file not found: {}", font_path);
        log::error!("Please install a font or update the font_path in this example");
        log::error!("On Ubuntu/Debian: sudo apt install fonts-dejavu-core");
        log::error!("On Fedora: sudo dnf install dejavu-sans-fonts");
        log::error!("On Arch: sudo pacman -S dejavu-fonts-ttf");
        return;
    }

    log::info!("Using font: {}", font_path);

    // Example 1: Simple white text with black outline
    log::info!("\n=== Example 1: Simple white text ===");
    let style1 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(30)
        .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(2))
        .with_alignment(Some(2)) // Bottom-center
        .with_margin_vertical(Some(30));

    let mut img1 = RgbaImage::new(width, height);
    match render_text_to_image(&mut img1, "Hello, World! good day", &style1) {
        Ok(()) => {
            log::info!(
                "Successfully rendered subtitle to {}x{} image",
                img1.width(),
                img1.height()
            );
            // Save the image to verify
            if let Err(e) = img1.save("tmp/output_subtitle_example1.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/output_subtitle_example1.png");
            }
        }
        Err(e) => {
            log::error!("Failed to render subtitle: {}", e);
        }
    }

    // Example 2: Yellow text with black outline (high visibility)
    log::info!("\n=== Example 2: Yellow text (high visibility) ===");
    let style2 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(32)
        .with_primary_color(Some(Rgba([255, 255, 0, 255]))) // Yellow
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(3))
        .with_alignment(Some(2))
        .with_margin_vertical(Some(50));

    let mut img2 = RgbaImage::new(width, height);
    match render_text_to_image(&mut img2, "High Visibility Subtitle", &style2) {
        Ok(()) => {
            log::info!(
                "Successfully rendered subtitle to {}x{} image",
                img2.width(),
                img2.height()
            );
            if let Err(e) = img2.save("tmp/output_subtitle_example2.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/output_subtitle_example2.png");
            }
        }
        Err(e) => {
            log::error!("Failed to render subtitle: {}", e);
        }
    }

    // Example 3: Text with background box
    log::info!("\n=== Example 3: Text with background box ===");
    let style3 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(30)
        .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(2))
        .with_background_color(Some(Rgba([0, 0, 0, 128]))) // Semi-transparent black
        .with_padding(Some(8))
        .with_alignment(Some(8)) // Top-center
        .with_margin_vertical(Some(20));

    let mut img3 = RgbaImage::new(width, height);
    match render_text_to_image(&mut img3, "Subtitle with background", &style3) {
        Ok(()) => {
            log::info!(
                "Successfully rendered subtitle to {}x{} image",
                img3.width(),
                img3.height()
            );
            if let Err(e) = img3.save("tmp/output_subtitle_example3.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/output_subtitle_example3.png");
            }
        }
        Err(e) => {
            log::error!("Failed to render subtitle: {}", e);
        }
    }

    // Example 4: Different alignments
    log::info!("\n=== Example 4: Different alignments ===");
    let alignments = vec![
        (1, "Bottom-Left"),
        (2, "Bottom-Center"),
        (3, "Bottom-Right"),
        (4, "Middle-Left"),
        (5, "Middle-Center"),
        (6, "Middle-Right"),
        (7, "Top-Left"),
        (8, "Top-Center"),
        (9, "Top-Right"),
    ];

    for (align, name) in alignments {
        let style = SubtitleStyle::new()
            .with_font_path(font_path.clone().into())
            .with_font_size(30)
            .with_primary_color(Some(Rgba([255, 255, 255, 255])))
            .with_outline_color(Some(Rgba([0, 0, 0, 255])))
            .with_outline_width(Some(2))
            .with_alignment(Some(align))
            .with_margin_vertical(Some(20))
            .with_margin_horizontal(Some(20));

        let mut img = RgbaImage::new(width, height);
        match render_text_to_image(&mut img, &format!("Alignment: {}", name), &style) {
            Ok(()) => {
                if let Err(e) = img.save(&format!("tmp/output_subtitle_align_{}.png", align)) {
                    log::warn!("Failed to save image: {}", e);
                } else {
                    log::info!("Saved: tmp/output_subtitle_align_{}.png ({})", align, name);
                }
            }
            Err(e) => {
                log::error!("Failed to render {}: {}", name, e);
            }
        }
    }

    // Example 5: Multi-line text
    log::info!("\n=== Example 5: Multi-line text (using \\N for newlines) ===");
    let style5 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(30)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2))
        .with_alignment(Some(2))
        .with_margin_vertical(Some(100));

    let mut img5 = RgbaImage::new(width, height);
    // Note: Uses \N for newlines (not \n)
    match render_text_to_image(&mut img5, "Line 1\\NLine 2\\NLine 3", &style5) {
        Ok(()) => {
            log::info!(
                "Successfully rendered multi-line subtitle to {}x{} image",
                img5.width(),
                img5.height()
            );
            if let Err(e) = img5.save("tmp/output_subtitle_multiline.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/output_subtitle_multiline.png");
            }
        }
        Err(e) => {
            log::error!("Failed to render multi-line subtitle: {}", e);
        }
    }

    log::info!("\n=== Demo completed ===");
    log::info!("Note: This implementation uses pure Rust rendering with cosmic-text");
}
