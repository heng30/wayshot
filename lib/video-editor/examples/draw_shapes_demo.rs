use image::{Rgba, RgbaImage};
use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    Result,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    filters::video::{DrawCircleFilter, DrawRectangleFilter},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
};

fn create_test_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> RgbaImage {
    RgbaImage::from_fn(width, height, |_, _| {
        Rgba([color.0, color.1, color.2, color.3])
    })
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
    filter: &dyn VideoFilter,
    width: u32,
    height: u32,
    fps: f32,
    background_color: (u8, u8, u8, u8),
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height, background_color);

    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image {
            buffer,
        }],
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
    println!("Draw Shapes Filters Demo");
    println!("=======================\n");

    // Create tmp directory
    let tmp_dir = "tmp";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Simple rectangle with border
    println!("Example 1: Simple rectangle with border");
    let rect1 = DrawRectangleFilter::new(0.15, 0.2, 0.31, 0.31)  // Normalized: ~200px width, ~150px height at 640x480
        .with_border_color(Some((255, 0, 0, 255))) // Red border
        .with_border_width(3);
    let img1 = apply_filter_to_image(&rect1, width, height, fps, (30, 30, 30, 255))?;
    save_image(&img1, &format!("{}/example1_rectangle_border.png", tmp_dir))?;
    println!("  Saved: {}/example1_rectangle_border.png", tmp_dir);

    // Example 2: Filled rounded rectangle
    println!("\nExample 2: Filled rounded rectangle");
    let rect2 = DrawRectangleFilter::new(0.08, 0.1, 0.47, 0.42)  // Normalized: ~300px width, ~200px height at 640x480
        .with_fill_color(Some((0, 100, 255, 200))) // Semi-transparent blue fill
        .with_border_color(Some((255, 255, 255, 255))) // White border
        .with_border_width(2)
        .with_corner_radius(20); // 20px rounded corners
    let img2 = apply_filter_to_image(&rect2, width, height, fps, (30, 30, 30, 255))?;
    save_image(
        &img2,
        &format!("{}/example2_rounded_rectangle.png", tmp_dir),
    )?;
    println!("  Saved: {}/example2_rounded_rectangle.png", tmp_dir);

    // Example 3: Simple circle with border
    println!("\nExample 3: Simple circle with border");
    let circle1 = DrawCircleFilter::new(0.625, 0.625, 80)
        .with_border_color(Some((0, 255, 0, 255))) // Green border
        .with_border_width(3);
    let img3 = apply_filter_to_image(&circle1, width, height, fps, (30, 30, 30, 255))?;
    save_image(&img3, &format!("{}/example3_circle_border.png", tmp_dir))?;
    println!("  Saved: {}/example3_circle_border.png", tmp_dir);

    // Example 4: Filled circle
    println!("\nExample 4: Filled circle");
    let circle2 = DrawCircleFilter::new(0.31, 0.42, 100)
        .with_fill_color(Some((255, 100, 0, 180))) // Semi-transparent orange
        .with_border_color(Some((255, 255, 255, 255))) // White border
        .with_border_width(2);
    let img4 = apply_filter_to_image(&circle2, width, height, fps, (30, 30, 30, 255))?;
    save_image(&img4, &format!("{}/example4_filled_circle.png", tmp_dir))?;
    println!("  Saved: {}/example4_filled_circle.png", tmp_dir);

    // Example 5: Multiple shapes on one image
    println!("\nExample 5: Multiple shapes on one image");
    let img5 = create_test_image(width, height, (20, 20, 40, 255));

    // Apply multiple filters to the same image
    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image {
            buffer: img5,
        }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: Duration::ZERO,
    };

    // Apply red rectangle
    let red_rect = DrawRectangleFilter::new(0.08, 0.1, 0.23, 0.21)  // Normalized: ~150px width, ~100px height
        .with_fill_color(Some((255, 0, 0, 200)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2)
        .with_corner_radius(10);
    red_rect.apply(&mut video_data)?;

    // Apply blue circle
    let blue_circle = DrawCircleFilter::new(0.7, 0.15, 60)  // Normalized
        .with_fill_color(Some((0, 100, 255, 200)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2);
    blue_circle.apply(&mut video_data)?;

    // Apply green rounded rectangle
    let green_rect = DrawRectangleFilter::new(0.23, 0.42, 0.39, 0.31)  // Normalized: ~250px width, ~150px height
        .with_fill_color(Some((0, 255, 0, 180)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(3)
        .with_corner_radius(25);
    green_rect.apply(&mut video_data)?;

    // Apply yellow hollow circle
    let yellow_circle = DrawCircleFilter::new(0.78, 0.73, 70)  // Normalized
        .with_border_color(Some((255, 255, 0, 255)))
        .with_border_width(4);
    yellow_circle.apply(&mut video_data)?;

    if let Some(VideoImage::Image { buffer, .. }) = video_data.frames.first() {
        save_image(buffer, &format!("{}/example5_multiple_shapes.png", tmp_dir))?;
        println!("  Saved: {}/example5_multiple_shapes.png", tmp_dir);
    }

    // Example 6: Outline only shapes (hollow)
    println!("\nExample 6: Outline only shapes (hollow)");
    let img6 = create_test_image(width, height, (20, 20, 20, 255));
    let mut video_data6 = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image {
            buffer: img6,
        }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: Duration::ZERO,
    };

    let hollow_rect = DrawRectangleFilter::new(0.15, 0.15, 0.69, 0.31)  // Normalized: ~440px width, ~150px height
        .with_border_color(Some((0, 255, 255, 255))) // Cyan
        .with_border_width(3)
        .with_corner_radius(15);
    hollow_rect.apply(&mut video_data6)?;

    let hollow_circle = DrawCircleFilter::new(0.5, 0.73, 80)  // Normalized
        .with_border_color(Some((255, 0, 255, 255))) // Magenta
        .with_border_width(4);
    hollow_circle.apply(&mut video_data6)?;

    if let Some(VideoImage::Image { buffer, .. }) = video_data6.frames.first() {
        save_image(buffer, &format!("{}/example6_hollow_shapes.png", tmp_dir))?;
        println!("  Saved: {}/example6_hollow_shapes.png", tmp_dir);
    }

    // Example 7: Rotated rectangle (45 degrees)
    println!("\nExample 7: Rotated rectangle (45 degrees)");
    let rotated_rect = DrawRectangleFilter::new(0.5, 0.5, 0.31, 0.21)  // Centered: ~200px width, ~100px height at 640x480
        .with_fill_color(Some((255, 100, 50, 200))) // Orange fill
        .with_border_color(Some((255, 255, 255, 255))) // White border
        .with_border_width(3)
        .with_corner_radius(10);
    let img7 = apply_filter_to_image(&rotated_rect, width, height, fps, (30, 30, 30, 255))?;
    save_image(&img7, &format!("{}/example7_rotated_45deg.png", tmp_dir))?;
    println!("  Saved: {}/example7_rotated_45deg.png", tmp_dir);

    // Example 8: Rotated rounded rectangle (30 degrees)
    println!("\nExample 8: Rotated rounded rectangle (30 degrees)");
    let rotated_rounded = DrawRectangleFilter::new(0.25, 0.35, 0.39, 0.31)  // ~250px width, ~150px height
        .with_fill_color(Some((50, 150, 255, 230))) // Blue fill
        .with_border_color(Some((255, 255, 255, 255))) // White border
        .with_border_width(2)
        .with_corner_radius(25);
    let img8 = apply_filter_to_image(&rotated_rounded, width, height, fps, (40, 40, 40, 255))?;
    save_image(&img8, &format!("{}/example8_rotated_rounded.png", tmp_dir))?;
    println!("  Saved: {}/example8_rotated_rounded.png", tmp_dir);

    // Example 9: Anti-aliasing demonstration
    println!("\nExample 9: Anti-aliasing demonstration");
    let img9 = create_test_image(width, height, (255, 255, 255, 255));
    let mut video_data9 = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image {
            buffer: img9,
        }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: Duration::ZERO,
    };

    // Circle with thin border to show anti-aliasing
    let aa_circle = DrawCircleFilter::new(0.5, 0.5, 120)  // Normalized
        .with_border_color(Some((0, 0, 0, 255)))
        .with_border_width(2);
    aa_circle.apply(&mut video_data9)?;

    let aa_rect = DrawRectangleFilter::new(0.17, 0.25, 0.53, 0.38)  // Normalized: ~340px width, ~180px height
        .with_fill_color(Some((100, 150, 200, 255)))
        .with_border_color(Some((255, 0, 0, 255)))
        .with_border_width(3)
        .with_corner_radius(30);
    aa_rect.apply(&mut video_data9)?;

    if let Some(VideoImage::Image { buffer, .. }) = video_data9.frames.first() {
        save_image(buffer, &format!("{}/example9_antialiasing.png", tmp_dir))?;
        println!("  Saved: {}/example9_antialiasing.png", tmp_dir);
    }

    // Example 10: Large corner radius (pill shape) with rounded border
    println!("\nExample 10: Large corner radius (pill shape) with border");
    let pill_rect = DrawRectangleFilter::new(0.2, 0.35, 0.47, 0.21)  // Normalized: ~300px width, ~100px height
        .with_fill_color(Some((0, 200, 100, 230))) // Green
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2)
        .with_corner_radius(50); // Half the height = pill shape
    let img10 = apply_filter_to_image(&pill_rect, width, height, fps, (30, 30, 30, 255))?;
    save_image(&img10, &format!("{}/example10_pill_shape.png", tmp_dir))?;
    println!("  Saved: {}/example10_pill_shape.png", tmp_dir);

    // Example 11: Animation frames - Rotation animation
    println!("\nExample 11: Animation frames - Rotation animation");
    let anim_dir3 = format!("{}/animation_rotation", tmp_dir);
    fs::create_dir_all(&anim_dir3)?;

    for i in 0..12 {
        let img = create_test_image(width, height, (30, 30, 30, 255));

        let rotating_rect = DrawRectangleFilter::new(0.5, 0.5, 0.28, 0.17) // Centered: ~180px width, ~80px height
            .with_fill_color(Some((100, 200, 100, 230))) // Green fill
            .with_border_color(Some((255, 255, 255, 255))) // White border
            .with_border_width(2)
            .with_corner_radius(10);

        let mut video_data = VideoData {
            config: VideoFilterConfig::new(width, height, fps),
            frames: vec![VideoImage::Image {
                buffer: img,
            }],
            from_segment: create_dummy_segment(),
            relative_timeline_offset: Duration::ZERO,
        };

        rotating_rect.apply(&mut video_data)?;

        if let Some(VideoImage::Image { buffer, .. }) = video_data.frames.first() {
            let frame_path = format!("{}/frame_{:02}.png", anim_dir3, i);
            save_image(buffer, &frame_path)?;
        }
    }
    println!("  Saved 12 frames to: {}/", anim_dir3);

    // Example 12: Multiple rotated rectangles at different angles
    println!("\nExample 12: Multiple rotated rectangles at different angles");
    let img12 = create_test_image(width, height, (25, 25, 35, 255));
    let mut video_data12 = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image {
            buffer: img12,
        }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: Duration::ZERO,
    };

    // Red rectangle at 0 degrees
    let rect_0deg = DrawRectangleFilter::new(0.15, 0.15, 0.19, 0.12)  // Normalized: ~120px width, ~60px height
        .with_fill_color(Some((255, 50, 50, 200)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2)
        .with_corner_radius(8);
    rect_0deg.apply(&mut video_data12)?;

    // Blue rectangle at 15 degrees
    let rect_15deg = DrawRectangleFilter::new(0.35, 0.25, 0.19, 0.12)  // Normalized: ~120px width, ~60px height
        .with_fill_color(Some((50, 50, 255, 200)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2)
        .with_corner_radius(8);
    rect_15deg.apply(&mut video_data12)?;

    // Green rectangle at 30 degrees
    let rect_30deg = DrawRectangleFilter::new(0.55, 0.35, 0.19, 0.12)  // Normalized: ~120px width, ~60px height
        .with_fill_color(Some((50, 255, 50, 200)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2)
        .with_corner_radius(8);
    rect_30deg.apply(&mut video_data12)?;

    // Yellow rectangle at 45 degrees
    let rect_45deg = DrawRectangleFilter::new(0.75, 0.45, 0.19, 0.12)  // Normalized: ~120px width, ~60px height
        .with_fill_color(Some((255, 255, 50, 200)))
        .with_border_color(Some((255, 255, 255, 255)))
        .with_border_width(2)
        .with_corner_radius(8);
    rect_45deg.apply(&mut video_data12)?;

    if let Some(VideoImage::Image { buffer, .. }) = video_data12.frames.first() {
        save_image(buffer, &format!("{}/example12_multi_rotation.png", tmp_dir))?;
        println!("  Saved: {}/example12_multi_rotation.png", tmp_dir);
    }

    println!("\n=======================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("Animation sequences:");
    println!("  - {}/animation_rotation/ - Rotation animation (0-330 degrees)", tmp_dir);
    println!("\nUsage Tips:");
    println!("- Set fill_color to None for hollow/outline-only shapes");
    println!("- Set border_color to None for shapes without borders");
    println!("- Use corner_radius > 0 for rounded rectangles");
    println!("- Use corner_radius = height/2 for pill/capsule shapes");
    println!("- Use rotation (in radians) to rotate rectangles");
    println!("- Positive rotation = clockwise, Negative = counter-clockwise");
    println!("- Anti-aliasing is applied automatically for smooth edges");

    Ok(())
}
