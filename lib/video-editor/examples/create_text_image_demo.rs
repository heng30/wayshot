use image::{imageops, Rgba, RgbaImage};
use std::path::Path;
use video_editor::filters::subtitle::{renderer::create_text_image, style::SubtitleStyle};

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

    log::info!("=== create_text_image Demo ===");
    log::info!("This function creates a transparent image sized exactly to fit the text");

    // Create tmp directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all("tmp") {
        log::error!("Failed to create tmp directory: {}", e);
        return;
    }

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

    // Example 1: Simple text - image sized to fit text
    log::info!("\n=== Example 1: Simple text (auto-sized) ===");
    let style1 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(30)
        .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(2));

    match create_text_image("Hello World!", &style1) {
        Ok(img) => {
            log::info!(
                "Created image: {}x{} (sized to fit text)",
                img.width(),
                img.height()
            );
            if let Err(e) = img.save("tmp/create_text_example1.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/create_text_example1.png");
            }
        }
        Err(e) => {
            log::error!("Failed to create text image: {}", e);
        }
    }

    // Example 2: Text with background
    log::info!("\n=== Example 2: Text with background ===");
    let style2 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(32)
        .with_primary_color(Some(Rgba([255, 255, 0, 255]))) // Yellow
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
        .with_outline_width(Some(3))
        .with_background_color(Some(Rgba([0, 0, 0, 180]))) // Semi-transparent black
        .with_padding(Some(10))
        .with_border_radius(Some(8));

    match create_text_image("Styled Text", &style2) {
        Ok(img) => {
            log::info!("Created image: {}x{}", img.width(), img.height());
            if let Err(e) = img.save("tmp/create_text_example2.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/create_text_example2.png");
            }
        }
        Err(e) => {
            log::error!("Failed to create text image: {}", e);
        }
    }

    // Example 3: Multi-line text
    log::info!("\n=== Example 3: Multi-line text ===");
    let style3 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(28)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2))
        .with_padding(Some(8));

    match create_text_image("Line 1\\NLine 2\\NLine 3", &style3) {
        Ok(img) => {
            log::info!("Created image: {}x{}", img.width(), img.height());
            if let Err(e) = img.save("tmp/create_text_example3.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/create_text_example3.png");
            }
        }
        Err(e) => {
            log::error!("Failed to create text image: {}", e);
        }
    }

    // Example 4: Demonstrate how to position the text image on a canvas
    log::info!("\n=== Example 4: Position text on a 640x480 canvas ===");
    let style4 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(36)
        .with_primary_color(Some(Rgba([255, 100, 100, 255]))) // Light red
        .with_outline_color(Some(Rgba([255, 255, 255, 255]))) // White
        .with_outline_width(Some(3));

    match create_text_image("Positioned Text", &style4) {
        Ok(text_img) => {
            // Create a canvas and place text at different positions
            let canvas_size = 640;
            let mut canvas = RgbaImage::new(canvas_size, canvas_size);

            // Fill with dark background
            for pixel in canvas.pixels_mut() {
                *pixel = Rgba([50, 50, 50, 255]);
            }

            // Position 1: Top-left
            let x = 20_i64;
            let y = 20_i64;
            imageops::overlay(&mut canvas, &text_img, x, y);
            log::info!("Placed text at top-left ({}, {})", x, y);

            // Position 2: Center
            let x = ((canvas_size - text_img.width()) / 2) as i64;
            let y = ((canvas_size - text_img.height()) / 2) as i64;
            imageops::overlay(&mut canvas, &text_img, x, y);
            log::info!("Placed text at center ({}, {})", x, y);

            // Position 3: Bottom-right
            let x = (canvas_size - text_img.width() - 20) as i64;
            let y = (canvas_size - text_img.height() - 20) as i64;
            imageops::overlay(&mut canvas, &text_img, x, y);
            log::info!("Placed text at bottom-right ({}, {})", x, y);

            if let Err(e) = canvas.save("tmp/create_text_position_demo.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/create_text_position_demo.png");
            }
        }
        Err(e) => {
            log::error!("Failed to create text image: {}", e);
        }
    }

    // Example 5: Different font sizes
    log::info!("\n=== Example 5: Different font sizes ===");
    for size in [16, 24, 32, 48, 64] {
        let style = SubtitleStyle::new()
            .with_font_path(font_path.clone().into())
            .with_font_size(size)
            .with_primary_color(Some(Rgba([100, 255, 100, 255]))) // Light green
            .with_outline_color(Some(Rgba([0, 0, 0, 255])))
            .with_outline_width(Some(2));

        match create_text_image(&format!("Size {}", size), &style) {
            Ok(img) => {
                if let Err(e) = img.save(&format!("tmp/create_text_size_{}.png", size)) {
                    log::warn!("Failed to save image: {}", e);
                } else {
                    log::info!(
                        "Saved: tmp/create_text_size_{}.png ({}x{})",
                        size,
                        img.width(),
                        img.height()
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to create text image (size {}): {}", size, e);
            }
        }
    }

    // Example 6: Text with border around background
    log::info!("\n=== Example 6: Text with border around background ===");
    let style6 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(36)
        .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White text
        .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black outline
        .with_outline_width(Some(2))
        .with_background_color(Some(Rgba([40, 40, 40, 200]))) // Dark semi-transparent background
        .with_padding(Some(12))
        .with_border_radius(Some(10))
        .with_border_width(Some(4))
        .with_border_color(Some(Rgba([255, 100, 50, 255]))); // Orange border

    match create_text_image("Border Test", &style6) {
        Ok(img) => {
            log::info!("Created image: {}x{}", img.width(), img.height());
            if let Err(e) = img.save("tmp/create_text_example6_border.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/create_text_example6_border.png");
            }
        }
        Err(e) => {
            log::error!("Failed to create text image: {}", e);
        }
    }

    // Example 7: Border without background (border only)
    log::info!("\n=== Example 7: Border only (no background) ===");
    let style7 = SubtitleStyle::new()
        .with_font_path(font_path.clone().into())
        .with_font_size(32)
        .with_primary_color(Some(Rgba([255, 255, 255, 255])))
        .with_outline_color(Some(Rgba([0, 0, 0, 255])))
        .with_outline_width(Some(2))
        .with_padding(Some(10))
        .with_border_radius(Some(8))
        .with_border_width(Some(3))
        .with_border_color(Some(Rgba([0, 200, 255, 255]))); // Cyan border

    match create_text_image("Border Only", &style7) {
        Ok(img) => {
            log::info!("Created image: {}x{}", img.width(), img.height());
            if let Err(e) = img.save("tmp/create_text_example7_border_only.png") {
                log::warn!("Failed to save image: {}", e);
            } else {
                log::info!("Saved to: tmp/create_text_example7_border_only.png");
            }
        }
        Err(e) => {
            log::error!("Failed to create text image: {}", e);
        }
    }

    // Example 8: Different border widths
    log::info!("\n=== Example 8: Different border widths ===");
    for border_width in [1, 2, 4, 6, 8] {
        let style = SubtitleStyle::new()
            .with_font_path(font_path.clone().into())
            .with_font_size(28)
            .with_primary_color(Some(Rgba([255, 255, 255, 255])))
            .with_outline_color(Some(Rgba([0, 0, 0, 255])))
            .with_outline_width(Some(2))
            .with_background_color(Some(Rgba([30, 30, 30, 180])))
            .with_padding(Some(8))
            .with_border_radius(Some(6))
            .with_border_width(Some(border_width))
            .with_border_color(Some(Rgba([255, 0, 100, 255]))); // Red border

        match create_text_image(&format!("BW {}", border_width), &style) {
            Ok(img) => {
                if let Err(e) = img.save(&format!("tmp/create_text_border_width_{}.png", border_width)) {
                    log::warn!("Failed to save image: {}", e);
                } else {
                    log::info!(
                        "Saved: tmp/create_text_border_width_{}.png ({}x{})",
                        border_width,
                        img.width(),
                        img.height()
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to create text image (border_width {}): {}", border_width, e);
            }
        }
    }

    // Example 9: Chinese text with punctuation (test baseline fix)
    log::info!("\n=== Example 9: Chinese text with punctuation ===");
    let chinese_font_path = "/home/blue/Code/rust/wayshot/wayshot/ui/fonts/SourceHanSerifCN.ttf";

    if Path::new(chinese_font_path).exists() {
        let style_chinese = SubtitleStyle::new()
            .with_font_path(chinese_font_path.into())
            .with_font_size(48)
            .with_primary_color(Some(Rgba([255, 255, 255, 255]))) // White
            .with_outline_color(Some(Rgba([0, 0, 0, 255]))) // Black
            .with_outline_width(Some(2))
            .with_background_color(Some(Rgba([0, 0, 0, 150])))
            .with_padding(Some(10));

        // Test Chinese punctuation: ，。、
        match create_text_image("你好，世界。测试、验证", &style_chinese) {
            Ok(img) => {
                log::info!("Created Chinese text image: {}x{}", img.width(), img.height());
                if let Err(e) = img.save("tmp/chinese_punctuation_test.png") {
                    log::warn!("Failed to save image: {}", e);
                } else {
                    log::info!("Saved to: tmp/chinese_punctuation_test.png");
                    log::info!("Check that punctuation marks (，。、) appear at line bottom, not centered");
                }
            }
            Err(e) => {
                log::error!("Failed to create Chinese text image: {}", e);
            }
        }
    } else {
        log::warn!("Chinese font not found: {}", chinese_font_path);
        log::warn!("Skipping Chinese punctuation test");
    }

    log::info!("\n=== Demo completed ===");
    log::info!("The create_text_image function returns a transparent image sized to fit the text");
    log::info!("This allows for flexible positioning, rotation, and scaling in subsequent filters");
}
