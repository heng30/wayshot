// Example demonstrating the text highlight filter
// Generates test images and saves them to tmp/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{TextHighlightFilter, HighlightMode, HighlightRegion},
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a test image with dark background and white "text" areas
fn create_test_image(width: u32, height: u32) -> RgbaImage {
    // Dark background
    let mut img = RgbaImage::from_pixel(width, height, Rgba([30, 30, 30, 255]));

    // Draw some white "text" rectangles to simulate text areas
    let text_color = Rgba([255, 255, 255, 255]);

    // Row 1: text areas
    draw_text_rect(&mut img, 100, 50, 400, 25, text_color);
    draw_text_rect(&mut img, 510, 50, 150, 25, text_color);

    // Row 2: text areas
    draw_text_rect(&mut img, 100, 100, 600, 25, text_color);

    // Row 3: text areas
    draw_text_rect(&mut img, 100, 150, 300, 25, text_color);
    draw_text_rect(&mut img, 420, 150, 250, 25, text_color);

    // Row 4: text areas
    draw_text_rect(&mut img, 100, 200, 550, 25, text_color);

    // Row 5: text areas
    draw_text_rect(&mut img, 100, 250, 200, 25, text_color);
    draw_text_rect(&mut img, 320, 250, 350, 25, text_color);

    img
}

/// Create a test image with light background and dark text
fn create_light_test_image(width: u32, height: u32) -> RgbaImage {
    // Light background (white)
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    // Draw dark "text" rectangles
    let text_color = Rgba([0, 0, 0, 255]);

    // Row 1: text areas
    draw_text_rect(&mut img, 100, 50, 400, 25, text_color);
    draw_text_rect(&mut img, 510, 50, 150, 25, text_color);

    // Row 2: text areas
    draw_text_rect(&mut img, 100, 100, 600, 25, text_color);

    // Row 3: text areas
    draw_text_rect(&mut img, 100, 150, 300, 25, text_color);
    draw_text_rect(&mut img, 420, 150, 250, 25, text_color);

    // Row 4: text areas
    draw_text_rect(&mut img, 100, 200, 550, 25, text_color);

    // Row 5: text areas
    draw_text_rect(&mut img, 100, 250, 200, 25, text_color);
    draw_text_rect(&mut img, 320, 250, 350, 25, text_color);

    img
}

/// Draw a rectangle simulating a text area
fn draw_text_rect(img: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, color: Rgba<u8>) {
    for py in y..(y + height).min(img.height()) {
        for px in x..(x + width).min(img.width()) {
            img.put_pixel(px, py, color);
        }
    }
}

/// Create a dummy segment for VideoData
fn create_dummy_segment(duration: Duration) -> std::sync::Arc<video_editor::tracks::segment::Segment> {
    let metadata = std::sync::Arc::new(Metadata {
        path: PathBuf::from("dummy.mp4"),
        size: 0,
        bitrate: 0,
        duration,
        format: vec![],
        videos: vec![],
        audios: vec![],
        subtitles: vec![],
    });
    std::sync::Arc::new(video_editor::tracks::segment::Segment::new(
        Duration::ZERO,
        duration,
        metadata,
        1.0,
    ))
}

/// Apply filter to an image and return the result
fn apply_filter_to_image(
    filter: &TextHighlightFilter,
    buffer: RgbaImage,
    width: u32,
    height: u32,
    fps: f32,
    time_offset: Duration,
    segment_duration: Duration,
) -> Result<RgbaImage> {
    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image { buffer }],
        from_segment: create_dummy_segment(segment_duration),
        relative_timeline_offset: time_offset,
    };

    filter.apply(&mut video_data)?;

    if let Some(VideoImage::Image { buffer, .. }) = video_data.frames.first() {
        Ok(buffer.clone())
    } else {
        Err(video_editor::Error::InvalidConfig("No image generated".into()))
    }
}

