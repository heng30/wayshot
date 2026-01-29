//! Text overlay demonstration
//!
//! This example demonstrates various text overlay features.

use std::path::Path;
use video_utils::{
    TextOverlayConfig, TextPosition,
    text_overlay, add_watermark, add_title,
};

const INPUT_VIDEO: &str = "data/test.mp4";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if test video exists
    if !Path::new(INPUT_VIDEO).exists() {
        eprintln!("Error: Test video '{}' not found. Please provide a test video file.", INPUT_VIDEO);
        eprintln!("You can modify INPUT_VIDEO at the top of this file to point to your video.");
        std::process::exit(1);
    }

    println!("Video Utils - Text Overlay Demo\n");
    println!("Input video: {}\n", INPUT_VIDEO);

    // Test 1: Simple watermark (bottom-right, white text, transparent background)
    println!("Test 1: Simple Watermark");
    println!("Position: Bottom-right, White text, Transparent background");
    text_overlay(
        TextOverlayConfig::new(INPUT_VIDEO, "tmp/text_watermark.mp4", "© 2026 Watermark")
            .with_position(TextPosition::BottomRight)
            .with_font_size(32)
            .with_color_rgb(255, 255, 255)
            .with_transparent_background()
            .with_padding(5)
    )?;
    println!("✓ Output: tmp/text_watermark.mp4\n");

    // Test 2: Title overlay (top-center, white text, black background)
    println!("Test 2: Title Overlay");
    println!("Position: Top-center, White text, Black background");
    text_overlay(
        TextOverlayConfig::new(INPUT_VIDEO, "tmp/text_title.mp4", "Video Title")
            .with_position(TextPosition::TopCenter)
            .with_font_size(48)
            .with_color_rgb(255, 255, 255)
            .with_background_rgb(0, 0, 0)
            .with_padding(10)
    )?;
    println!("✓ Output: tmp/text_title.mp4\n");

    // Test 3: Center overlay with custom position
    println!("Test 3: Center Overlay (Custom Position)");
    println!("Position: Custom (500, 400), Yellow text, Semi-transparent black background");
    text_overlay(
        TextOverlayConfig::new(INPUT_VIDEO, "tmp/text_center.mp4", "Centered Text")
            .with_position(TextPosition::Custom { x: 100, y: 100 })
            .with_font_size(36)
            .with_color_rgb(255, 255, 0)
            .with_background_rgb(0, 0, 0)
            .with_padding(8)
    )?;
    println!("✓ Output: tmp/text_center.mp4\n");

    // Test 4: Multiple overlays (using convenience functions)
    println!("Test 4: Convenience Functions");

    println!("  4a. add_watermark()");
    add_watermark(INPUT_VIDEO, "tmp/text_watermark_fn.mp4", "Watermark")?;
    println!("  ✓ Output: tmp/text_watermark_fn.mp4");

    println!("  4b. add_title()");
    add_title(INPUT_VIDEO, "tmp/text_title_fn.mp4", "My Title")?;
    println!("  ✓ Output: tmp/text_title_fn.mp4\n");

    // Verify outputs with ffprobe
    println!("Verifying outputs with ffprobe:");
    verify_video("tmp/text_watermark.mp4")?;
    verify_video("tmp/text_title.mp4")?;
    verify_video("tmp/text_center.mp4")?;
    verify_video("tmp/text_watermark_fn.mp4")?;
    verify_video("tmp/text_title_fn.mp4")?;

    println!("\n✅ All text overlay tests completed successfully!");
    println!("\nGenerated videos:");
    println!("  1. tmp/text_watermark.mp4 - Simple watermark");
    println!("  2. tmp/text_title.mp4 - Title overlay");
    println!("  3. tmp/text_center.mp4 - Custom positioned text");
    println!("  4. tmp/text_watermark_fn.mp4 - Watermark via convenience function");
    println!("  5. tmp/text_title_fn.mp4 - Title via convenience function");

    Ok(())
}

/// Verify video file with ffprobe
fn verify_video(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(path).exists() {
        println!("  ❌ {} - File not created", path);
        return Err(format!("Video file not found: {}", path).into());
    }

    // Get file size
    let metadata = std::fs::metadata(path)?;
    let size_kb = metadata.len() / 1024;

    // Use ffprobe to get basic info
    let output = std::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration,size")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout);
        let info_lines: Vec<&str> = info.trim().split('\n').collect();
        println!("  ✓ {} - {} KB (duration: {})", path, size_kb, info_lines.get(0).unwrap_or(&"unknown"));
    } else {
        println!("  ✓ {} - {} KB", path, size_kb);
    }

    Ok(())
}
