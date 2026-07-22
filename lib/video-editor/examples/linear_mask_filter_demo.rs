// Example demonstrating the linear mask filter
// Generates test images and saves them to tmp/linear_mask_demo directory

use image::{Rgba, RgbaImage};
use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    Result,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    filters::video::LinearMaskFilter,
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, _y| {
        // Gradient background: blue left, red right
        let t = x as f32 / width as f32;
        let r = (50.0 + 200.0 * t) as u8;
        let b = (200.0 - 150.0 * t) as u8;
        Rgba([r, 100, b, 255])
    });

    // Add a bright yellow circle in the center
    let cx = width / 2;
    let cy = height / 2;
    let radius = 60;
    for y in (cy - radius)..(cy + radius) {
        for x in (cx - radius)..(cx + radius) {
            if x < width && y < height {
                let dx = x as i32 - cx as i32;
                let dy = y as i32 - cy as i32;
                if dx * dx + dy * dy < (radius * radius) as i32 {
                    img.put_pixel(x, y, Rgba([255, 230, 50, 255]));
                }
            }
        }
    }

    // Add a green rectangle in the top-left
    for y in 40..120 {
        for x in 40..160 {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([50, 200, 50, 255]));
            }
        }
    }

    // Add a purple rectangle in the bottom-right
    for y in (height - 120)..(height - 40) {
        for x in (width - 160)..(width - 40) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([150, 50, 200, 255]));
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
    filter: &LinearMaskFilter,
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
        Err(video_editor::Error::InvalidConfig(
            "No image generated".into(),
        ))
    }
}

fn save_image(image: &RgbaImage, path: &str) -> Result<()> {
    image
        .save(path)
        .map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

fn main() -> Result<()> {
    println!("Linear Mask Filter Demo");
    println!("========================\n");

    let tmp_dir = "tmp/linear_mask_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default mask (vertical line at center, left side masked)
    println!("\nExample 1: Default mask (center, left side masked, no feather)");
    let filter1 = LinearMaskFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/mask_default.png", tmp_dir))?;
    println!("  Saved: mask_default.png");

    // Example 2: Horizontal mask (rotation=90, top side masked)
    println!("\nExample 2: Horizontal mask (rotation=90, top masked)");
    let filter2 = LinearMaskFilter::new(0.5, 0.5, 90.0, 0.0, 1.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/mask_horizontal.png", tmp_dir))?;
    println!("  Saved: mask_horizontal.png");

    // Example 3: Diagonal mask (rotation=45)
    println!("\nExample 3: Diagonal mask (rotation=45)");
    let filter3 = LinearMaskFilter::new(0.5, 0.5, 45.0, 0.0, 1.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/mask_diagonal_45.png", tmp_dir))?;
    println!("  Saved: mask_diagonal_45.png");

    // Example 4: Flipped mask (right side visible instead of left)
    println!("\nExample 4: Flipped mask (flip=true, right side visible)");
    let filter4 = LinearMaskFilter::default().with_flip(true);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/mask_flipped.png", tmp_dir))?;
    println!("  Saved: mask_flipped.png");

    // Example 5: Offset center mask
    println!("\nExample 5: Offset center mask (center at 0.3, 0.4)");
    let filter5 = LinearMaskFilter::new(0.3, 0.4, 0.0, 0.0, 1.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/mask_offset_center.png", tmp_dir))?;
    println!("  Saved: mask_offset_center.png");

    // Example 6: Feathered edge (soft transition)
    println!("\nExample 6: Feathered edge (feather=0.15)");
    let filter6 = LinearMaskFilter::new(0.5, 0.5, 0.0, 0.15, 1.0);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/mask_feathered.png", tmp_dir))?;
    println!("  Saved: mask_feathered.png");

    // Example 7: Partial opacity (semi-transparent mask)
    println!("\nExample 7: Partial opacity (opacity=0.5)");
    let filter7 = LinearMaskFilter::new(0.5, 0.5, 0.0, 0.0, 0.5);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/mask_partial_opacity.png", tmp_dir))?;
    println!("  Saved: mask_partial_opacity.png");

    // Example 8: Feather comparison
    println!("\nExample 8: Feather comparison (0.0 to 0.3)");
    for feather in [0.0, 0.05, 0.1, 0.15, 0.2, 0.3] {
        let filter = LinearMaskFilter::new(0.5, 0.5, 0.0, feather, 1.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(
            &img,
            &format!("{}/mask_feather_{:.2}.png", tmp_dir, feather),
        )?;
    }
    println!("  Saved: mask_feather_0.00.png through mask_feather_0.30.png");

    // Example 9: Opacity comparison
    println!("\nExample 9: Opacity comparison (0.2 to 1.0)");
    for opacity in [0.2, 0.4, 0.6, 0.8, 1.0] {
        let filter = LinearMaskFilter::new(0.5, 0.5, 0.0, 0.1, opacity);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(
            &img,
            &format!("{}/mask_opacity_{:.1}.png", tmp_dir, opacity),
        )?;
    }
    println!("  Saved: mask_opacity_0.2.png through mask_opacity_1.0.png");

    // Example 10: Rotation comparison (full rotation)
    println!("\nExample 10: Rotation comparison (various angles)");
    for rotation in [0, 30, 45, 60, 90, 120, 135, 150, 180] {
        let filter = LinearMaskFilter::new(0.5, 0.5, rotation as f32, 0.05, 1.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/mask_rotation_{}.png", tmp_dir, rotation))?;
    }
    println!("  Saved: mask_rotation_0.png through mask_rotation_180.png");

    // Example 11: Combined — feather + opacity + rotation + flip
    println!("\nExample 11: Combined (rotation=30, feather=0.2, opacity=0.7, flip=true)");
    let filter11 = LinearMaskFilter::new(0.5, 0.5, 30.0, 0.2, 0.7).with_flip(true);
    let img11 = apply_filter_to_image(&filter11, width, height, fps)?;
    save_image(&img11, &format!("{}/mask_combined.png", tmp_dir))?;
    println!("  Saved: mask_combined.png");

    println!("\n========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - center_x/center_y (0.0-1.0): Position of the mask line center");
    println!("  - rotation (0.0-360.0): Angle of the mask line in degrees");
    println!("    - 0 = vertical line (left/right split)");
    println!("    - 90 = horizontal line (top/bottom split)");
    println!("  - feather (0.0-1.0): Edge softness (0 = hard edge, larger = smooth transition)");
    println!(
        "  - opacity (0.0-1.0): Mask strength (0 = no masking, 1 = fully transparent masked side)"
    );
    println!("  - flip (bool): Reverse which side is masked (default: left/top side masked)");

    Ok(())
}

