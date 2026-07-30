use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    filters::video::{LightingDirection, LightingFilter, LightingScene},
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::{segment::Segment, video_frame_cache::VideoImage},
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        let fx = x as f32 / width as f32;
        let fy = y as f32 / height as f32;

        let r = (40.0 + 60.0 * fx) as u8;
        let g = (40.0 + 60.0 * fy) as u8;
        let b = (60.0 + 40.0 * (fx + fy) / 2.0) as u8;

        Rgba([r, g, b, 255])
    });

    let grid_size = 40u32;
    for y in 0..height {
        for x in 0..width {
            if x % grid_size == 0 || y % grid_size == 0 {
                img.put_pixel(x, y, Rgba([70, 70, 90, 255]));
            }
        }
    }

    let rect_w = (width as f32 * 0.5) as u32;
    let rect_h = (height as f32 * 0.5) as u32;
    let rect_x = (width - rect_w) / 2;
    let rect_y = (height - rect_h) / 2;

    for y in rect_y..(rect_y + rect_h) {
        for x in rect_x..(rect_x + rect_w) {
            if x < width && y < height {
                let fx = (x - rect_x) as f32 / rect_w as f32;
                let fy = (y - rect_y) as f32 / rect_h as f32;
                let r = (120.0 + 135.0 * fx) as u8;
                let g = (100.0 + 100.0 * (1.0 - fy)) as u8;
                let b = (180.0 + 75.0 * fy) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
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

fn create_dummy_segment(duration: Duration) -> Arc<Segment> {
    let metadata = Arc::new(Metadata {
        path: PathBuf::from("dummy.mp4"),
        size: 0,
        bitrate: 0,
        duration,
        format: vec![],
        videos: vec![],
        audios: vec![],
        subtitles: vec![],
    });
    Arc::new(Segment::new(
        Duration::ZERO,
        duration,
        metadata,
        1.0,
    ))
}

fn apply_filter_to_image(
    filter: &LightingFilter,
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

fn render_animation(
    filter: &LightingFilter,
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
    println!("Lighting Filter Demo");
    println!("====================\n");
    println!("Spotlight effect with pendulum swing physics.\n");

    let tmp_dir = "tmp/lighting_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png\n");

    // Example 1: Default — warm spotlight
    println!("Example 1: Default warm spotlight");
    let filter1 = LightingFilter::default();
    render_animation(
        &filter1,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/default_warm", tmp_dir),
        "default_warm",
    )?;

    // Example 2: Brighter, wider cone
    println!("\nExample 2: Brighter, wider cone");
    let filter2 = LightingFilter::default()
        .with_brightness(2500.0)
        .with_angle_deg(50.0)
        .with_penumbra(0.9);
    render_animation(
        &filter2,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/bright_wide", tmp_dir),
        "bright_wide",
    )?;

    // Example 3: Cool white light
    println!("\nExample 3: Cool white light");
    let filter3 = LightingFilter::default().with_color([0.7, 0.8, 1.0]);
    render_animation(
        &filter3,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/cool_white", tmp_dir),
        "cool_white",
    )?;

    // Example 4: Horizontal scene (floor lighting)
    println!("\nExample 4: Horizontal scene (floor lighting)");
    let filter4 = LightingFilter::default().with_scene(LightingScene::Horizontal);
    render_animation(
        &filter4,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/horizontal_floor", tmp_dir),
        "horizontal_floor",
    )?;

    // Example 5: Light from the left
    println!("\nExample 5: Light from the left");
    let filter5 = LightingFilter::default()
        .with_direction(LightingDirection::Left)
        .with_pos((0.0, 0.0));
    render_animation(
        &filter5,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/from_left", tmp_dir),
        "from_left",
    )?;

    // Example 6: Static spotlight (no swing)
    println!("\nExample 6: Static spotlight (no swing)");
    let filter6 = LightingFilter::default().with_swing(0.0);
    render_animation(
        &filter6,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/static_spot", tmp_dir),
        "static_spot",
    )?;

    // Example 7: High ambient light (less dramatic)
    println!("\nExample 7: High ambient light");
    let filter7 = LightingFilter::default().with_ambient(0.3);
    render_animation(
        &filter7,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/high_ambient", tmp_dir),
        "high_ambient",
    )?;

    // Example 8: Off-center spotlight position
    println!("\nExample 8: Off-center spotlight position");
    let filter8 = LightingFilter::default().with_pos((0.8, 0.3));
    render_animation(
        &filter8,
        width,
        height,
        fps,
        Duration::from_secs(3),
        &format!("{}/off_center", tmp_dir),
        "off_center",
    )?;

    // Summary
    println!("\n=======================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);

    Ok(())
}
