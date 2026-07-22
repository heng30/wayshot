// Example demonstrating the edge detection filter
// Generates test images and saves them to tmp/edge_detect_demo/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::EdgeDetectFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_edges(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([240, 240, 240, 255]));

    // Add several shapes with clear edges

    // Large rectangle in center
    let center_x = width / 2;
    let center_y = height / 2;
    let rect_w = width / 3;
    let rect_h = height / 3;
    let rect_start_x = center_x.saturating_sub(rect_w / 2);
    let rect_end_x = center_x.saturating_add(rect_w / 2).min(width);
    let rect_start_y = center_y.saturating_sub(rect_h / 2);
    let rect_end_y = center_y.saturating_add(rect_h / 2).min(height);

    for y in rect_start_y..rect_end_y {
        for x in rect_start_x..rect_end_x {
            img.put_pixel(x, y, Rgba([100, 80, 60, 255]));
        }
    }

    // Circle in top-left corner
    let circle_center = (width / 4, height / 4);
    let radius = (height / 6) as f32;
    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 - circle_center.0 as f32).abs();
            let dy = (y as f32 - circle_center.1 as f32).abs();
            if dx * dx + dy * dy < radius * radius {
                img.put_pixel(x, y, Rgba([200, 100, 50, 255]));
            }
        }
    }

    // Gradient stripe on right side
    for y in 0..height {
        for x in (width * 3 / 4)..width {
            let gradient = (y as f32 / height as f32 * 200.0) as u8;
            img.put_pixel(x, y, Rgba([gradient, gradient, 255, 255]));
        }
    }

    // Triangle in bottom-left
    let tri_center = (width / 5, height * 4 / 5);
    for y in (height * 3 / 5)..height {
        for x in 0..(width / 3) {
            let dy = y as f32 - tri_center.1 as f32;
            if dy.abs() < 50.0 {
                let dx = (x as f32 - tri_center.0 as f32).abs();
                if dx < (50.0 - dy.abs() * 0.5) {
                    img.put_pixel(x, y, Rgba([50, 150, 200, 255]));
                }
            }
        }
    }

    img
}

fn create_dummy_segment() -> std::sync::Arc<video_editor::tracks::segment::Segment> {
    let metadata = std::sync::Arc::new(Metadata {
        path: PathBuf::from("dummy.mp4"),
        size: 0,
        bitrate: 0,
        duration: Duration::from_secs(10),
        format: vec![],
        videos: vec![],
        audios: vec![],
        subtitles: vec![],
    });
    std::sync::Arc::new(video_editor::tracks::segment::Segment::new(
        Duration::ZERO,
        Duration::from_secs(10),
        metadata,
        1.0,
    ))
}

