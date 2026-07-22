// Example demonstrating the fisheye/spherize filter
// Generates test images and saves them to tmp/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::FisheyeFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_grid(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a checkerboard pattern for better visibility of distortion
        let checker_size = 40;
        let is_white = ((x / checker_size) + (y / checker_size)) % 2 == 0;
        if is_white {
            Rgba([240, 240, 240, 255])
        } else {
            Rgba([180, 180, 200, 255])
        }
    });

    // Add colored circles to make distortion more visible
    let center_x = width / 2;
    let center_y = height / 2;

    // Draw a central colored square
    let rect_size = 60;
    for y in (center_y - rect_size/2)..(center_y + rect_size/2) {
        for x in (center_x - rect_size/2)..(center_x + rect_size/2) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([255, 100, 100, 255]));
            }
        }
    }

    // Draw corner markers
    let marker_size = 30;
    let corners = [(20, 20), (width - 50, 20), (20, height - 50), (width - 50, height - 50)];
    for (cx, cy) in corners {
        for y in cy..(cy + marker_size) {
            for x in cx..(cx + marker_size) {
                if x < width && y < height {
                    img.put_pixel(x, y, Rgba([100, 255, 100, 255]));
                }
            }
        }
    }

    // Draw horizontal and vertical lines through center
    for x in 0..width {
        img.put_pixel(x, center_y, Rgba([50, 50, 255, 255]));
    }
    for y in 0..height {
        img.put_pixel(center_x, y, Rgba([50, 50, 255, 255]));
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
    filter: &FisheyeFilter,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<RgbaImage> {
    let buffer = create_test_image_with_grid(width, height);

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
    println!("Fisheye/Spherize Filter Demo");
    println!("============================\n");

    // Create tmp directory
    let tmp_dir = "tmp/fisheye_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Default fisheye (moderate convex effect)
    println!("Example 1: Default fisheye (strength=0.5, radius=200)");
    let filter1 = FisheyeFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/fisheye_default.png", tmp_dir))?;
    println!("  Saved: fisheye_default.png");

    // Example 2: Strong bulge effect (convex mirror)
    println!("\nExample 2: Strong bulge effect (strength=1.0, radius=200)");
    let filter2 = FisheyeFilter::new(0.5, 0.5, 1.0, 200);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/fisheye_strong_bulge.png", tmp_dir))?;
    println!("  Saved: fisheye_strong_bulge.png");

    // Example 3: Maximum bulge effect
    println!("\nExample 3: Maximum bulge effect (strength=2.0, radius=200)");
    let filter3 = FisheyeFilter::new(0.5, 0.5, 2.0, 200);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/fisheye_max_bulge.png", tmp_dir))?;
    println!("  Saved: fisheye_max_bulge.png");

    // Example 4: Pinch/concave effect (negative strength)
    println!("\nExample 4: Pinch effect (strength=-0.5, radius=200)");
    let filter4 = FisheyeFilter::new(0.5, 0.5, -0.5, 200);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/fisheye_pinch.png", tmp_dir))?;
    println!("  Saved: fisheye_pinch.png");

    // Example 5: Strong pinch effect
    println!("\nExample 5: Strong pinch effect (strength=-1.0, radius=200)");
    let filter5 = FisheyeFilter::new(0.5, 0.5, -1.0, 200);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/fisheye_strong_pinch.png", tmp_dir))?;
    println!("  Saved: fisheye_strong_pinch.png");

    // Example 6: Large radius fisheye
    println!("\nExample 6: Large radius (strength=0.5, radius=300)");
    let filter6 = FisheyeFilter::new(0.5, 0.5, 0.5, 300);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/fisheye_large_radius.png", tmp_dir))?;
    println!("  Saved: fisheye_large_radius.png");

    // Example 7: Small radius fisheye
    println!("\nExample 7: Small radius (strength=0.8, radius=100)");
    let filter7 = FisheyeFilter::new(0.5, 0.5, 0.8, 100);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/fisheye_small_radius.png", tmp_dir))?;
    println!("  Saved: fisheye_small_radius.png");

    // Example 8: Offset center (top-left quadrant)
    println!("\nExample 8: Offset center at (0.3, 0.3)");
    let filter8 = FisheyeFilter::new(0.3, 0.3, 0.8, 150);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/fisheye_offset_top_left.png", tmp_dir))?;
    println!("  Saved: fisheye_offset_top_left.png");

    // Example 9: Offset center (bottom-right quadrant)
    println!("\nExample 9: Offset center at (0.7, 0.7)");
    let filter9 = FisheyeFilter::new(0.7, 0.7, 0.8, 150);
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/fisheye_offset_bottom_right.png", tmp_dir))?;
    println!("  Saved: fisheye_offset_bottom_right.png");

    // Example 10: Strength comparison
    println!("\nExample 10: Strength comparison (-1.0 to 2.0)");
    for strength in [-1.0, -0.5, 0.0, 0.3, 0.5, 0.8, 1.0, 1.5, 2.0] {
        let filter = FisheyeFilter::new(0.5, 0.5, strength, 200);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/fisheye_strength_{:.1}.png", tmp_dir, strength))?;
    }
    println!("  Saved: fisheye_strength_-1.0.png to fisheye_strength_2.0.png");

    // Example 11: Radius comparison
    println!("\nExample 11: Radius comparison (50 to 300)");
    for radius in [50, 100, 150, 200, 250, 300] {
        let filter = FisheyeFilter::new(0.5, 0.5, 0.8, radius);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/fisheye_radius_{}.png", tmp_dir, radius))?;
    }
    println!("  Saved: fisheye_radius_50.png to fisheye_radius_300.png");

    println!("\n============================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - strength: Distortion intensity (-1.0 to 2.0)");
    println!("    * Positive values create bulge/convex effect (fisheye lens)");
    println!("    * Negative values create pinch/concave effect");
    println!("    * 0.0 = no distortion");
    println!("  - radius: Area of effect in pixels (based on 1080p resolution)");
    println!("  - center_x/center_y: Center position (0.0-1.0 normalized coordinates)");
    println!("\nMathematical formula:");
    println!("  new_dist = dist * (1 + strength * (1 - (dist/radius)^2))");
    println!("  This creates smooth spherical distortion with maximum effect at center");

    Ok(())
}