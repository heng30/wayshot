// Example demonstrating the fly-in filter
// Generates test images and saves them to tmp/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::FlyInFilter,
    filters::traits::{EasingFunction, VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> RgbaImage {
    RgbaImage::from_fn(width, height, |_, _| Rgba([color.0, color.1, color.2, color.3]))
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
    filter: &FlyInFilter,
    width: u32,
    height: u32,
    fps: f32,
    background_color: (u8, u8, u8, u8),
    frame_time: Duration,
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height, background_color);

    // Add some visual content to the image (a colored rectangle)
    let mut img_with_content = buffer.clone();
    for y in 50..250u32 {
        for x in 50..250u32 {
            if x < img_with_content.width() && y < img_with_content.height() {
                img_with_content.put_pixel(x, y, Rgba([255, 100, 50, 255]));
            }
        }
    }

    // Add a border rectangle
    for y in 40..260u32 {
        for x in 40..260u32 {
            if x < img_with_content.width() && y < img_with_content.height() {
                if x < 45 || x > 254 || y < 45 || y > 254 {
                    img_with_content.put_pixel(x, y, Rgba([100, 200, 255, 255]));
                }
            }
        }
    }

    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image {
            buffer: img_with_content,
        }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: frame_time,
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
    println!("Fly-In Filter Demo");
    println!("==================\n");

    // Create tmp directory
    let tmp_dir = "tmp";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Fly-in to center (nearest edge = all edges, picks left)
    // Target (0.5, 0.5): equal distance to all edges, picks first (left)
    println!("Example 1: Fly-in to center with linear easing");
    let filter1 = FlyInFilter::new(Duration::from_secs(2), (0.5, 0.5))
        .with_easing(EasingFunction::Linear);

    let anim_dir1 = format!("{}/flyin_center_linear", tmp_dir);
    fs::create_dir_all(&anim_dir1)?;

    for i in 0..15 {
        let frame_time = Duration::from_millis(i * 133); // 2 seconds total
        let img = apply_filter_to_image(&filter1, width, height, fps, (40, 40, 50, 255), frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir1, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 15 frames to: {}/", anim_dir1);

    // Example 2: Fly-in to right side (nearest edge = right)
    // Target (0.7, 0.5): right edge distance = 0.3, nearest edge is right
    println!("\nExample 2: Fly-in from right (target at 0.7, 0.5)");
    let filter2 = FlyInFilter::new(Duration::from_secs(2), (0.7, 0.5))
        .with_easing(EasingFunction::EaseOut);

    let anim_dir2 = format!("{}/flyin_from_right", tmp_dir);
    fs::create_dir_all(&anim_dir2)?;

    for i in 0..15 {
        let frame_time = Duration::from_millis(i * 133);
        let img = apply_filter_to_image(&filter2, width, height, fps, (40, 40, 50, 255), frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir2, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 15 frames to: {}/", anim_dir2);

    // Example 3: Fly-in from bottom (target at 0.5, 0.8)
    // Target (0.5, 0.8): bottom edge distance = 0.2, nearest edge is bottom
    println!("\nExample 3: Fly-in from bottom (target at 0.5, 0.8)");
    let filter3 = FlyInFilter::new(Duration::from_secs(2), (0.5, 0.8))
        .with_easing(EasingFunction::EaseOut);

    let anim_dir3 = format!("{}/flyin_from_bottom", tmp_dir);
    fs::create_dir_all(&anim_dir3)?;

    for i in 0..15 {
        let frame_time = Duration::from_millis(i * 133);
        let img = apply_filter_to_image(&filter3, width, height, fps, (40, 40, 50, 255), frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir3, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 15 frames to: {}/", anim_dir3);

    // Example 4: Fly-in from top-left corner direction
    // Target (0.2, 0.2): left edge distance = 0.2, top edge distance = 0.2
    // Picks first (left) as they're equal
    println!("\nExample 4: Fly-in to top-left area (target at 0.2, 0.2)");
    let filter4 = FlyInFilter::new(Duration::from_secs(2), (0.2, 0.2))
        .with_easing(EasingFunction::EaseInOut);

    let anim_dir4 = format!("{}/flyin_top_left", tmp_dir);
    fs::create_dir_all(&anim_dir4)?;

    for i in 0..15 {
        let frame_time = Duration::from_millis(i * 133);
        let img = apply_filter_to_image(&filter4, width, height, fps, (40, 40, 50, 255), frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir4, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 15 frames to: {}/", anim_dir4);

    // Example 5: Comparison of all easing functions (fly-in to center)
    println!("\nExample 5: Comparison of all easing functions");
    let comparison_dir = format!("{}/flyin_comparison", tmp_dir);
    fs::create_dir_all(&comparison_dir)?;

    let easing_functions: Vec<(&str, EasingFunction)> = vec![
        ("Linear", EasingFunction::Linear),
        ("EaseIn", EasingFunction::EaseIn),
        ("EaseOut", EasingFunction::EaseOut),
        ("EaseInOut", EasingFunction::EaseInOut),
    ];

    for (name, easing) in easing_functions {
        let filter = FlyInFilter::new(Duration::from_secs(2), (0.5, 0.5))
            .with_easing(easing);

        let easing_dir = format!("{}/{}", comparison_dir, name.to_lowercase());
        fs::create_dir_all(&easing_dir)?;

        for i in 0..15 {
            let frame_time = Duration::from_millis(i * 133);
            let img = apply_filter_to_image(&filter, width, height, fps, (30, 30, 40, 255), frame_time)?;
            let frame_path = format!("{}/frame_{:02}.png", easing_dir, i);
            save_image(&img, &frame_path)?;
        }
        println!("  Saved {} frames to: {}/{}", 15, comparison_dir, name);
    }

    // Example 6: Fast fly-in (short duration)
    println!("\nExample 6: Fast fly-in (0.5 second duration)");
    let filter6 = FlyInFilter::new(Duration::from_millis(500), (0.3, 0.3))
        .with_easing(EasingFunction::EaseOut);

    let anim_dir6 = format!("{}/flyin_fast", tmp_dir);
    fs::create_dir_all(&anim_dir6)?;

    // 500ms total, 8 frames at ~62.5ms intervals
    for i in 0..8 {
        let frame_time = Duration::from_millis(i * 62);
        let img = apply_filter_to_image(&filter6, width, height, fps, (50, 50, 60, 255), frame_time)?;
        let frame_path = format!("{}/frame_{:02}.png", anim_dir6, i);
        save_image(&img, &frame_path)?;
    }
    println!("  Saved 8 frames to: {}/", anim_dir6);

    println!("\n==================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nAnimation sequences:");
    println!("  - {}/flyin_center_linear/ - Fly-in to center (Linear)", tmp_dir);
    println!("  - {}/flyin_from_right/ - Fly-in from right edge (EaseOut)", tmp_dir);
    println!("  - {}/flyin_from_bottom/ - Fly-in from bottom edge (EaseOut)", tmp_dir);
    println!("  - {}/flyin_top_left/ - Fly-in to top-left (EaseInOut)", tmp_dir);
    println!("  - {}/flyin_comparison/ - All easing functions compared", tmp_dir);
    println!("  - {}/flyin_fast/ - Fast 0.5s fly-in", tmp_dir);
    println!("\nFly-in Logic:");
    println!("- Image flies in from the nearest edge to the target position");
    println!("- Target at (0.7, 0.5) -> nearest edge is right (dist 0.3)");
    println!("- Target at (0.2, 0.8) -> nearest edge is bottom (dist 0.2)");
    println!("- Target at (0.5, 0.5) -> all edges equal, picks left first");
    println!("\nEasing Tips:");
    println!("- Linear: Constant speed, mechanical feel");
    println!("- EaseIn: Slow start, fast end - dramatic entrance");
    println!("- EaseOut: Fast start, slow end - natural landing feel");
    println!("- EaseInOut: Slow start and end - smooth, polished feel");

    Ok(())
}