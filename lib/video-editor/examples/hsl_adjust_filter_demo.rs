// Example demonstrating the HSL adjustment filter
// Generates test images and saves them to tmp/hsl_adjust_demo/

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::{HSLAdjustFilter, LuminanceStandard},
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_colors(width: u32, height: u32) -> RgbaImage {
    let img = RgbaImage::from_fn(width, height, |x, y| {
        // Create a gradient image with various colors to demonstrate HSL effects
        let nx = x as f32 / width as f32;
        let ny = y as f32 / height as f32;

        // Divide into color zones
        let zone_x = (nx * 6.0) as i32;
        let zone_y = (ny * 3.0) as i32;

        // Base colors for different zones (creates a colorful test image)
        let (r, g, b) = match (zone_x, zone_y) {
            // Row 1: Primary and secondary colors
            (0, 0) => (255, 0, 0),      // Red
            (1, 0) => (255, 127, 0),    // Orange
            (2, 0) => (255, 255, 0),    // Yellow
            (3, 0) => (0, 255, 0),      // Green
            (4, 0) => (0, 127, 255),    // Cyan
            (5, 0) => (0, 0, 255),      // Blue
            // Row 2: Various saturations
            (0, 1) => (255, 127, 127),  // Light red (low saturation)
            (1, 1) => (200, 100, 50),   // Brown-ish
            (2, 1) => (127, 127, 127),  // Gray
            (3, 1) => (100, 200, 100),  // Light green
            (4, 1) => (127, 200, 255),  // Light blue
            (5, 1) => (200, 127, 255),  // Purple-ish
            // Row 3: Various lightness levels
            (0, 2) => (127, 0, 0),      // Dark red
            (1, 2) => (50, 50, 50),     // Very dark gray
            (2, 2) => (255, 255, 255),  // White
            (3, 2) => (0, 127, 0),      // Dark green
            (4, 2) => (0, 0, 127),      // Dark blue
            (5, 2) => (127, 127, 0),    // Dark yellow/olive
            _ => (200, 200, 220),
        };

        Rgba([r, g, b, 255])
    });

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
    filter: &HSLAdjustFilter,
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
    println!("HSL Adjust Filter Demo");
    println!("======================\n");

    // Create tmp directory
    let tmp_dir = "tmp/hsl_adjust_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (600, 300);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_colors(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Hue shift - rotate colors by 90 degrees
    println!("\nExample 1: Hue shift +90 degrees");
    let filter1 = HSLAdjustFilter::new(90.0, 0.0, 0.0);
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/hue_shift_90.png", tmp_dir))?;
    println!("  Saved: hue_shift_90.png (Red becomes cyan, Green becomes blue)");

    // Example 2: Hue shift - rotate colors by -90 degrees
    println!("\nExample 2: Hue shift -90 degrees");
    let filter2 = HSLAdjustFilter::new(-90.0, 0.0, 0.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/hue_shift_-90.png", tmp_dir))?;
    println!("  Saved: hue_shift_-90.png (Red becomes blue, Green becomes red)");

    // Example 3: Hue shift 180 degrees (inverted colors)
    println!("\nExample 3: Hue shift 180 degrees (color inversion)");
    let filter3 = HSLAdjustFilter::new(180.0, 0.0, 0.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/hue_shift_180.png", tmp_dir))?;
    println!("  Saved: hue_shift_180.png (Red becomes cyan, Green becomes magenta)");

    // Example 4: Increase saturation
    println!("\nExample 4: Increase saturation +0.5");
    let filter4 = HSLAdjustFilter::new(0.0, 0.5, 0.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/saturation_increase.png", tmp_dir))?;
    println!("  Saved: saturation_increase.png (Colors become more vivid)");

    // Example 5: Decrease saturation
    println!("\nExample 5: Decrease saturation -0.5");
    let filter5 = HSLAdjustFilter::new(0.0, -0.5, 0.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/saturation_decrease.png", tmp_dir))?;
    println!("  Saved: saturation_decrease.png (Colors become less vivid)");

    // Example 6: Desaturate completely (grayscale)
    println!("\nExample 6: Full desaturation -1.0 (grayscale)");
    let filter6 = HSLAdjustFilter::new(0.0, -1.0, 0.0);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/grayscale.png", tmp_dir))?;
    println!("  Saved: grayscale.png (Image becomes grayscale)");

    // Example 7: Increase lightness
    println!("\nExample 7: Increase lightness +0.3");
    let filter7 = HSLAdjustFilter::new(0.0, 0.0, 0.3);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/lightness_increase.png", tmp_dir))?;
    println!("  Saved: lightness_increase.png (Colors become lighter)");

    // Example 8: Decrease lightness
    println!("\nExample 8: Decrease lightness -0.3");
    let filter8 = HSLAdjustFilter::new(0.0, 0.0, -0.3);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/lightness_decrease.png", tmp_dir))?;
    println!("  Saved: lightness_decrease.png (Colors become darker)");

    // Example 9: Combined adjustment - warm tint
    println!("\nExample 9: Combined adjustment (warm tint)");
    let filter9 = HSLAdjustFilter::new(-30.0, 0.3, 0.1);
    let img9 = apply_filter_to_image(&filter9, width, height, fps)?;
    save_image(&img9, &format!("{}/warm_tint.png", tmp_dir))?;
    println!("  Saved: warm_tint.png (Hue shift -30, sat +0.3, light +0.1)");

    // Example 10: Combined adjustment - cool tint
    println!("\nExample 10: Combined adjustment (cool tint)");
    let filter10 = HSLAdjustFilter::new(30.0, 0.2, 0.0);
    let img10 = apply_filter_to_image(&filter10, width, height, fps)?;
    save_image(&img10, &format!("{}/cool_tint.png", tmp_dir))?;
    println!("  Saved: cool_tint.png (Hue shift +30, sat +0.2)");

    // Example 11: Preserve luminance (BT709 standard)
    println!("\nExample 11: Hue shift with preserved luminance (BT709)");
    let filter11 = HSLAdjustFilter::new(90.0, 0.0, 0.0)
        .with_preserve_luminance(LuminanceStandard::BT709);
    let img11 = apply_filter_to_image(&filter11, width, height, fps)?;
    save_image(&img11, &format!("{}/preserve_luminance_bt709.png", tmp_dir))?;
    println!("  Saved: preserve_luminance_bt709.png");

    // Example 12: Preserve luminance (BT601 standard)
    println!("\nExample 12: Hue shift with preserved luminance (BT601)");
    let filter12 = HSLAdjustFilter::new(90.0, 0.0, 0.0)
        .with_preserve_luminance(LuminanceStandard::BT601);
    let img12 = apply_filter_to_image(&filter12, width, height, fps)?;
    save_image(&img12, &format!("{}/preserve_luminance_bt601.png", tmp_dir))?;
    println!("  Saved: preserve_luminance_bt601.png");

    // Example 13: Preserve luminance (BT2020 standard)
    println!("\nExample 13: Hue shift with preserved luminance (BT2020)");
    let filter13 = HSLAdjustFilter::new(90.0, 0.0, 0.0)
        .with_preserve_luminance(LuminanceStandard::BT2020);
    let img13 = apply_filter_to_image(&filter13, width, height, fps)?;
    save_image(&img13, &format!("{}/preserve_luminance_bt2020.png", tmp_dir))?;
    println!("  Saved: preserve_luminance_bt2020.png");

    // Example 14: Saturation increase with preserved luminance
    println!("\nExample 14: High saturation with preserved luminance");
    let filter14 = HSLAdjustFilter::new(0.0, 0.8, 0.0)
        .with_preserve_luminance(LuminanceStandard::BT709);
    let img14 = apply_filter_to_image(&filter14, width, height, fps)?;
    save_image(&img14, &format!("{}/high_sat_preserve_lum.png", tmp_dir))?;
    println!("  Saved: high_sat_preserve_lum.png");

    // Example 15: Hue shift comparison series
    println!("\nExample 15: Hue shift comparison series");
    for hue_shift in [0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0] {
        let filter = HSLAdjustFilter::new(hue_shift, 0.0, 0.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/hue_series_{:.0}.png", tmp_dir, hue_shift))?;
    }
    println!("  Saved: hue_series_0.png through hue_series_180.png");

    // Example 16: Saturation comparison series
    println!("\nExample 16: Saturation comparison series");
    for saturation in [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0] {
        let filter = HSLAdjustFilter::new(0.0, saturation, 0.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/sat_series_{:.2}.png", tmp_dir, saturation))?;
    }
    println!("  Saved: sat_series_-1.00.png through sat_series_1.00.png");

    // Example 17: Lightness comparison series
    println!("\nExample 17: Lightness comparison series");
    for lightness in [-0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75] {
        let filter = HSLAdjustFilter::new(0.0, 0.0, lightness);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/light_series_{:.2}.png", tmp_dir, lightness))?;
    }
    println!("  Saved: light_series_-0.75.png through light_series_0.75.png");

    println!("\n======================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - hue_shift: Rotation of hue in degrees (-180 to 180)");
    println!("  - saturation: Color intensity adjustment (-1 = grayscale, 0 = no change, 1 = max)");
    println!("  - lightness: Brightness adjustment (-1 = black, 0 = no change, 1 = white)");
    println!("  - preserve_luminance: Maintains perceived brightness after adjustments");
    println!("  - luminance_standard: BT709 (HDTV), BT601 (SDTV), BT2020 (HDR)");

    Ok(())
}