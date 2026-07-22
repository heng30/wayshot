// Example demonstrating the mirror mask filter
// Generates test images and saves them to tmp/mirror_mask_demo directory

use image::{Rgba, RgbaImage};
use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    Result,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    filters::video::MirrorMaskFilter,
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, _y| {
        let t = x as f32 / width as f32;
        let r = (50.0 + 200.0 * t) as u8;
        let b = (200.0 - 150.0 * t) as u8;
        Rgba([r, 100, b, 255])
    });

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

    for y in 40..120 {
        for x in 40..160 {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([50, 200, 50, 255]));
            }
        }
    }

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
    filter: &MirrorMaskFilter,
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
    println!("Mirror Mask Filter Demo");
    println!("========================\n");

    let tmp_dir = "tmp/mirror_mask_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default mask (center, width=0.5, height=0.5)
    println!("\nExample 1: Default mask (center, width=0.5, height=0.5)");
    let filter1 = MirrorMaskFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/mask_default.png", tmp_dir))?;
    println!("  Saved: mask_default.png");

    // Example 2: Narrow band
    println!("\nExample 2: Narrow band (width=0.2, height=0.5)");
    let filter2 = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.0, 1.0, 0.2, 0.5);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/mask_narrow.png", tmp_dir))?;
    println!("  Saved: mask_narrow.png");

    // Example 3: Wide band
    println!("\nExample 3: Wide band (width=0.8, height=0.5)");
    let filter3 = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.0, 1.0, 0.8, 0.5);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/mask_wide.png", tmp_dir))?;
    println!("  Saved: mask_wide.png");

    // Example 4: Short band
    println!("\nExample 4: Short band (width=0.5, height=0.3)");
    let filter4 = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.0, 1.0, 0.5, 0.3);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/mask_short.png", tmp_dir))?;
    println!("  Saved: mask_short.png");

    // Example 5: Offset center
    println!("\nExample 5: Offset center (center at 0.3, 0.4)");
    let filter5 = MirrorMaskFilter::new(0.3, 0.4, 0.0, 0.0, 1.0, 0.4, 0.4);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/mask_offset_center.png", tmp_dir))?;
    println!("  Saved: mask_offset_center.png");

    // Example 6: Rotated band (45 degrees)
    println!("\nExample 6: Rotated band (rotation=45)");
    let filter6 = MirrorMaskFilter::new(0.5, 0.5, 45.0, 0.0, 1.0, 0.3, 0.5);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/mask_rotated_45.png", tmp_dir))?;
    println!("  Saved: mask_rotated_45.png");

    // Example 7: Feathered edge
    println!("\nExample 7: Feathered edge (feather=0.1)");
    let filter7 = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.1, 1.0, 0.5, 0.5);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/mask_feathered.png", tmp_dir))?;
    println!("  Saved: mask_feathered.png");

    // Example 8: Partial opacity
    println!("\nExample 8: Partial opacity (opacity=0.5)");
    let filter8 = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.0, 0.5, 0.5, 0.5);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/mask_partial_opacity.png", tmp_dir))?;
    println!("  Saved: mask_partial_opacity.png");

    // Example 9: Flipped mask (band visible, outside masked)
    println!("\nExample 9: Flipped mask (flip=true, band visible, outside masked)");
    let mut filter9 = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.0, 1.0, 0.5, 0.5);
    filter9.flip = true;
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/mask_flipped.png", tmp_dir))?;
    println!("  Saved: mask_flipped.png");

    // Example 10: Rotation comparison
    println!("\nExample 10: Rotation comparison (various angles)");
    for rotation in [0, 30, 45, 60, 90, 120, 150] {
        let filter = MirrorMaskFilter::new(0.5, 0.5, rotation as f32, 0.05, 1.0, 0.3, 0.5);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/mask_rotation_{}.png", tmp_dir, rotation))?;
    }
    println!("  Saved: mask_rotation_0.png through mask_rotation_150.png");

    // Example 11: Width comparison
    println!("\nExample 11: Width comparison (0.1 to 0.8)");
    for w in [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8] {
        let filter = MirrorMaskFilter::new(0.5, 0.5, 0.0, 0.0, 1.0, w, 0.5);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/mask_width_{:.1}.png", tmp_dir, w))?;
    }
    println!("  Saved: mask_width_0.1.png through mask_width_0.8.png");

    // Example 12: Feather comparison
    println!("\nExample 12: Feather comparison (0.0 to 0.3)");
    for feather in [0.0, 0.05, 0.1, 0.15, 0.2, 0.3] {
        let filter = MirrorMaskFilter::new(0.5, 0.5, 0.0, feather, 1.0, 0.5, 0.5);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/mask_feather_{:.2}.png", tmp_dir, feather))?;
    }
    println!("  Saved: mask_feather_0.00.png through mask_feather_0.30.png");

    // Example 13: Combined — rotation + feather + opacity + flip
    println!("\nExample 13: Combined (rotation=30, feather=0.15, opacity=0.7, flip=true)");
    let mut filter13 = MirrorMaskFilter::new(0.5, 0.5, 30.0, 0.15, 0.7, 0.3, 0.5);
    filter13.flip = true;
    let img13 = apply_filter_to_image(&filter13, width, height, fps)?;
    save_image(&img13, &format!("{}/mask_combined.png", tmp_dir))?;
    println!("  Saved: mask_combined.png");

    println!("\n========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - center_x/center_y (0.0-1.0): Position of the band center");
    println!("  - rotation (0.0-360.0): Rotation angle in degrees");
    println!("  - feather (0.0-1.0): Edge softness (0 = hard edge, larger = smooth transition)");
    println!("  - opacity (0.0-1.0): Mask strength (0 = no masking, 1 = fully transparent masked area)");
    println!("  - width (0.0-1.0): Band width as fraction of frame width");
    println!("  - height (0.0-1.0): Band height as fraction of frame height");
    println!("  - flip (bool): Reverse mask direction (default: band masked, outside visible)");

    Ok(())
}