fn apply_filter_to_image(
    filter: &EdgeDetectFilter,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<RgbaImage> {
    let buffer = create_test_image_with_edges(width, height);

    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image { buffer }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: Duration::ZERO,
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
    println!("Edge Detection Filter Demo");
    println!("==========================\n");

    // Create tmp directory
    let tmp_dir = "tmp/edge_detect_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for comparison
    println!("Saving original test image...");
    let original = create_test_image_with_edges(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default edge detection (black edges on white)
    println!("\nExample 1: Default edge detection (threshold=30, strength=1.0)");
    let filter1 = EdgeDetectFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/edge_default.png", tmp_dir))?;
    println!("  Saved: edge_default.png");

    // Example 2: Inverted edges (white edges on black)
    println!("\nExample 2: Inverted edges (white edges on black background)");
    let filter2 = EdgeDetectFilter::new(30.0, 1.0).with_invert(true);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/edge_inverted.png", tmp_dir))?;
    println!("  Saved: edge_inverted.png");

    // Example 3: Threshold variations
    println!("\nExample 3: Threshold comparison (10 to 100)");
    for threshold in [10.0, 20.0, 30.0, 50.0, 75.0, 100.0] {
        let filter = EdgeDetectFilter::new(threshold, 1.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/edge_threshold_{:.0}.png", tmp_dir, threshold))?;
    }
    println!("  Saved: edge_threshold_10.png to edge_threshold_100.png");

    // Example 4: Strength variations
    println!("\nExample 4: Strength comparison (0.5 to 2.0)");
    for strength in [0.5, 1.0, 1.5, 2.0] {
        let filter = EdgeDetectFilter::new(30.0, strength);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/edge_strength_{:.1}.png", tmp_dir, strength))?;
    }
    println!("  Saved: edge_strength_0.5.png to edge_strength_2.0.png");

    // Example 5: Custom edge colors
    println!("\nExample 5: Custom edge colors");
    let filter5_blue = EdgeDetectFilter::new(30.0, 1.0).with_edge_color([0, 0, 255, 255]);
    let img5_blue = apply_filter_to_image(&filter5_blue, width, height, fps)?;
    save_image(&img5_blue, &format!("{}/edge_blue.png", tmp_dir))?;

    let filter5_red = EdgeDetectFilter::new(30.0, 1.0).with_edge_color([255, 0, 0, 255]);
    let img5_red = apply_filter_to_image(&filter5_red, width, height, fps)?;
    save_image(&img5_red, &format!("{}/edge_red.png", tmp_dir))?;

    let filter5_green = EdgeDetectFilter::new(30.0, 1.0).with_edge_color([0, 255, 0, 255]);
    let img5_green = apply_filter_to_image(&filter5_green, width, height, fps)?;
    save_image(&img5_green, &format!("{}/edge_green.png", tmp_dir))?;
    println!("  Saved: edge_blue.png, edge_red.png, edge_green.png");

    // Example 6: Custom background colors (colored paper effect)
    println!("\nExample 6: Custom background colors");
    let filter6_gray = EdgeDetectFilter::new(30.0, 1.0)
        .with_background_color([200, 200, 200, 255])
        .with_edge_color([50, 50, 50, 255]);
    let img6_gray = apply_filter_to_image(&filter6_gray, width, height, fps)?;
    save_image(&img6_gray, &format!("{}/edge_gray_bg.png", tmp_dir))?;

    let filter6_sepia = EdgeDetectFilter::new(30.0, 1.0)
        .with_background_color([255, 240, 200, 255])
        .with_edge_color([80, 60, 40, 255]);
    let img6_sepia = apply_filter_to_image(&filter6_sepia, width, height, fps)?;
    save_image(&img6_sepia, &format!("{}/edge_sepia.png", tmp_dir))?;
    println!("  Saved: edge_gray_bg.png, edge_sepia.png");

    // Example 7: Fine detail mode (low threshold)
    println!("\nExample 7: Fine detail mode (threshold=15, captures more edges)");
    let filter7 = EdgeDetectFilter::new(15.0, 1.0);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/edge_fine_detail.png", tmp_dir))?;
    println!("  Saved: edge_fine_detail.png");

    // Example 8: Bold outline mode (high strength)
    println!("\nExample 8: Bold outline mode (strength=2.0, threshold=40)");
    let filter8 = EdgeDetectFilter::new(40.0, 2.0);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/edge_bold.png", tmp_dir))?;
    println!("  Saved: edge_bold.png");

    // Example 9: Minimal edges (high threshold)
    println!("\nExample 9: Minimal edges (threshold=80, only major contours)");
    let filter9 = EdgeDetectFilter::new(80.0, 1.0);
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/edge_minimal.png", tmp_dir))?;
    println!("  Saved: edge_minimal.png");

    println!("\n==========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - threshold: Edge detection threshold (0-255), higher = fewer edges visible");
    println!("  - strength: Edge intensity (0.0-2.0), controls how strong edges appear");
    println!("  - invert: true = white edges on black, false = black edges on white");
    println!("  - edge_color: RGB color for detected edges");
    println!("  - background_color: RGB color for non-edge areas");

    Ok(())
}