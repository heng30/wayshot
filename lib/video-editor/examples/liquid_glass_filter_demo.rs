// Example demonstrating the liquid glass filter.
// Generates test images and keyframe animation frames into tmp/liquid_glass_demo/.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    filters::{
        keyframe::{Keyframe, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter, VideoFilterConfig},
        video::LiquidGlassFilter,
    },
    metadata::Metadata,
    tracks::{segment::Segment, video_frame_cache::VideoImage},
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        let fx = x as f32 / width as f32;
        let fy = y as f32 / height as f32;
        let r = (30.0 + 80.0 * fx) as u8;
        let g = (40.0 + 90.0 * fy) as u8;
        let b = (70.0 + 50.0 * ((fx + fy) / 2.0)) as u8;
        Rgba([r, g, b, 255])
    });

    let grid = 32u32;
    for y in 0..height {
        for x in 0..width {
            if x % grid == 0 || y % grid == 0 {
                img.put_pixel(x, y, Rgba([220, 220, 240, 255]));
            }
        }
    }

    let circles: [(u32, u32, u32, [u8; 3]); 4] = [
        (width / 4, height / 4, 70, [255, 80, 80]),
        (3 * width / 4, height / 4, 55, [80, 255, 90]),
        (width / 4, 3 * height / 4, 55, [80, 140, 255]),
        (3 * width / 4, 3 * height / 4, 70, [255, 220, 60]),
    ];
    for (cx, cy, radius, color) in circles {
        for y in cy.saturating_sub(radius)..(cy + radius).min(height) {
            for x in cx.saturating_sub(radius)..(cx + radius).min(width) {
                let dx = x as i32 - cx as i32;
                let dy = y as i32 - cy as i32;
                if dx * dx + dy * dy < (radius * radius) as i32 {
                    img.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
                }
            }
        }
    }

    let rect_w = width / 2;
    let rect_h = height / 2;
    let rect_x = (width - rect_w) / 2;
    let rect_y = (height - rect_h) / 2;
    for y in rect_y..(rect_y + rect_h) {
        for x in rect_x..(rect_x + rect_w) {
            if (x / 16 + y / 16) % 2 == 0 {
                img.put_pixel(x, y, Rgba([245, 245, 245, 255]));
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
    filter: &LiquidGlassFilter,
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
    filter: &LiquidGlassFilter,
    width: u32,
    height: u32,
    fps: f32,
    total_duration: Duration,
    output_dir: &str,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    let frame_count = (total_duration.as_secs_f32() * fps).ceil() as u32;
    let frame_interval = Duration::from_secs_f32(1.0 / fps);

    for i in 0..frame_count {
        let frame_time = frame_interval * i;
        let img = apply_filter_to_image(filter, width, height, fps, frame_time, total_duration)?;
        save_image(&img, &format!("{}/frame_{:03}.png", output_dir, i))?;
    }

    println!("    Saved {} frames to: {}/", frame_count, output_dir);
    Ok(())
}

fn main() -> Result<()> {
    println!("Liquid Glass Filter Demo");
    println!("========================\n");

    let tmp_dir = "tmp/liquid_glass_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;

    println!("\nExample 1: Default glass card");
    let filter1 = LiquidGlassFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps, Duration::ZERO, Duration::from_secs(3))?;
    save_image(&img1, &format!("{}/default.png", tmp_dir))?;

    println!("\nExample 2: Frosted glass (blur)");
    let filter2 = LiquidGlassFilter::default().with_blur_radius(40.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps, Duration::ZERO, Duration::from_secs(3))?;
    save_image(&img2, &format!("{}/frosted.png", tmp_dir))?;

    println!("\nExample 3: Strong dispersion");
    let filter3 = LiquidGlassFilter::default().with_chromatic_aberration(1.5);
    let img3 = apply_filter_to_image(&filter3, width, height, fps, Duration::ZERO, Duration::from_secs(3))?;
    save_image(&img3, &format!("{}/dispersion.png", tmp_dir))?;

    println!("\nExample 4: Tinted glass");
    let filter4 = LiquidGlassFilter::default()
        .with_tint_color([0.2, 0.4, 0.9])
        .with_tint_alpha(0.35)
        .with_blur_radius(30.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps, Duration::ZERO, Duration::from_secs(3))?;
    save_image(&img4, &format!("{}/tinted.png", tmp_dir))?;

    println!("\nExample 5: Full-frame card");
    let filter5 = LiquidGlassFilter::default()
        .with_x(0.0)
        .with_y(0.0)
        .with_width(1.0)
        .with_height(1.0)
        .with_corner_radius(80.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps, Duration::ZERO, Duration::from_secs(3))?;
    save_image(&img5, &format!("{}/full_frame.png", tmp_dir))?;

    println!("\nExample 6: Keyframe animation (moving card + pulsing refraction + blur fade)");
    let mut filter6 = LiquidGlassFilter::default();
    let mut tracks = KeyframeTracks::new();
    tracks.add_keyframe("position", Keyframe::new(0, KeyframeValue::Float2(0.1, 0.3)));
    tracks.add_keyframe("position", Keyframe::new(1000, KeyframeValue::Float2(0.5, 0.3)));
    tracks.add_keyframe("position", Keyframe::new(2000, KeyframeValue::Float2(0.1, 0.3)));
    tracks.add_keyframe("refraction_amount", Keyframe::float(0, 40.0));
    tracks.add_keyframe("refraction_amount", Keyframe::float(1000, 120.0));
    tracks.add_keyframe("refraction_amount", Keyframe::float(2000, 40.0));
    tracks.add_keyframe("blur_radius", Keyframe::float(0, 0.0));
    tracks.add_keyframe("blur_radius", Keyframe::float(1000, 50.0));
    tracks.add_keyframe("blur_radius", Keyframe::float(2000, 0.0));
    filter6.set_keyframe_tracks(tracks);
    render_animation(
        &filter6,
        width,
        height,
        fps,
        Duration::from_secs(2),
        &format!("{}/keyframes", tmp_dir),
    )?;

    println!("\n=======================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}/", tmp_dir);

    Ok(())
}
