// Example demonstrating the Wind Scatter filter
// End position:  the whole image blows apart into pixel particles ("scatter")
// Start position: particles fly back from the upwind side and reassemble
// Generates test images and saves frame sequences to tmp/wind_scatter_demo

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::traits::{EffectPosition, VideoData, VideoFilter, VideoFilterConfig},
    filters::video::WindScatterFilter,
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a colorful test image with geometric shapes to visualize the particles.
fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |_, _| Rgba([30, 30, 45, 255]));

    let grid_size = 40u32;
    for y in 0..height {
        for x in 0..width {
            if x % grid_size == 0 || y % grid_size == 0 {
                img.put_pixel(x, y, Rgba([60, 60, 80, 255]));
            }
        }
    }

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

    let border = 8u32;
    for y in border..(height - border) {
        for x in border..(width - border) {
            if x < border + 3 || x >= width - border - 3 || y < border + 3
                || y >= height - border - 3
            {
                img.put_pixel(x, y, Rgba([150, 150, 200, 255]));
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
    filter: &WindScatterFilter,
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

/// Render a time range of the filter as a frame sequence.
/// `start_time`/`end_time` are segment-relative times.
fn render_animation_range(
    filter: &WindScatterFilter,
    width: u32,
    height: u32,
    fps: f32,
    start_time: Duration,
    end_time: Duration,
    output_dir: &str,
    label: &str,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let total = end_time.saturating_sub(start_time);
    let frame_count = (total.as_secs_f32() * fps).ceil() as u32;
    let frame_interval = Duration::from_secs_f32(1.0 / fps);

    println!(
        "  Rendering '{}' ({} frames, {:.2}s, segment time {:.2}s → {:.2}s)...",
        label,
        frame_count,
        total.as_secs_f32(),
        start_time.as_secs_f32(),
        end_time.as_secs_f32(),
    );

    for i in 0..frame_count {
        let frame_time = start_time + frame_interval * i;
        let img = apply_filter_to_image(filter, width, height, fps, frame_time)?;
        let frame_path = format!("{}/frame_{:03}.png", output_dir, i);
        save_image(&img, &frame_path)?;
    }

    println!("    Saved {} frames to: {}/", frame_count, output_dir);
    Ok(())
}

fn main() -> Result<()> {
    println!("Wind Scatter Filter Demo");
    println!("========================");
    println!("End:     the image blows apart into rotating, fading particles");
    println!("Start:   the particles reassemble back into the full image");
    println!("         (from the upwind side, so both read left→right)\n");

    let tmp_dir = "tmp/wind_scatter_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;
    // Segment is 10s; the animation is 1s at the start (reassemble) or end (scatter).
    let anim = Duration::from_secs_f32(1.0);
    let seg = Duration::from_secs(10);
    // Render a little padding around the animation zone.
    let pad = Duration::from_millis(200);

    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png\n");

    // Example 1: End — scatter, default wind (left → right)
    println!("Example 1: Scatter at end (0°, tile 4, rotation 45°)");
    let filter1 = WindScatterFilter::new(EffectPosition::End, anim);
    render_animation_range(
        &filter1,
        width,
        height,
        fps,
        seg - anim - pad,
        seg,
        &format!("{}/scatter_0deg", tmp_dir),
        "scatter_0deg",
    )?;

    // Example 2: Start — reassemble, particles return from the left (upwind side)
    println!("\nExample 2: Reassemble at start (particles return from the left)");
    let filter2 = WindScatterFilter::new(EffectPosition::Start, anim);
    render_animation_range(
        &filter2,
        width,
        height,
        fps,
        Duration::ZERO,
        anim + pad,
        &format!("{}/reassemble_0deg", tmp_dir),
        "reassemble_0deg",
    )?;

    // Example 3: End — diagonal wind (45°)
    println!("\nExample 3: Scatter with 45° diagonal wind");
    let filter3 = WindScatterFilter::new(EffectPosition::End, anim).with_angle_deg(45.0);
    render_animation_range(
        &filter3,
        width,
        height,
        fps,
        seg - anim - pad,
        seg,
        &format!("{}/scatter_45deg", tmp_dir),
        "scatter_45deg",
    )?;

    // Example 4: End — coarse flakes (tile 8)
    println!("\nExample 4: Scatter with coarse flakes (tile_size=8)");
    let filter4 = WindScatterFilter::new(EffectPosition::End, anim).with_tile_size(8);
    render_animation_range(
        &filter4,
        width,
        height,
        fps,
        seg - anim - pad,
        seg,
        &format!("{}/scatter_tile8", tmp_dir),
        "scatter_tile8",
    )?;

    // Example 5: End — wild spin (rotation 90°)
    println!("\nExample 5: Scatter with heavy rotation (max_rotation_deg=90)");
    let filter5 = WindScatterFilter::new(EffectPosition::End, anim)
        .with_max_rotation_deg(90.0)
        .with_speed(1.5);
    render_animation_range(
        &filter5,
        width,
        height,
        fps,
        seg - anim - pad,
        seg,
        &format!("{}/scatter_heavy_rotation", tmp_dir),
        "scatter_heavy_rotation",
    )?;

    println!("\n=======================");
    println!("All examples generated!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nParameter guide:");
    println!("  - position: Start=reassemble (particles → image), End=scatter (image → particles)");
    println!("  - duration: Animation length (1.0s default)");
    println!("  - angle_deg: Wind direction, 0=left→right, 90=top→bottom, 180=right→left");
    println!("  - tile_size: Particle cluster edge in pixels (1=pixel dust, 4=default, 8+=flakes)");
    println!("  - max_rotation_deg: Max particle spin while flying (45 default)");
    println!("  - speed: Scatter distance as a fraction of the shorter frame side (1.0 default)");
    println!("  - seed: Random seed, fixed by default for deterministic renders");

    Ok(())
}
