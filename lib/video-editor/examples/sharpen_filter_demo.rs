// Example demonstrating the sharpen filter
// Generates test images and saves them to tmp/sharpen_demo/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::SharpenFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a gradient background (slightly soft)
        let gradient_x = (x as f32 / width as f32 * 180.0) as u8;
        let gradient_y = (y as f32 / height as f32 * 75.0) as u8;
        let r = gradient_x.min(255);
        let g = (gradient_x as u16 + 40).min(255) as u8;
        let b = (gradient_x as u16 + gradient_y as u16 + 40).min(255) as u8;
        Rgba([r, g, b, 255])
    });

    // Add a colored rectangle in center for visual contrast
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
            img.put_pixel(x, y, Rgba([255, 180, 120, 255]));
        }
    }

    // Add sharp edges to better see sharpen effects
    // Top-left corner: sharp edge area
    let edge_end_y = height / 4;
    let edge_end_x = width / 4;
    for y in 0..edge_end_y {
        for x in 0..edge_end_x {
            img.put_pixel(x, y, Rgba([128, 128, 128, 255]));
        }
    }

    // Add circles for edge visualization
    for i in 0..3 {
        let cx = width / 4 + i * width / 4;
        let cy = height / 4;
        let radius = 30;
        for y in (cy - radius)..(cy + radius) {
            for x in (cx - radius)..(cx + radius) {
                if x < width && y < height {
                    let dx = x as i32 - cx as i32;
                    let dy = y as i32 - cy as i32;
                    if dx * dx + dy * dy < (radius * radius) as i32 {
                        let colors: [[u8; 3]; 3] = [[255, 100, 100], [100, 255, 100], [100, 100, 255]];
                        img.put_pixel(x, y, Rgba([colors[i as usize][0], colors[i as usize][1], colors[i as usize][2], 255]));
                    }
                }
            }
        }
    }

    // Add a diagonal line for edge detection
    let line_thickness = 3;
    for i in 0..(width.min(height) as i32) {
        for t in 0..line_thickness {
            let x = i as u32;
            let y = i as u32 + t;
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
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
    filter: &SharpenFilter,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<RgbaImage> {
    let buffer = create_test_image_with_content(width, height);

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
    println!("Sharpen Filter Demo");
    println!("====================\n");

    // Create tmp directory
    let tmp_dir = "tmp/sharpen_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default sharpen
    println!("\nExample 1: Default sharpen (strength=1.0, radius=1.0)");
    let filter1 = SharpenFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/sharpen_default.png", tmp_dir))?;
    println!("  Saved: sharpen_default.png");

    // Example 2: Light sharpen
    println!("\nExample 2: Light sharpen (strength=0.3)");
    let filter2 = SharpenFilter::new(0.3);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/sharpen_light.png", tmp_dir))?;
    println!("  Saved: sharpen_light.png");

    // Example 3: Medium sharpen
    println!("\nExample 3: Medium sharpen (strength=1.5)");
    let filter3 = SharpenFilter::new(1.5);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/sharpen_medium.png", tmp_dir))?;
    println!("  Saved: sharpen_medium.png");

    // Example 4: Heavy sharpen
    println!("\nExample 4: Heavy sharpen (strength=3.0)");
    let filter4 = SharpenFilter::new(3.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/sharpen_heavy.png", tmp_dir))?;
    println!("  Saved: sharpen_heavy.png");

    // Example 5: Very heavy sharpen (may show artifacts)
    println!("\nExample 5: Very heavy sharpen (strength=4.0, shows artifacts)");
    let filter5 = SharpenFilter::new(4.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/sharpen_very_heavy.png", tmp_dir))?;
    println!("  Saved: sharpen_very_heavy.png");

    // Example 6: Strength comparison
    println!("\nExample 6: Strength comparison");
    for strength in [0.1, 0.3, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0] {
        let filter = SharpenFilter::new(strength);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/sharpen_strength_{:.1}.png", tmp_dir, strength))?;
    }
    println!("  Saved: sharpen_strength_0.1.png through sharpen_strength_5.0.png");

    // Example 7: Radius comparison
    println!("\nExample 7: Radius comparison (strength=2.0)");
    for radius in [0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0] {
        let filter = SharpenFilter::new(2.0).with_radius(radius);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/sharpen_radius_{:.1}.png", tmp_dir, radius))?;
    }
    println!("  Saved: sharpen_radius_0.5.png through sharpen_radius_10.0.png");

    // Example 8: Fine detail sharpen
    println!("\nExample 8: Fine detail sharpen (strength=1.0, radius=0.5)");
    let filter8 = SharpenFilter::new(1.0).with_radius(0.5);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/sharpen_fine_detail.png", tmp_dir))?;
    println!("  Saved: sharpen_fine_detail.png");

    // Example 9: Large detail sharpen
    println!("\nExample 9: Large detail sharpen (strength=2.0, radius=5.0)");
    let filter9 = SharpenFilter::new(2.0).with_radius(5.0);
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/sharpen_large_detail.png", tmp_dir))?;
    println!("  Saved: sharpen_large_detail.png");

    // Example 10: Strength/Radius combinations
    println!("\nExample 10: Strength/Radius combinations");
    let combinations = [
        ("subtle", 0.5, 1.0),
        ("normal", 1.0, 1.0),
        ("enhanced", 2.0, 2.0),
        ("crisp", 2.5, 0.5),
        ("soft_enhance", 1.5, 3.0),
        ("aggressive", 3.5, 1.0),
    ];
    for (name, strength, radius) in combinations {
        let filter = SharpenFilter::new(strength).with_radius(radius);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/sharpen_combo_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: sharpen_combo_subtle.png, normal.png, enhanced.png, etc.");

    println!("\n====================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - strength (0-5): Sharpening intensity");
    println!("    - 0.5-1.0: Subtle, natural sharpening");
    println!("    - 2.0-3.0: Noticeable edge enhancement");
    println!("    - 4.0-5.0: Strong sharpening, may show artifacts");
    println!("  - radius (0-10): Detail size to enhance");
    println!("    - 0.5-1.0: Fine details (small edges)");
    println!("    - 2.0-3.0: Medium details");
    println!("    - 5.0-10.0: Large details (broader edges)");
    println!("\nNote: Uses Unsharp Mask technique: original + strength * (original - blurred)");

    Ok(())
}