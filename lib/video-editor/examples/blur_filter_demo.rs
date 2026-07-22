// Example demonstrating the Gaussian blur filter
// Generates test images and saves them to tmp/blur_demo/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::GaussianBlurFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a gradient background
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

    // Add sharp edges to better see blur effects
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
    filter: &GaussianBlurFilter,
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
    println!("Gaussian Blur Filter Demo");
    println!("==========================\n");

    // Create tmp directory
    let tmp_dir = "tmp/blur_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default blur
    println!("\nExample 1: Default Gaussian blur (radius=5.0, sigma=2.0)");
    let filter1 = GaussianBlurFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/blur_default.png", tmp_dir))?;
    println!("  Saved: blur_default.png");

    // Example 2: Light blur
    println!("\nExample 2: Light Gaussian blur (radius=3.0)");
    let filter2 = GaussianBlurFilter::new(3.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/blur_light.png", tmp_dir))?;
    println!("  Saved: blur_light.png");

    // Example 3: Medium blur
    println!("\nExample 3: Medium Gaussian blur (radius=10.0)");
    let filter3 = GaussianBlurFilter::new(10.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/blur_medium.png", tmp_dir))?;
    println!("  Saved: blur_medium.png");

    // Example 4: Heavy blur
    println!("\nExample 4: Heavy Gaussian blur (radius=20.0)");
    let filter4 = GaussianBlurFilter::new(20.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/blur_heavy.png", tmp_dir))?;
    println!("  Saved: blur_heavy.png");

    // Example 5: Very heavy blur
    println!("\nExample 5: Very heavy Gaussian blur (radius=30.0)");
    let filter5 = GaussianBlurFilter::new(30.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/blur_very_heavy.png", tmp_dir))?;
    println!("  Saved: blur_very_heavy.png");

    // Example 6: Radius comparison
    println!("\nExample 6: Radius comparison");
    for radius in [2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 40.0] {
        let filter = GaussianBlurFilter::new(radius);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/blur_radius_{:.0}.png", tmp_dir, radius))?;
    }
    println!("  Saved: blur_radius_2.png through blur_radius_40.png");

    // Example 7: Sigma comparison (fixed radius)
    println!("\nExample 7: Sigma comparison (radius=10.0)");
    for sigma in [0.5, 1.0, 2.0, 5.0, 8.0, 10.0] {
        let filter = GaussianBlurFilter::new(10.0).with_sigma(sigma);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/blur_sigma_{:.1}.png", tmp_dir, sigma))?;
    }
    println!("  Saved: blur_sigma_0.5.png through blur_sigma_10.0.png");

    // Example 8: Radius/Sigma combinations
    println!("\nExample 8: Radius/Sigma combinations");
    let combinations = [
        ("tight", 5.0, 1.0),   // Tight blur, sharp edges
        ("normal", 10.0, 3.0), // Normal blur
        ("wide", 15.0, 5.0),   // Wide blur
        ("soft", 20.0, 8.0),   // Soft, smooth blur
        ("extreme", 40.0, 15.0), // Extreme blur
    ];
    for (name, radius, sigma) in combinations {
        let filter = GaussianBlurFilter::new(radius).with_sigma(sigma);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/blur_combo_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: blur_combo_tight.png, normal.png, wide.png, soft.png, extreme.png");

    // Example 9: Subtle blur (for smoothing)
    println!("\nExample 9: Subtle blur for smoothing (radius=1.5, sigma=0.5)");
    let filter9 = GaussianBlurFilter::new(1.5).with_sigma(0.5);
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/blur_subtle.png", tmp_dir))?;
    println!("  Saved: blur_subtle.png");

    // Example 10: Dreamy/soft focus effect
    println!("\nExample 10: Dreamy soft focus effect (radius=25, sigma=10)");
    let filter10 = GaussianBlurFilter::new(25.0).with_sigma(10.0);
    let img10 = apply_filter_to_image(&filter10, width, height, fps)?;
    save_image(&img10, &format!("{}/blur_dreamy.png", tmp_dir))?;
    println!("  Saved: blur_dreamy.png");

    println!("\n==========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - radius (0-50): Kernel size/blur extent, larger = stronger blur");
    println!("  - sigma (0.1-20): Gaussian distribution standard deviation");
    println!("    - Low sigma: sharper, more defined blur edges");
    println!("    - High sigma: softer, smoother blur transition");
    println!("\nNote: For directional blur (motion blur), see directional_blur_filter_demo.rs");

    Ok(())
}