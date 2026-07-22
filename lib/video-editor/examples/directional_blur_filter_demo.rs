// Example demonstrating the directional blur filter
// Generates test images and saves them to tmp/directional_blur_demo directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::DirectionalBlurFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |_, _| Rgba([200, 200, 220, 255]));

    // Add a colored rectangle in center (good for showing blur direction)
    let center_x = width / 2;
    let center_y = height / 2;
    let rect_w = width / 4;
    let rect_h = height / 4;

    for y in (center_y - rect_h/2)..(center_y + rect_h/2) {
        for x in (center_x - rect_w/2)..(center_x + rect_w/2) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([255, 100, 50, 255]));
            }
        }
    }

    // Add some circles to show blur effect
    for i in 0..5 {
        let cx = width / 6 + i * width / 6;
        let cy = height / 2;
        let radius = 25;
        for y in (cy - radius)..(cy + radius) {
            for x in (cx - radius)..(cx + radius) {
                if x < width && y < height {
                    let dx = x as i32 - cx as i32;
                    let dy = y as i32 - cy as i32;
                    if dx * dx + dy * dy < (radius * radius) as i32 {
                        img.put_pixel(x, y, Rgba([50, 150, 255, 255]));
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
    filter: &DirectionalBlurFilter,
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
    println!("Directional Blur Filter Demo");
    println!("============================\n");

    // Create tmp directory
    let tmp_dir = "tmp/directional_blur_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Horizontal blur (angle=0)
    println!("\nExample 1: Horizontal blur (angle=0, length=30)");
    let filter1 = DirectionalBlurFilter::new(0.0, 30.0);
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/blur_horizontal.png", tmp_dir))?;
    println!("  Saved: blur_horizontal.png");

    // Example 2: Vertical blur (angle=90)
    println!("\nExample 2: Vertical blur (angle=90, length=30)");
    let filter2 = DirectionalBlurFilter::new(90.0, 30.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/blur_vertical.png", tmp_dir))?;
    println!("  Saved: blur_vertical.png");

    // Example 3: Diagonal blur (angle=45)
    println!("\nExample 3: Diagonal blur (angle=45, length=30)");
    let filter3 = DirectionalBlurFilter::new(45.0, 30.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/blur_diagonal_45.png", tmp_dir))?;
    println!("  Saved: blur_diagonal_45.png");

    // Example 4: Diagonal blur (angle=-45 / 315)
    println!("\nExample 4: Diagonal blur (angle=315, length=30)");
    let filter4 = DirectionalBlurFilter::new(315.0, 30.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/blur_diagonal_315.png", tmp_dir))?;
    println!("  Saved: blur_diagonal_315.png");

    // Example 5: Motion blur effect (horizontal, longer blur)
    println!("\nExample 5: Motion blur effect (angle=0, length=50)");
    let filter5 = DirectionalBlurFilter::new(0.0, 50.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/blur_motion.png", tmp_dir))?;
    println!("  Saved: blur_motion.png");

    // Example 6: Length comparison
    println!("\nExample 6: Blur length comparison (angle=0)");
    for length in [5, 10, 20, 30, 50, 80] {
        let filter = DirectionalBlurFilter::new(0.0, length as f32);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/blur_length_{}.png", tmp_dir, length))?;
    }
    println!("  Saved: blur_length_5.png through blur_length_80.png");

    // Example 7: Spread comparison
    println!("\nExample 7: Spread comparison (angle=45, length=30)");
    for spread in [0.0, 0.3, 0.5, 0.7, 1.0] {
        let filter = DirectionalBlurFilter::new(45.0, 30.0).with_spread(spread);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/blur_spread_{:.1}.png", tmp_dir, spread))?;
    }
    println!("  Saved: blur_spread_0.0.png through blur_spread_1.0.png");

    // Example 8: Angle comparison (full rotation)
    println!("\nExample 8: Angle comparison (length=25, various angles)");
    for angle in [0, 30, 45, 60, 90, 120, 135, 150, 180] {
        let filter = DirectionalBlurFilter::new(angle as f32, 25.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/blur_angle_{}.png", tmp_dir, angle))?;
    }
    println!("  Saved: blur_angle_0.png through blur_angle_180.png");

    println!("\n============================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - angle (0-360): Blur direction in degrees");
    println!("    - 0 = horizontal right");
    println!("    - 90 = vertical down");
    println!("    - 180 = horizontal left");
    println!("    - 270 = vertical up");
    println!("  - length (0-100): Blur distance/extent");
    println!("  - spread (0-1.0): Blur softness (0 = uniform, 1 = gaussian-like)");

    Ok(())
}