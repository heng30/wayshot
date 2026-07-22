// Example demonstrating the wave filter
// Generates test images and saves them to tmp/wave_demo directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{WaveFilter, WaveType},
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Checkerboard pattern for wave visualization
        let checker_size = 40;
        let is_dark = ((x / checker_size) + (y / checker_size)) % 2 == 0;
        if is_dark {
            Rgba([80, 80, 100, 255])
        } else {
            Rgba([200, 200, 220, 255])
        }
    });

    // Add a grid of circles to show wave distortion clearly
    for row in 0..5 {
        for col in 0..6 {
            let cx = col * (width / 5) + width / 10;
            let cy = row * (height / 4) + height / 8;
            let radius = 25;
            for y in (cy - radius)..(cy + radius) {
                for x in (cx - radius)..(cx + radius) {
                    if x < width && y < height {
                        let dx = x as i32 - cx as i32;
                        let dy = y as i32 - cy as i32;
                        if dx * dx + dy * dy < (radius * radius) as i32 {
                            // Different colors for different positions
                            let color_idx = ((row + col) % 3) as usize;
                            let colors: [[u8; 3]; 3] = [[255, 100, 100], [100, 255, 100], [100, 100, 255]];
                            img.put_pixel(x, y, Rgba([colors[color_idx][0], colors[color_idx][1], colors[color_idx][2], 255]));
                        }
                    }
                }
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
    filter: &WaveFilter,
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
        Err(video_editor::Error::InvalidConfig("No image generated".into()))
    }
}

fn save_image(image: &RgbaImage, path: &str) -> Result<()> {
    image.save(path).map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

fn main() -> Result<()> {
    println!("Wave Filter Demo");
    println!("================\n");

    // Create tmp directory
    let tmp_dir = "tmp/wave_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Horizontal waves (waves along X axis)
    println!("\nExample 1: Horizontal waves (amplitude=20, frequency=3)");
    let filter1 = WaveFilter::new(20.0, 3.0, WaveType::Horizontal);
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/wave_horizontal.png", tmp_dir))?;
    println!("  Saved: wave_horizontal.png");

    // Example 2: Vertical waves (waves along Y axis)
    println!("\nExample 2: Vertical waves (amplitude=20, frequency=3)");
    let filter2 = WaveFilter::new(20.0, 3.0, WaveType::Vertical);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/wave_vertical.png", tmp_dir))?;
    println!("  Saved: wave_vertical.png");

    // Example 3: Radial waves (radiating from center)
    println!("\nExample 3: Radial waves (amplitude=20, frequency=4)");
    let filter3 = WaveFilter::new(20.0, 4.0, WaveType::Radial);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/wave_radial.png", tmp_dir))?;
    println!("  Saved: wave_radial.png");

    // Example 4: Concentric waves (pond ripple effect)
    println!("\nExample 4: Concentric waves (ripple effect, amplitude=25, frequency=5)");
    let filter4 = WaveFilter::new(25.0, 5.0, WaveType::Concentric);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/wave_concentric.png", tmp_dir))?;
    println!("  Saved: wave_concentric.png");

    // Example 5: Amplitude comparison
    println!("\nExample 5: Amplitude comparison (horizontal waves)");
    for amplitude in [5.0, 10.0, 20.0, 40.0, 60.0, 80.0] {
        let filter = WaveFilter::new(amplitude, 3.0, WaveType::Horizontal);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/wave_amplitude_{:.0}.png", tmp_dir, amplitude))?;
    }
    println!("  Saved: wave_amplitude_5.png through wave_amplitude_80.png");

    // Example 6: Frequency comparison
    println!("\nExample 6: Frequency comparison (horizontal waves)");
    for frequency in [0.5, 1.0, 2.0, 4.0, 6.0, 8.0] {
        let filter = WaveFilter::new(20.0, frequency, WaveType::Horizontal);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/wave_frequency_{:.1}.png", tmp_dir, frequency))?;
    }
    println!("  Saved: wave_frequency_0.5.png through wave_frequency_8.0.png");

    // Example 7: Concentric waves with different centers
    println!("\nExample 7: Concentric waves with offset center");
    let center_positions = [
        ("center", 0.5, 0.5),
        ("top_left", 0.25, 0.25),
        ("top_right", 0.75, 0.25),
        ("bottom_left", 0.25, 0.75),
        ("bottom_right", 0.75, 0.75),
    ];
    for (name, cx, cy) in center_positions {
        let filter = WaveFilter::new(25.0, 4.0, WaveType::Concentric)
            .with_center_x(cx)
            .with_center_y(cy);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/wave_center_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: wave_center_center.png, top_left.png, top_right.png, etc.");

    // Example 8: Radial waves with different centers
    println!("\nExample 8: Radial waves with offset center");
    for (name, cx, cy) in [("center", 0.5, 0.5), ("left", 0.2, 0.5), ("top", 0.5, 0.2)] {
        let filter = WaveFilter::new(15.0, 3.0, WaveType::Radial)
            .with_center_x(cx)
            .with_center_y(cy);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/wave_radial_center_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: wave_radial_center_center.png, left.png, top.png");

    // Example 9: All wave types at moderate settings
    println!("\nExample 9: All wave types comparison");
    for wave_type in WaveType::all_types() {
        let filter = WaveFilter::new(20.0, 3.0, *wave_type);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/wave_type_{}.png", tmp_dir, wave_type.name()))?;
    }
    println!("  Saved: wave_type_horizontal.png, vertical.png, radial.png, concentric.png");

    // Example 10: Subtle wave (low amplitude, low frequency)
    println!("\nExample 10: Subtle wave effect (amplitude=5, frequency=1)");
    let filter10 = WaveFilter::new(5.0, 1.0, WaveType::Horizontal);
    let img10 = apply_filter_to_image(&filter10, width, height, fps)?;
    save_image(&img10, &format!("{}/wave_subtle.png", tmp_dir))?;
    println!("  Saved: wave_subtle.png");

    // Example 11: Intense wave (high amplitude, high frequency)
    println!("\nExample 11: Intense wave effect (amplitude=50, frequency=6)");
    let filter11 = WaveFilter::new(50.0, 6.0, WaveType::Horizontal);
    let img11 = apply_filter_to_image(&filter11, width, height, fps)?;
    save_image(&img11, &format!("{}/wave_intense.png", tmp_dir))?;
    println!("  Saved: wave_intense.png");

    println!("\n================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - amplitude (0-100): Wave displacement in pixels (higher = more distortion)");
    println!("  - frequency (0.1-10): Wave cycles per unit (higher = denser waves)");
    println!("  - speed (0-10): Animation speed (0 = static, higher = faster animation)");
    println!("  - phase (0-360): Initial phase offset in degrees");
    println!("  - wave_type: horizontal, vertical, radial, concentric");
    println!("  - center_x/center_y (0-1): Center position for radial/concentric waves");

    Ok(())
}