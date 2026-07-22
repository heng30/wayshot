// Example demonstrating the Genie Effect filter
// Simulates macOS "sucked into Dock" / "pop out from Dock" animation
// Generates test images and saves them to tmp/genie_demo directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{GenieAnchor, GenieFilter},
    filters::traits::{EffectPosition, VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a colorful test image with geometric shapes to visualize the funnel distortion.
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
            if x < border + 3 || x >= width - border - 3 || y < border + 3 || y >= height - border - 3 {
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
    filter: &GenieFilter,
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

fn render_animation(
    filter: &GenieFilter,
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

    println!("  Rendering '{}' ({} frames, {:.2}s)...", label, frame_count, total_duration.as_secs_f32());

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
    println!("Genie Effect Filter Demo");
    println!("========================\n");
    println!("Simulates macOS \"sucked into Dock\" / \"pop out from Dock\" Genie animation.\n");

    let tmp_dir = "tmp/genie_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png\n");

    // Example 1: Sucked INTO Dock — classic Genie minimize
    println!("Example 1: Sucked INTO Dock (End position)");
    let filter1 = GenieFilter::new(
        EffectPosition::End,
        Duration::from_secs_f32(0.8),
        GenieAnchor::BottomCenter,
        2.0,  // parabolic funnel
        0.0,  // no bounce
    );
    render_animation(&filter1, width, height, fps, Duration::from_secs(2), &format!("{}/suck_in_center", tmp_dir), "suck_in_center")?;

    // Example 2: Pop OUT from Dock — with bounce
    println!("\nExample 2: Pop OUT from Dock (Start position, with bounce)");
    let filter2 = GenieFilter::new(
        EffectPosition::Start,
        Duration::from_secs_f32(0.8),
        GenieAnchor::BottomCenter,
        2.0,
        1.0,
    );
    render_animation(&filter2, width, height, fps, Duration::from_secs(2), &format!("{}/pop_out_center", tmp_dir), "pop_out_center")?;

    // Example 3: Suck into bottom-left
    println!("\nExample 3: Sucked into bottom-left Dock position");
    let filter3 = GenieFilter::new(
        EffectPosition::End,
        Duration::from_secs_f32(0.8),
        GenieAnchor::BottomLeft,
        2.0,
        0.0,
    );
    render_animation(&filter3, width, height, fps, Duration::from_secs(2), &format!("{}/suck_in_left", tmp_dir), "suck_in_left")?;

    // Example 4: Cubic funnel (more dramatic pinch)
    println!("\nExample 4: Cubic funnel (funnel_power=3.0)");
    let filter4 = GenieFilter::new(
        EffectPosition::End,
        Duration::from_secs_f32(0.8),
        GenieAnchor::BottomCenter,
        3.0,
        0.0,
    );
    render_animation(&filter4, width, height, fps, Duration::from_secs(2), &format!("{}/cubic_funnel", tmp_dir), "cubic_funnel")?;

    // Example 5: Linear trapezoid (power=1.0)
    println!("\nExample 5: Linear trapezoid (funnel_power=1.0)");
    let filter5 = GenieFilter::new(
        EffectPosition::End,
        Duration::from_secs_f32(0.8),
        GenieAnchor::BottomCenter,
        1.0,
        0.0,
    );
    render_animation(&filter5, width, height, fps, Duration::from_secs(2), &format!("{}/linear_trapezoid", tmp_dir), "linear_trapezoid")?;

    // Example 6: Double bounce pop-out
    println!("\nExample 6: Pop OUT with double bounce");
    let filter6 = GenieFilter::new(
        EffectPosition::Start,
        Duration::from_secs_f32(1.0),
        GenieAnchor::BottomCenter,
        2.0,
        2.0,
    );
    render_animation(&filter6, width, height, fps, Duration::from_secs(2), &format!("{}/pop_out_double_bounce", tmp_dir), "pop_out_double_bounce")?;

    println!("\n=======================");
    println!("All examples generated!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nParameter guide:");
    println!("  - position: Start=pop out (small→big), End=suck in (big→small)");
    println!("  - duration: Animation length (0.3-3.0s recommended)");
    println!("  - anchor: Dock position — BottomLeft, BottomCenter, BottomRight");
    println!("  - funnel_power: 1=trapezoid, 2=parabolic/classic, 3+=dramatic pinch");
    println!("  - bounce_count: Overshoot bounces on pop-out (0=smooth, 1=one bounce)");
    println!("  - shadow: Edge shadow for depth (0-1)");

    Ok(())
}
