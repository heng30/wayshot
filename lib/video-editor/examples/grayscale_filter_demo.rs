// Example demonstrating the grayscale filter
// Generates test images and saves them to tmp/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{GrayscaleFilter, LuminanceStandard},
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_colors(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a gradient with various colors to demonstrate grayscale conversion
        let hue = (x as f32 / width as f32) * 360.0;
        let saturation = 0.8;
        let lightness = 0.5 + (y as f32 / height as f32) * 0.3;

        // Convert HSL to RGB
        let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let x2 = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
        let m = lightness - c / 2.0;

        let (r, g, b) = if hue < 60.0 {
            (c, x2, 0.0)
        } else if hue < 120.0 {
            (x2, c, 0.0)
        } else if hue < 180.0 {
            (0.0, c, x2)
        } else if hue < 240.0 {
            (0.0, x2, c)
        } else if hue < 300.0 {
            (x2, 0.0, c)
        } else {
            (c, 0.0, x2)
        };

        Rgba([
            ((r + m) * 255.0) as u8,
            ((g + m) * 255.0) as u8,
            ((b + m) * 255.0) as u8,
            255,
        ])
    });

    // Add some distinct colored regions for better demonstration
    let center_x = width / 2;
    let center_y = height / 2;

    // Red square
    let rect_size = 40;
    for y in (center_y - rect_size)..(center_y + rect_size) {
        for x in (center_x - rect_size - 100)..(center_x - rect_size) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([255, 50, 50, 255]));
            }
        }
    }

    // Green square
    for y in (center_y - rect_size)..(center_y + rect_size) {
        for x in (center_x - rect_size)..(center_x + rect_size) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([50, 255, 50, 255]));
            }
        }
    }

    // Blue square
    for y in (center_y - rect_size)..(center_y + rect_size) {
        for x in (center_x + rect_size)..(center_x + rect_size + 100) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([50, 50, 255, 255]));
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
    filter: &GrayscaleFilter,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<RgbaImage> {
    let buffer = create_test_image_with_colors(width, height);

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
    println!("Grayscale Filter Demo");
    println!("=====================\n");

    // Create tmp directory
    let tmp_dir = "tmp/grayscale_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original color image for reference");
    let original = create_test_image_with_colors(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Full grayscale (default)
    println!("\nExample 1: Full grayscale (intensity=1.0, contrast=0.0)");
    let filter1 = GrayscaleFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/grayscale_full.png", tmp_dir))?;
    println!("  Saved: grayscale_full.png");

    // Example 2: Partial grayscale (50% intensity)
    println!("\nExample 2: Partial grayscale (intensity=0.5)");
    let filter2 = GrayscaleFilter::new(0.5);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/grayscale_half.png", tmp_dir))?;
    println!("  Saved: grayscale_half.png");

    // Example 3: Light grayscale tint (25% intensity)
    println!("\nExample 3: Light grayscale tint (intensity=0.25)");
    let filter3 = GrayscaleFilter::new(0.25);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/grayscale_light.png", tmp_dir))?;
    println!("  Saved: grayscale_light.png");

    // Example 4: High contrast grayscale
    println!("\nExample 4: High contrast grayscale (intensity=1.0, contrast=0.5)");
    let filter4 = GrayscaleFilter::default().with_contrast(0.5);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/grayscale_high_contrast.png", tmp_dir))?;
    println!("  Saved: grayscale_high_contrast.png");

    // Example 5: Maximum contrast (black or white)
    println!("\nExample 5: Maximum contrast (intensity=1.0, contrast=1.0)");
    let filter5 = GrayscaleFilter::default().with_contrast(1.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/grayscale_max_contrast.png", tmp_dir))?;
    println!("  Saved: grayscale_max_contrast.png");

    // Example 6: Low contrast (soft gray)
    println!("\nExample 6: Low contrast (intensity=1.0, contrast=-0.5)");
    let filter6 = GrayscaleFilter::default().with_contrast(-0.5);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/grayscale_low_contrast.png", tmp_dir))?;
    println!("  Saved: grayscale_low_contrast.png");

    // Example 7: Minimum contrast (all gray)
    println!("\nExample 7: Minimum contrast (intensity=1.0, contrast=-1.0)");
    let filter7 = GrayscaleFilter::default().with_contrast(-1.0);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/grayscale_min_contrast.png", tmp_dir))?;
    println!("  Saved: grayscale_min_contrast.png");

    // Example 8: Different luminance standards
    println!("\nExample 8: Luminance standards comparison");
    for (name, standard) in [
        ("BT709", LuminanceStandard::BT709),
        ("BT601", LuminanceStandard::BT601),
        ("BT2020", LuminanceStandard::BT2020),
    ] {
        let filter = GrayscaleFilter::default().with_luminance_standard(standard);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grayscale_{}.png", tmp_dir, name))?;
        println!("  Saved: grayscale_{}.png", name);
    }

    // Example 9: Intensity comparison
    println!("\nExample 9: Intensity comparison (0.0 to 1.0)");
    for intensity in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let filter = GrayscaleFilter::new(intensity);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grayscale_intensity_{}.png", tmp_dir, intensity))?;
    }
    println!("  Saved: grayscale_intensity_0.0.png to grayscale_intensity_1.0.png");

    // Example 10: Contrast comparison
    println!("\nExample 10: Contrast comparison (-1.0 to 1.0)");
    for contrast in [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0] {
        let filter = GrayscaleFilter::default().with_contrast(contrast);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grayscale_contrast_{}.png", tmp_dir, contrast))?;
    }
    println!("  Saved: grayscale_contrast_-1.0.png to grayscale_contrast_1.0.png");

    println!("\n=====================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - intensity: Grayscale strength (0.0-1.0)");
    println!("    * 0.0 = original color (no effect)");
    println!("    * 1.0 = full grayscale");
    println!("    * Values between create partial desaturation");
    println!("  - contrast: Contrast adjustment (-1.0 to 1.0)");
    println!("    * -1.0 = minimum contrast (all mid-gray)");
    println!("    * 0.0 = no contrast change");
    println!("    * 1.0 = maximum contrast (black or white)");
    println!("  - luminance_standard: Formula for calculating brightness");
    println!("    * BT709: Standard for HD video (R=0.2126, G=0.7152, B=0.0722)");
    println!("    * BT601: Standard for SD video (R=0.299, G=0.587, B=0.114)");
    println!("    * BT2020: Standard for HDR/UHD (R=0.2627, G=0.6780, B=0.0593)");

    Ok(())
}