// Example demonstrating the vignette filter
// Generates test images and saves them to tmp/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::VignetteFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |_, _| Rgba([200, 200, 220, 255]));

    // Add some visual content - a bright colored rectangle in center
    let center_x = width / 2;
    let center_y = height / 2;
    let rect_w = width / 3;
    let rect_h = height / 3;

    for y in (center_y - rect_h/2)..(center_y + rect_h/2) {
        for x in (center_x - rect_w/2)..(center_x + rect_w/2) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([255, 150, 100, 255]));
            }
        }
    }

    // Add border around rectangle
    for y in (center_y - rect_h/2 - 5)..(center_y + rect_h/2 + 5) {
        for x in (center_x - rect_w/2 - 5)..(center_x + rect_w/2 + 5) {
            if x < width && y < height {
                if x < (center_x - rect_w/2) || x >= (center_x + rect_w/2) ||
                   y < (center_y - rect_h/2) || y >= (center_y + rect_h/2) {
                    img.put_pixel(x, y, Rgba([100, 180, 255, 255]));
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
    filter: &VignetteFilter,
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
    println!("Vignette Filter Demo");
    println!("====================\n");

    // Create tmp directory
    let tmp_dir = "tmp/vignette_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Default vignette (centered, moderate intensity)
    println!("Example 1: Default vignette (intensity=0.8, inner=0.3, outer=0.8)");
    let filter1 = VignetteFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/vignette_default.png", tmp_dir))?;
    println!("  Saved: vignette_default.png");

    // Example 2: Strong vignette (corners go nearly black)
    println!("\nExample 2: Strong vignette (intensity=1.0, inner=0.1, outer=0.5)");
    let filter2 = VignetteFilter::new(1.0, 0.1, 0.5);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/vignette_strong.png", tmp_dir))?;
    println!("  Saved: vignette_strong.png");

    // Example 3: Soft vignette (gentle fade)
    println!("\nExample 3: Soft vignette (intensity=0.5, inner=0.4, outer=0.9)");
    let filter3 = VignetteFilter::new(0.5, 0.4, 0.9);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/vignette_soft.png", tmp_dir))?;
    println!("  Saved: vignette_soft.png");

    // Example 4: Offset center vignette
    println!("\nExample 4: Offset center (center at 0.3, 0.3)");
    let filter4 = VignetteFilter::new(0.8, 0.2, 0.7).with_center(0.3, 0.3);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/vignette_offset_center.png", tmp_dir))?;
    println!("  Saved: vignette_offset_center.png");

    // Example 5: Circular vignette (forced aspect ratio 1.0)
    println!("\nExample 5: Circular vignette (aspect=1.0)");
    let filter5 = VignetteFilter::new(0.9, 0.2, 0.6).with_aspect(1.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/vignette_circular.png", tmp_dir))?;
    println!("  Saved: vignette_circular.png");

    // Example 6: Wide elliptical vignette
    println!("\nExample 6: Wide elliptical vignette (aspect=2.0)");
    let filter6 = VignetteFilter::new(0.8, 0.2, 0.6).with_aspect(2.0);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/vignette_wide.png", tmp_dir))?;
    println!("  Saved: vignette_wide.png");

    // Example 7: Minimal vignette (subtle effect)
    println!("\nExample 7: Minimal vignette (intensity=0.3, large inner radius)");
    let filter7 = VignetteFilter::new(0.3, 0.5, 0.95);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/vignette_minimal.png", tmp_dir))?;
    println!("  Saved: vignette_minimal.png");

    // Example 8: Intensity comparison
    println!("\nExample 8: Intensity comparison (0.2 to 1.0)");
    for intensity in [0.2, 0.4, 0.6, 0.8, 1.0] {
        let filter = VignetteFilter::new(intensity, 0.3, 0.8);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/vignette_intensity_{:.1}.png", tmp_dir, intensity))?;
    }
    println!("  Saved: vignette_intensity_0.2.png to vignette_intensity_1.0.png");

    // Example 9: Inner/outer radius comparison
    println!("\nExample 9: Radius comparison (different inner/outer combinations)");
    let radius_combos = [
        ("tight", 0.1, 0.3),    // Small bright area, quick transition
        ("medium", 0.3, 0.6),   // Medium bright area
        ("wide", 0.4, 0.9),     // Large bright area, slow transition
        ("edge_only", 0.7, 0.95), // Only edges are darkened
    ];
    for (name, inner, outer) in radius_combos {
        let filter = VignetteFilter::new(0.9, inner, outer);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/vignette_radius_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: vignette_radius_tight.png, medium.png, wide.png, edge_only.png");

    println!("\n====================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - intensity: How dark the vignette gets (0.0 = no effect, 1.0 = black)");
    println!("  - inner_radius: Distance from center where vignette starts (0.0-1.0)");
    println!("  - outer_radius: Distance from center where vignette reaches max darkness (0.0-1.0)");
    println!("  - center_x/center_y: Position of vignette center (0.0-1.0, 0.5 = image center)");
    println!("  - aspect: Shape of vignette (1.0 = circular, image aspect = elliptical)");

    Ok(())
}