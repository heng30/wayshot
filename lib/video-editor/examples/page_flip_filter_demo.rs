// Example demonstrating the page flip filter using the turn-rs library.
// Simulates a page being turned from various corners, revealing the image underneath.
// Generates test images and saves them to tmp/page_flip_demo directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{PageFlipAxis, PageFlipCorner, PageFlipDirection, PageFlipFilter, PageFlipPosition},
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a colorful test image with geometric shapes to visualize the page flip effect.
fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    // Dark background
    let mut img = RgbaImage::from_fn(width, height, |_, _| Rgba([30, 30, 45, 255]));

    // Draw a grid pattern to show the flip clearly
    let grid_size = 40u32;
    for y in 0..height {
        for x in 0..width {
            if x % grid_size == 0 || y % grid_size == 0 {
                img.put_pixel(x, y, Rgba([60, 60, 80, 255]));
            }
        }
    }

    // Draw a large colored rectangle in the center
    let rect_w = (width as f32 * 0.4) as u32;
    let rect_h = (height as f32 * 0.4) as u32;
    let rect_x = (width - rect_w) / 2;
    let rect_y = (height - rect_h) / 2;

    for y in rect_y..(rect_y + rect_h) {
        for x in rect_x..(rect_x + rect_w) {
            if x < width && y < height {
                let fx = (x - rect_x) as f32 / rect_w as f32;
                let fy = (y - rect_y) as f32 / rect_h as f32;
                let r = (100.0 + 155.0 * fx) as u8;
                let g = (80.0 + 100.0 * (1.0 - fy)) as u8;
                let b = (180.0 + 75.0 * fy) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }

    // Draw colored circles at key positions to track the flip
    let circle_positions = [
        (width / 4, height / 4, [255, 80, 80, 255]),
        (3 * width / 4, height / 4, [80, 255, 80, 255]),
        (width / 2, height / 2, [80, 80, 255, 255]),
        (width / 4, 3 * height / 4, [255, 255, 80, 255]),
        (3 * width / 4, 3 * height / 4, [255, 80, 255, 255]),
    ];

    for (cx, cy, color) in circle_positions {
        let radius = 20u32;
        for y in (cy.saturating_sub(radius))..(cy + radius) {
            for x in (cx.saturating_sub(radius))..(cx + radius) {
                if x < width && y < height {
                    let dx = x as i32 - cx as i32;
                    let dy = y as i32 - cy as i32;
                    if dx * dx + dy * dy < (radius * radius) as i32 {
                        img.put_pixel(x, y, Rgba(color));
                    }
                }
            }
        }
    }

    // Draw a border frame
    let border = 8u32;
    for y in border..(height - border) {
        for x in border..(width - border) {
            if x < border + 3 || x >= width - border - 3 || y < border + 3 || y >= height - border - 3 {
                img.put_pixel(x, y, Rgba([150, 150, 200, 255]));
            }
        }
    }

    // Draw diagonal line to visualize the fold
    for i in 0..(width.min(height)) {
        let x = width - 1 - i;
        let y = height - 1 - i;
        if x < width && y < height {
            img.put_pixel(x, y, Rgba([200, 200, 100, 255]));
        }
    }

    img
}

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

fn apply_filter_to_image(
    filter: &PageFlipFilter,
    width: u32,
    height: u32,
    fps: f32,
    frame_time: Duration,
    segment_duration: Duration,
) -> Result<RgbaImage> {
    let buffer = create_test_image_with_content(width, height);

    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image { buffer }],
        from_segment: create_dummy_segment(segment_duration),
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
    image
        .save(path)
        .map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

/// Render an animation sequence and save frames.
fn render_animation(
    filter: &PageFlipFilter,
    width: u32,
    height: u32,
    fps: f32,
    total_duration: Duration,
    output_dir: &str,
    label: &str,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let frame_count = (total_duration.as_secs_f32() * fps).ceil() as u32;
    let frame_interval = Duration::from_secs_f32(1.0 / fps);

    println!(
        "  Rendering '{}' ({} frames, {:.2}s)...",
        label,
        frame_count,
        total_duration.as_secs_f32()
    );

    for i in 0..frame_count {
        let frame_time = frame_interval * i;
        let img = apply_filter_to_image(filter, width, height, fps, frame_time, total_duration)?;
        let frame_path = format!("{}/frame_{:03}.png", output_dir, i);
        save_image(&img, &frame_path)?;
    }

    println!("    Saved {} frames to: {}/", frame_count, output_dir);
    Ok(())
}

fn main() -> Result<()> {
    println!("Page Flip Filter Demo");
    println!("=====================\n");
    println!("Page flip effect using the turn-rs perpendicular bisector fold model.\n");

    let tmp_dir = "tmp/page_flip_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original test image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png\n");

    // ============================================================
    // Example 1: Default — forward flip from bottom-right corner
    // ============================================================
    println!("Example 1: Forward flip, bottom-right corner (default)");
    let filter1 = PageFlipFilter::default();
    render_animation(
        &filter1,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_forward_br", tmp_dir),
        "flip_forward_br",
    )?;

    // ============================================================
    // Example 2: Top-left corner flip
    // ============================================================
    println!("\nExample 2: Forward flip, top-left corner");
    let filter2 = PageFlipFilter::default().with_corner(PageFlipCorner::TopLeft);
    render_animation(
        &filter2,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_forward_tl", tmp_dir),
        "flip_forward_tl",
    )?;

    // ============================================================
    // Example 3: Bottom-left corner flip
    // ============================================================
    println!("\nExample 3: Forward flip, bottom-left corner");
    let filter3 = PageFlipFilter::default().with_corner(PageFlipCorner::BottomLeft);
    render_animation(
        &filter3,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_forward_bl", tmp_dir),
        "flip_forward_bl",
    )?;

    // ============================================================
    // Example 4: Backward flip (page flips back to front)
    // ============================================================
    println!("\nExample 4: Backward flip, bottom-right corner");
    let filter4 = PageFlipFilter::default().with_direction(PageFlipDirection::Backward);
    render_animation(
        &filter4,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_backward", tmp_dir),
        "flip_backward",
    )?;

    // ============================================================
    // Example 5: Round-trip flip (forward then backward)
    // ============================================================
    println!("\nExample 5: Round-trip flip");
    let filter5 = PageFlipFilter::default().with_direction(PageFlipDirection::RoundTrip);
    render_animation(
        &filter5,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_roundtrip", tmp_dir),
        "flip_roundtrip",
    )?;

    // ============================================================
    // Example 6: Vertical axis (calendar-style flip)
    // ============================================================
    println!("\nExample 6: Vertical axis flip (calendar-style)");
    let filter6 = PageFlipFilter::default().with_axis(PageFlipAxis::Vertical);
    render_animation(
        &filter6,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_vertical", tmp_dir),
        "flip_vertical",
    )?;

    // ============================================================
    // Example 7: No shadow
    // ============================================================
    println!("\nExample 7: No shadow");
    let filter7 = PageFlipFilter::default().with_shadow(false);
    render_animation(
        &filter7,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_no_shadow", tmp_dir),
        "flip_no_shadow",
    )?;

    // ============================================================
    // Example 8: Slow flip (2s duration)
    // ============================================================
    println!("\nExample 8: Slow flip (2s duration)");
    let filter8 = PageFlipFilter::new(Duration::from_secs(2));
    render_animation(
        &filter8,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/flip_slow", tmp_dir),
        "flip_slow",
    )?;

    // ============================================================
    // Example 9: Fast flip (0.4s duration)
    // ============================================================
    println!("\nExample 9: Fast flip (0.4s duration)");
    let filter9 = PageFlipFilter::new(Duration::from_millis(400));
    render_animation(
        &filter9,
        width,
        height,
        fps,
        Duration::from_secs(1),
        &format!("{}/flip_fast", tmp_dir),
        "flip_fast",
    )?;

    // ============================================================
    // Example 10: Position=End (animation at segment end)
    // ============================================================
    println!("\nExample 10: Position=End (flip occurs at segment end)");
    let filter10 = PageFlipFilter::default().with_position(PageFlipPosition::End);
    render_animation(
        &filter10,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/flip_at_end", tmp_dir),
        "flip_at_end",
    )?;

    // ============================================================
    // Example 11: Snapshot comparison — key frames at different flip stages
    // ============================================================
    println!("\nExample 11: Snapshot comparison of flip stages");
    let snapshot_dir = format!("{}/snapshots", tmp_dir);
    fs::create_dir_all(&snapshot_dir)?;

    let filter11 = PageFlipFilter::default();
    let stages = [
        (0.0, "stage_0_start"),
        (0.1, "stage_1_just_started"),
        (0.2, "stage_2_peeling"),
        (0.3, "stage_3_quarter"),
        (0.5, "stage_4_halfway"),
        (0.7, "stage_5_mostly_revealed"),
        (0.85, "stage_6_almost_done"),
        (1.0, "stage_7_complete"),
    ];

    for (ratio, name) in stages {
        let frame_time = Duration::from_secs_f32(ratio * filter11.duration.as_secs_f32());
        let img = apply_filter_to_image(&filter11, width, height, fps, frame_time, Duration::from_secs(10))?;
        save_image(&img, &format!("{}/{}.png", snapshot_dir, name))?;
    }
    println!("  Saved 8 stage snapshots to: {}/", snapshot_dir);

    // Summary
    println!("\n=======================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nAnimation sequences (rendered at {} fps):", fps);
    println!("  - flip_forward_br/    — Forward flip, bottom-right corner (default)");
    println!("  - flip_forward_tl/    — Forward flip, top-left corner");
    println!("  - flip_forward_bl/    — Forward flip, bottom-left corner");
    println!("  - flip_backward/      — Backward flip");
    println!("  - flip_roundtrip/     — Round-trip flip (forward then backward)");
    println!("  - flip_vertical/      — Vertical axis flip (calendar-style)");
    println!("  - flip_no_shadow/     — No shadow");
    println!("  - flip_slow/          — Slow flip (2s duration)");
    println!("  - flip_fast/          — Fast flip (0.4s duration)");
    println!("  - flip_at_end/        — Flip occurs at segment end");
    println!("\nStatic comparisons:");
    println!("  - snapshots/          — 8 key frames through the flip animation");
    println!("\nParameter guide:");
    println!("  - duration: Animation length (0.3s-3.0s recommended)");
    println!("  - position (Start/End): When the animation occurs in the segment");
    println!("  - corner (BottomRight/BottomLeft/TopRight/TopLeft): Which corner flips");
    println!("  - direction (Forward/Backward/RoundTrip): Flip direction");
    println!("  - axis (Horizontal/Vertical): Horizontal=book-style, Vertical=calendar-style");
    println!("  - shadow (bool): Whether to render shadow/highlight gradients");

    Ok(())
}
