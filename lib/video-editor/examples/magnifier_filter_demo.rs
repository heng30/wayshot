// Example demonstrating the magnifier filter
// Generates test images and saves them to tmp/magnifier_demo/

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::MagnifierFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_grid(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a checkerboard pattern for better visibility of magnification
        let checker_size = 40;
        let is_white = ((x / checker_size) + (y / checker_size)) % 2 == 0;
        if is_white {
            Rgba([240, 240, 240, 255])
        } else {
            Rgba([180, 180, 200, 255])
        }
    });

    // Add colored circles to make magnification more visible
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

    // Add some small text-like patterns for detail
    // Draw small squares near the center to show detail magnification
    for i in 0..5 {
        let offset = i * 20;
        // Small squares near center
        for y in (center_y + 80 + offset)..(center_y + 95 + offset) {
            for x in (center_x + offset)..(center_x + 15 + offset) {
                if x < width && y < height {
                    img.put_pixel(x, y, Rgba([200, 100, 200, 255]));
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
    filter: &MagnifierFilter,
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
    println!("Magnifier Filter Demo");
    println!("====================\n");

    // Create tmp directory
    let tmp_dir = "tmp/magnifier_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Default magnifier (2x magnification)
    println!("Example 1: Default magnifier (scale=2.0, radius=100)");
    let filter1 = MagnifierFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/magnifier_default.png", tmp_dir))?;
    println!("  Saved: magnifier_default.png");

    // Example 2: Higher magnification (3x)
    println!("\nExample 2: Higher magnification (scale=3.0, radius=100)");
    let filter2 = MagnifierFilter::new(0.5, 0.5, 100, 3.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/magnifier_3x.png", tmp_dir))?;
    println!("  Saved: magnifier_3x.png");

    // Example 3: Higher magnification (5x)
    println!("\nExample 3: Higher magnification (scale=5.0, radius=100)");
    let filter3 = MagnifierFilter::new(0.5, 0.5, 100, 5.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/magnifier_5x.png", tmp_dir))?;
    println!("  Saved: magnifier_5x.png");

    // Example 4: Larger magnifier
    println!("\nExample 4: Larger magnifier (scale=2.0, radius=150)");
    let filter4 = MagnifierFilter::new(0.5, 0.5, 150, 2.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/magnifier_large.png", tmp_dir))?;
    println!("  Saved: magnifier_large.png");

    // Example 5: Small magnifier
    println!("\nExample 5: Small magnifier (scale=3.0, radius=50)");
    let filter5 = MagnifierFilter::new(0.5, 0.5, 50, 3.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/magnifier_small.png", tmp_dir))?;
    println!("  Saved: magnifier_small.png");

    // Example 6: Offset center (top-left quadrant)
    println!("\nExample 6: Offset center at (0.3, 0.3)");
    let filter6 = MagnifierFilter::new(0.3, 0.3, 100, 3.0);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/magnifier_offset_top_left.png", tmp_dir))?;
    println!("  Saved: magnifier_offset_top_left.png");

    // Example 7: Offset center (bottom-right quadrant)
    println!("\nExample 7: Offset center at (0.7, 0.7)");
    let filter7 = MagnifierFilter::new(0.7, 0.7, 100, 3.0);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/magnifier_offset_bottom_right.png", tmp_dir))?;
    println!("  Saved: magnifier_offset_bottom_right.png");

    // Example 8: Magnifier near edge (testing boundary clipping)
    println!("\nExample 8: Magnifier near edge at (0.1, 0.1)");
    let filter8 = MagnifierFilter::new(0.1, 0.1, 100, 2.0);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/magnifier_near_edge.png", tmp_dir))?;
    println!("  Saved: magnifier_near_edge.png");

    // Example 9: Custom border color
    println!("\nExample 9: Red border");
    let filter9 = MagnifierFilter::new(0.5, 0.5, 100, 2.0)
        .with_border_color(Some((255, 0, 0, 255)));
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/magnifier_red_border.png", tmp_dir))?;
    println!("  Saved: magnifier_red_border.png");

    // Example 10: Thicker border
    println!("\nExample 10: Thicker border (width=5)");
    let filter10 = MagnifierFilter::new(0.5, 0.5, 100, 2.0)
        .with_border_width(5);
    let img10 = apply_filter_to_image(&filter10, width, height, fps)?;
    save_image(&img10, &format!("{}/magnifier_thick_border.png", tmp_dir))?;
    println!("  Saved: magnifier_thick_border.png");

    // Example 11: No border
    println!("\nExample 11: No border");
    let filter11 = MagnifierFilter::new(0.5, 0.5, 100, 2.0)
        .with_border_color(None);
    let img11 = apply_filter_to_image(&filter11, width, height, fps)?;
    save_image(&img11, &format!("{}/magnifier_no_border.png", tmp_dir))?;
    println!("  Saved: magnifier_no_border.png");

    // Example 12: Scale comparison
    println!("\nExample 12: Scale comparison (1.5 to 8.0)");
    for scale in [1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0] {
        let filter = MagnifierFilter::new(0.5, 0.5, 100, scale);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/magnifier_scale_{:.1}.png", tmp_dir, scale))?;
    }
    println!("  Saved: magnifier_scale_1.5.png to magnifier_scale_8.0.png");

    // Example 13: Radius comparison
    println!("\nExample 13: Radius comparison (50 to 200)");
    for radius in [50, 75, 100, 150, 200] {
        let filter = MagnifierFilter::new(0.5, 0.5, radius, 2.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/magnifier_radius_{}.png", tmp_dir, radius))?;
    }
    println!("  Saved: magnifier_radius_50.png to magnifier_radius_200.png");

    println!("\n====================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - center_x/center_y: Center position (0.0-1.0 normalized coordinates)");
    println!("  - radius: Fixed magnifier circle radius in pixels (based on 1080p resolution)");
    println!("  - scale: Magnification factor (1.0-10.0)");
    println!("    * Inside magnifier: shows content from smaller source area (radius/scale)");
    println!("    * Outside magnifier: shows original unchanged");
    println!("  - border_color: Border color (R, G, B, A) - None for no border");
    println!("  - border_width: Border width in pixels");
    println!("\nDifference from local_magnify:");
    println!("  - local_magnify: selection area -> magnified -> larger output circle");
    println!("  - magnifier: fixed-size circle -> inside shows magnified content -> outside unchanged");
    println!("\nBoundary behavior:");
    println!("  - When magnifier samples outside image bounds, shows transparent");

    Ok(())
}