// Example demonstrating the Split transition filter
// The image splits into two halves that slide apart (End position)
// or come together (Start position) — a classic transition effect.
// Generates test images and saves them to tmp/split_demo directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{SplitDirection, SplitFilter},
    filters::traits::{EasingFunction, EffectPosition, VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a colorful test image with geometric shapes to visualize the split effect.
fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |_, _| Rgba([30, 30, 45, 255]));

    // Grid pattern
    let grid_size = 40u32;
    for y in 0..height {
        for x in 0..width {
            if x % grid_size == 0 || y % grid_size == 0 {
                img.put_pixel(x, y, Rgba([60, 60, 80, 255]));
            }
        }
    }

    // Large colored rectangle in the center
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

    // Colored circles at key positions
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

    // Border frame
    let border = 8u32;
    for y in border..(height - border) {
        for x in border..(width - border) {
            if x < border + 3 || x >= width - border - 3 || y < border + 3 || y >= height - border - 3 {
                img.put_pixel(x, y, Rgba([150, 150, 200, 255]));
            }
        }
    }

    // Center crosshair to show the split line
    let cx = width / 2;
    let cy = height / 2;
    for i in 0..width {
        img.put_pixel(i, cy, Rgba([200, 200, 100, 255]));
    }
    for i in 0..height {
        img.put_pixel(cx, i, Rgba([200, 200, 100, 255]));
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
    filter: &SplitFilter,
    width: u32,
    height: u32,
    fps: f32,
    frame_time: Duration,
) -> Result<RgbaImage> {
    let buffer = create_test_image_with_content(width, height);

    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image { buffer }],
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
    image
        .save(path)
        .map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

/// Render an animation sequence and save frames.
fn render_animation(
    filter: &SplitFilter,
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
        let img = apply_filter_to_image(filter, width, height, fps, frame_time)?;
        let frame_path = format!("{}/frame_{:03}.png", output_dir, i);
        save_image(&img, &frame_path)?;
    }

    println!("    Saved {} frames to: {}/", frame_count, output_dir);
    Ok(())
}

