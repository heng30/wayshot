// Example demonstrating the grain filter
// Generates test images and saves them to tmp/grain_demo/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::GrainFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a gradient background
        let gradient_x = (x as f32 / width as f32 * 180.0) as u8;
        let gradient_y = (y as f32 / height as f32 * 75.0) as u8;
        let r = gradient_x.min(255);
        let g = (gradient_x as u16 + 40).min(255) as u8;
        let b = (gradient_x as u16 + gradient_y as u16 + 40).min(255) as u8;
        Rgba([r, g, b, 255])
    });

    // Add a colored rectangle in center for visual contrast
    let center_x = width / 2;
    let center_y = height / 2;
    let rect_w = width / 3;
    let rect_h = height / 3;

    let rect_start_x = center_x.saturating_sub(rect_w / 2);
    let rect_end_x = center_x.saturating_add(rect_w / 2).min(width);
    let rect_start_y = center_y.saturating_sub(rect_h / 2);
    let rect_end_y = center_y.saturating_add(rect_h / 2).min(height);

    for y in rect_start_y..rect_end_y {
        for x in rect_start_x..rect_end_x {
            img.put_pixel(x, y, Rgba([255, 180, 120, 255]));
        }
    }

    // Add smooth areas to better see the grain effect
    // Top-left corner: smooth gray area
    let gray_end_y = height / 4;
    let gray_end_x = width / 4;
    for y in 0..gray_end_y {
        for x in 0..gray_end_x {
            img.put_pixel(x, y, Rgba([128, 128, 128, 255]));
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
    filter: &GrainFilter,
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
    println!("Grain Filter Demo");
    println!("==================\n");

    // Create tmp directory
    let tmp_dir = "tmp/grain_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Default grain (moderate intensity)
    println!("Example 1: Default grain (intensity=0.3, grain_size=2.0, monochrome)");
    let filter1 = GrainFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/grain_default.png", tmp_dir))?;
    println!("  Saved: grain_default.png");

    // Example 2: Strong grain (old film look)
    println!("\nExample 2: Strong grain (intensity=0.6, grain_size=3.0) - old film look");
    let filter2 = GrainFilter::new(0.6).with_grain_size(3.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/grain_strong.png", tmp_dir))?;
    println!("  Saved: grain_strong.png");

    // Example 3: Subtle grain (light texture)
    println!("\nExample 3: Subtle grain (intensity=0.1) - light texture");
    let filter3 = GrainFilter::new(0.1);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/grain_subtle.png", tmp_dir))?;
    println!("  Saved: grain_subtle.png");

    // Example 4: Colored grain vs monochrome
    println!("\nExample 4: Colored grain (intensity=0.4, colored=true)");
    let filter4 = GrainFilter::new(0.4).with_colored(true);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/grain_colored.png", tmp_dir))?;
    println!("  Saved: grain_colored.png");

    // Example 5: Different grain sizes
    println!("\nExample 5: Grain size comparison (sizes 1.0 to 10.0)");
    for grain_size in [1.0, 2.0, 4.0, 6.0, 10.0] {
        let filter = GrainFilter::new(0.4).with_grain_size(grain_size);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grain_size_{:.0}.png", tmp_dir, grain_size))?;
    }
    println!("  Saved: grain_size_1.png, grain_size_2.png, grain_size_4.png, grain_size_6.png, grain_size_10.png");

    // Example 6: Roughness variations
    println!("\nExample 6: Roughness comparison (0.0 to 1.0)");
    for roughness in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let filter = GrainFilter::new(0.4).with_roughness(roughness);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grain_roughness_{:.2}.png", tmp_dir, roughness))?;
    }
    println!("  Saved: grain_roughness_0.00.png to grain_roughness_1.00.png");

    // Example 7: High roughness with colored grain (dramatic old film)
    println!("\nExample 7: Dramatic old film (intensity=0.5, roughness=0.9, colored)");
    let filter7 = GrainFilter::new(0.5).with_roughness(0.9).with_colored(true);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/grain_dramatic.png", tmp_dir))?;
    println!("  Saved: grain_dramatic.png");

    // Example 8: Intensity comparison
    println!("\nExample 8: Intensity comparison (0.1 to 0.9)");
    for intensity in [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] {
        let filter = GrainFilter::new(intensity);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grain_intensity_{:.1}.png", tmp_dir, intensity))?;
    }
    println!("  Saved: grain_intensity_0.1.png to grain_intensity_0.9.png");

    // Example 9: Different seeds (for testing determinism)
    println!("\nExample 9: Different seeds (seed 0, 1, 2, 3, 4)");
    for seed in 0u32..5 {
        let filter = GrainFilter::new(0.4).with_seed(seed);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grain_seed_{}.png", tmp_dir, seed))?;
    }
    println!("  Saved: grain_seed_0.png to grain_seed_4.png");

    // Example 10: Film noir look (high intensity, high roughness, monochrome)
    println!("\nExample 10: Film noir look (intensity=0.8, roughness=1.0, grain_size=4.0)");
    let filter10 = GrainFilter::new(0.8).with_roughness(1.0).with_grain_size(4.0);
    let img10 = apply_filter_to_image(&filter10, width, height, fps)?;
    save_image(&img10, &format!("{}/grain_noir.png", tmp_dir))?;
    println!("  Saved: grain_noir.png");

    println!("\n==================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - intensity: Amount of grain added (0.0-1.0)");
    println!("  - grain_size: Size of grain particles (1.0-10.0), larger = more visible particles");
    println!("  - colored: true = separate noise per RGB channel, false = same noise for all");
    println!("  - roughness: Grain contrast/clumping (0.0 = smooth, 1.0 = harsh, high contrast)");
    println!("  - seed: Random seed for consistent grain pattern across frames");

    Ok(())
}