fn save_image(image: &RgbaImage, path: &str) -> Result<()> {
    image.save(path).map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

fn main() -> Result<()> {
    println!("Text Highlight Filter Demo");
    println!("==========================\n");

    // Create tmp directory
    let tmp_dir = "tmp/text_highlight_demo";
    fs::create_dir_all(tmp_dir)?;

    let width = 800u32;
    let height = 400u32;
    let fps = 25.0;

    // Define regions to highlight (normalized 0-1 coordinates)
    // These cover the "text rows" in our test image
    let regions = vec![
        HighlightRegion { x: 0.1, y: 0.1, width: 0.75, height: 0.0875 },  // Row 1
        HighlightRegion { x: 0.1, y: 0.225, width: 0.8, height: 0.0875 }, // Row 2
        HighlightRegion { x: 0.1, y: 0.35, width: 0.75, height: 0.0875 }, // Row 3
        HighlightRegion { x: 0.1, y: 0.475, width: 0.725, height: 0.0875 }, // Row 4
        HighlightRegion { x: 0.1, y: 0.6, width: 0.75, height: 0.0875 },  // Row 5
    ];

    // Example 1: Dark background with white text - TextColor mode
    println!("Example 1: Dark background, TextColor mode (detect non-white pixels)");
    let dark_img = create_test_image(width, height);
    save_image(&dark_img, &format!("{}/dark_original.png", tmp_dir))?;
    println!("  Saved: dark_original.png");

    // Total duration: 5 regions * 100ms + 1000ms pause = 1500ms
    let total_duration = Duration::from_millis(1500);

    // At time 0ms: no highlighting yet
    println!("  Creating frame at 0ms (no highlighting)");
    let filter1 = TextHighlightFilter::new(regions.clone())
        .with_highlight_mode(HighlightMode::TextColor)
        .with_highlight_color([255, 255, 0, 180])  // Yellow highlight
        .with_text_color([255, 255, 255])          // White text
        .with_duration_per_region_ms(100)
        .with_end_pause_seconds(1.0);

    let frame1 = apply_filter_to_image(
        &filter1,
        dark_img.clone(),
        width, height, fps,
        Duration::ZERO,
        total_duration,
    )?;
    save_image(&frame1, &format!("{}/dark_frame_0ms.png", tmp_dir))?;
    println!("    Saved: dark_frame_0ms.png");

    // At time 50ms: first region partially highlighted (progress ~0.5)
    println!("  Creating frame at 50ms (first region ~50% highlighted)");
    let frame2 = apply_filter_to_image(
        &filter1,
        dark_img.clone(),
        width, height, fps,
        Duration::from_millis(50),
        total_duration,
    )?;
    save_image(&frame2, &format!("{}/dark_frame_50ms.png", tmp_dir))?;
    println!("    Saved: dark_frame_50ms.png");

    // At time 100ms: first region fully highlighted
    println!("  Creating frame at 100ms (first region fully highlighted)");
    let frame3 = apply_filter_to_image(
        &filter1,
        dark_img.clone(),
        width, height, fps,
        Duration::from_millis(100),
        total_duration,
    )?;
    save_image(&frame3, &format!("{}/dark_frame_100ms.png", tmp_dir))?;
    println!("    Saved: dark_frame_100ms.png");

    // At time 250ms: middle of third region
    println!("  Creating frame at 250ms (third region ~50% highlighted)");
    let frame4 = apply_filter_to_image(
        &filter1,
        dark_img.clone(),
        width, height, fps,
        Duration::from_millis(250),
        total_duration,
    )?;
    save_image(&frame4, &format!("{}/dark_frame_250ms.png", tmp_dir))?;
    println!("    Saved: dark_frame_250ms.png");

    // At time 500ms: all regions complete, pause phase
    println!("  Creating frame at 500ms (all regions fully highlighted)");
    let frame5 = apply_filter_to_image(
        &filter1,
        dark_img.clone(),
        width, height, fps,
        Duration::from_millis(500),
        total_duration,
    )?;
    save_image(&frame5, &format!("{}/dark_frame_500ms.png", tmp_dir))?;
    println!("    Saved: dark_frame_500ms.png");

    // Example 2: Light background with dark text - BackgroundColor mode
    println!("\nExample 2: Light background, BackgroundColor mode (detect white pixels)");
    let light_img = create_light_test_image(width, height);
    save_image(&light_img, &format!("{}/light_original.png", tmp_dir))?;
    println!("  Saved: light_original.png");

    let filter2 = TextHighlightFilter::new(regions.clone())
        .with_highlight_mode(HighlightMode::BackgroundColor)
        .with_highlight_color([255, 200, 0, 150])  // Orange highlight
        .with_background_color_to_detect([255, 255, 255])  // White background
        .with_duration_per_region_ms(100)
        .with_end_pause_seconds(1.0);

    // At time 0ms: no highlighting
    println!("  Creating frame at 0ms (no highlighting)");
    let light_frame1 = apply_filter_to_image(
        &filter2,
        light_img.clone(),
        width, height, fps,
        Duration::ZERO,
        total_duration,
    )?;
    save_image(&light_frame1, &format!("{}/light_frame_0ms.png", tmp_dir))?;
    println!("    Saved: light_frame_0ms.png");

    // At time 500ms: all regions highlighted
    println!("  Creating frame at 500ms (all regions highlighted)");
    let light_frame2 = apply_filter_to_image(
        &filter2,
        light_img.clone(),
        width, height, fps,
        Duration::from_millis(500),
        total_duration,
    )?;
    save_image(&light_frame2, &format!("{}/light_frame_500ms.png", tmp_dir))?;
    println!("    Saved: light_frame_500ms.png");

    // Example 3: Different highlight colors
    println!("\nExample 3: Different highlight colors");
    for (color_name, color) in [
        ("yellow", [255, 255, 0, 180]),
        ("green", [0, 255, 0, 150]),
        ("blue", [0, 100, 255, 150]),
        ("pink", [255, 100, 200, 180]),
        ("cyan", [0, 255, 255, 150]),
    ] {
        println!("  Testing {} highlight color", color_name);
        let filter = TextHighlightFilter::new(regions.clone())
            .with_highlight_mode(HighlightMode::TextColor)
            .with_highlight_color(color)
            .with_text_color([255, 255, 255])
            .with_duration_per_region_ms(100)
            .with_end_pause_seconds(1.0);

        let frame = apply_filter_to_image(
            &filter,
            dark_img.clone(),
            width, height, fps,
            Duration::from_millis(500),  // All regions complete
            total_duration,
        )?;
        save_image(&frame, &format!("{}/highlight_{}.png", tmp_dir, color_name))?;
        println!("    Saved: highlight_{}.png", color_name);
    }

    // Example 4: Different alpha values
    println!("\nExample 4: Different alpha values (opacity)");
    for (alpha_name, alpha) in [
        ("50", 50),
        ("100", 100),
        ("150", 150),
        ("200", 200),
        ("255", 255),
    ] {
        println!("  Testing alpha = {}", alpha);
        let filter = TextHighlightFilter::new(regions.clone())
            .with_highlight_mode(HighlightMode::TextColor)
            .with_highlight_color([255, 255, 0, alpha])
            .with_text_color([255, 255, 255])
            .with_duration_per_region_ms(100)
            .with_end_pause_seconds(1.0);

        let frame = apply_filter_to_image(
            &filter,
            dark_img.clone(),
            width, height, fps,
            Duration::from_millis(500),
            total_duration,
        )?;
        save_image(&frame, &format!("{}/alpha_{}.png", tmp_dir, alpha_name))?;
        println!("    Saved: alpha_{}.png", alpha_name);
    }

    // Example 5: Single region highlight
    println!("\nExample 5: Single region highlight");
    let single_region = vec![HighlightRegion { x: 0.1, y: 0.225, width: 0.8, height: 0.0875 }];  // Row 2 only
    let filter5 = TextHighlightFilter::new(single_region)
        .with_highlight_mode(HighlightMode::TextColor)
        .with_highlight_color([255, 255, 0, 200])
        .with_text_color([255, 255, 255])
        .with_duration_per_region_ms(200)
        .with_end_pause_seconds(0.5);

    let single_duration = Duration::from_millis(700);  // 200ms + 500ms pause

    // At time 100ms: region ~50% highlighted
    println!("  Creating frame at 100ms (~50% highlighted)");
    let single_frame1 = apply_filter_to_image(
        &filter5,
        dark_img.clone(),
        width, height, fps,
        Duration::from_millis(100),
        single_duration,
    )?;
    save_image(&single_frame1, &format!("{}/single_region_50percent.png", tmp_dir))?;
    println!("    Saved: single_region_50percent.png");

    // At time 200ms: region fully highlighted
    println!("  Creating frame at 200ms (fully highlighted)");
    let single_frame2 = apply_filter_to_image(
        &filter5,
        dark_img.clone(),
        width, height, fps,
        Duration::from_millis(200),
        single_duration,
    )?;
    save_image(&single_frame2, &format!("{}/single_region_full.png", tmp_dir))?;
    println!("    Saved: single_region_full.png");

    println!("\n==========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nFilter parameters:");
    println!("  - regions: List of (x, y, width, height) in normalized 0-1 coords");
    println!("  - highlight_mode: TextColor or BackgroundColor");
    println!("  - highlight_color: RGBA color for highlighted pixels");
    println!("  - text_color: RGB color for text detection (TextColor mode)");
    println!("  - background_color_to_detect: RGB color for background detection (BackgroundColor mode)");
    println!("  - duration_per_region_ms: Time to animate each region");
    println!("  - end_pause_seconds: Hold time after all regions complete");

    Ok(())
}