fn main() -> Result<()> {
    println!("Split Transition Filter Demo");
    println!("=================================\n");
    println!("The image splits into two halves that slide apart (End position)\n\
              or come together (Start position) — a classic transition effect.\n");

    let tmp_dir = "tmp/split_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original test image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png\n");

    // ============================================================
    // Example 1: Horizontal split-out (default — center, End position)
    // ============================================================
    println!("Example 1: Horizontal split-out (default)");
    let filter1 = SplitFilter::default();
    render_animation(
        &filter1,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_default", tmp_dir),
        "split_h_default",
    )?;

    // ============================================================
    // Example 2: Vertical split-out (top/bottom halves move apart)
    // ============================================================
    println!("\nExample 2: Vertical split-out");
    let filter2 = SplitFilter::default()
        .with_direction(SplitDirection::Vertical);
    render_animation(
        &filter2,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_v_default", tmp_dir),
        "split_v_default",
    )?;

    // ============================================================
    // Example 3: Split-in (halves come together from off-screen)
    // ============================================================
    println!("\nExample 3: Split-in (Start position — halves come together)");
    let filter3 = SplitFilter::new(
        EffectPosition::Start,
        Duration::from_secs(1),
        SplitDirection::Horizontal,
    );
    render_animation(
        &filter3,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_in_h", tmp_dir),
        "split_in_h",
    )?;

    // ============================================================
    // Example 4: Vertical split-in
    // ============================================================
    println!("\nExample 4: Vertical split-in");
    let filter4 = SplitFilter::new(
        EffectPosition::Start,
        Duration::from_secs(1),
        SplitDirection::Vertical,
    );
    render_animation(
        &filter4,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_in_v", tmp_dir),
        "split_in_v",
    )?;

    // ============================================================
    // Example 5: Off-center split (split_position=0.3)
    // ============================================================
    println!("\nExample 5: Off-center horizontal split (split_position=0.3)");
    let filter5 = SplitFilter::default()
        .with_split_position(0.3);
    render_animation(
        &filter5,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_offcenter_30", tmp_dir),
        "split_h_offcenter_30",
    )?;

    // ============================================================
    // Example 6: Off-center split (split_position=0.7)
    // ============================================================
    println!("\nExample 6: Off-center horizontal split (split_position=0.7)");
    let filter6 = SplitFilter::default()
        .with_split_position(0.7);
    render_animation(
        &filter6,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_offcenter_70", tmp_dir),
        "split_h_offcenter_70",
    )?;

    // ============================================================
    // Example 7: No shadow
    // ============================================================
    println!("\nExample 7: No shadow (shadow=0.0)");
    let filter7 = SplitFilter::default()
        .with_shadow(0.0);
    render_animation(
        &filter7,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_no_shadow", tmp_dir),
        "split_h_no_shadow",
    )?;

    // ============================================================
    // Example 8: Strong shadow
    // ============================================================
    println!("\nExample 8: Strong shadow (shadow=0.9, shadow_width=40)");
    let filter8 = SplitFilter::default()
        .with_shadow(0.9)
        .with_shadow_width(40.0);
    render_animation(
        &filter8,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_strong_shadow", tmp_dir),
        "split_h_strong_shadow",
    )?;

    // ============================================================
    // Example 9: Fast split (0.4s duration)
    // ============================================================
    println!("\nExample 9: Fast split (0.4s duration)");
    let filter9 = SplitFilter::new(
        EffectPosition::End,
        Duration::from_millis(400),
        SplitDirection::Horizontal,
    );
    render_animation(
        &filter9,
        width,
        height,
        fps,
        Duration::from_secs(1),
        &format!("{}/split_h_fast", tmp_dir),
        "split_h_fast",
    )?;

    // ============================================================
    // Example 10: Slow split (2s duration)
    // ============================================================
    println!("\nExample 10: Slow split (2s duration)");
    let filter10 = SplitFilter::new(
        EffectPosition::End,
        Duration::from_secs(2),
        SplitDirection::Horizontal,
    );
    render_animation(
        &filter10,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/split_h_slow", tmp_dir),
        "split_h_slow",
    )?;

    // ============================================================
    // Example 11: EaseIn easing (slow start, fast end)
    // ============================================================
    println!("\nExample 11: EaseIn easing");
    let filter11 = SplitFilter::default()
        .with_easing(EasingFunction::EaseIn);
    render_animation(
        &filter11,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_ease_in", tmp_dir),
        "split_h_ease_in",
    )?;

    // ============================================================
    // Example 12: Linear easing
    // ============================================================
    println!("\nExample 12: Linear easing");
    let filter12 = SplitFilter::default()
        .with_easing(EasingFunction::Linear);
    render_animation(
        &filter12,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/split_h_linear", tmp_dir),
        "split_h_linear",
    )?;

    // ============================================================
    // Example 13: Snapshot comparison — key frames at different split stages
    // ============================================================
    println!("\nExample 13: Snapshot comparison of split stages");
    let snapshot_dir = format!("{}/snapshots", tmp_dir);
    fs::create_dir_all(&snapshot_dir)?;

    let filter13 = SplitFilter::default();
    let stages = [
        (0.0, "stage_0_intact"),
        (0.1, "stage_1_just_started"),
        (0.2, "stage_2_small_gap"),
        (0.3, "stage_3_quarter"),
        (0.5, "stage_4_halfway"),
        (0.7, "stage_5_mostly_apart"),
        (0.85, "stage_6_almost_gone"),
        (0.95, "stage_7_nearly_offscreen"),
    ];

    for (progress, name) in stages {
        // For End position: time_until_end = duration * (1 - progress)
        // frame_time_offset = total_duration - time_until_end
        let total_dur = Duration::from_secs(10);
        let time_until_end = filter13.duration.mul_f32(1.0 - progress);
        let frame_time = total_dur.saturating_sub(time_until_end);
        let img = apply_filter_to_image(&filter13, width, height, fps, frame_time)?;
        save_image(&img, &format!("{}/{}.png", snapshot_dir, name))?;
    }
    println!("  Saved 8 stage snapshots to: {}/", snapshot_dir);

    // ============================================================
    // Example 14: Direction comparison (single frame at 40% progress)
    // ============================================================
    println!("\nExample 14: Direction comparison (frame at 40% progress)");
    let dir_dir = format!("{}/direction_comparison", tmp_dir);
    fs::create_dir_all(&dir_dir)?;

    let directions = [
        (SplitDirection::Horizontal, "dir_horizontal"),
        (SplitDirection::Vertical, "dir_vertical"),
    ];

    for (direction, name) in directions {
        let filter = SplitFilter::default().with_direction(direction);
        let total_dur = Duration::from_secs(10);
        let time_until_end = filter.duration.mul_f32(0.6); // 40% progress
        let frame_time = total_dur.saturating_sub(time_until_end);
        let img = apply_filter_to_image(&filter, width, height, fps, frame_time)?;
        save_image(&img, &format!("{}/{}.png", dir_dir, name))?;
    }
    println!("  Saved 2 direction comparison images to: {}/", dir_dir);

    // Summary
    println!("\n=================================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nAnimation sequences (rendered at {} fps):", fps);
    println!("  - split_h_default/        — Horizontal split-out (default, center)");
    println!("  - split_v_default/        — Vertical split-out (center)");
    println!("  - split_in_h/             — Horizontal split-in (halves come together)");
    println!("  - split_in_v/             — Vertical split-in");
    println!("  - split_h_offcenter_30/   — Off-center split (30% from left)");
    println!("  - split_h_offcenter_70/   — Off-center split (70% from left)");
    println!("  - split_h_no_shadow/      — No shadow along split edge");
    println!("  - split_h_strong_shadow/  — Strong shadow (0.9, width=40)");
    println!("  - split_h_fast/           — Fast split (0.4s)");
    println!("  - split_h_slow/           — Slow split (2s)");
    println!("  - split_h_ease_in/        — EaseIn easing");
    println!("  - split_h_linear/         — Linear easing");
    println!("\nStatic comparisons:");
    println!("  - snapshots/              — 8 key frames through the split-out animation");
    println!("  - direction_comparison/   — Horizontal vs Vertical at 40%% progress");
    println!("\nParameter guide:");
    println!("  - position:  Start=split in (halves come together), End=split out (halves move apart)");
    println!("  - duration:  Animation length (0.3s-3.0s recommended)");
    println!("  - direction: Horizontal (left/right), Vertical (top/bottom)");
    println!("  - split_position (0.0-1.0): Where the split line is (0.5=center)");
    println!("  - shadow (0.0-1.0): Shadow intensity along split edge");
    println!("  - shadow_width (0.0-100.0): Shadow width in pixels");
    println!("  - easing: Linear, EaseIn, EaseOut, EaseInOut");

    Ok(())
}
