// Example demonstrating the sketch filter
// Generates test images and saves them to tmp/sketch_demo/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::SketchFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_for_sketch(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([250, 250, 245, 255]));

    // Add a face-like oval shape
    let face_center = (width / 2, height / 2);
    let face_w = (width / 3) as f32;
    let face_h = (height / 2) as f32;
    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 - face_center.0 as f32) / face_w;
            let dy = (y as f32 - face_center.1 as f32) / face_h;
            if dx * dx + dy * dy < 1.0 {
                // Skin tone gradient
                let gradient = (1.0 - (dx * dx + dy * dy).sqrt()) * 30.0;
                let base = 220.0 + gradient;
                img.put_pixel(x, y, Rgba([base as u8, (base - 20.0) as u8, (base - 40.0) as u8, 255]));
            }
        }
    }

    // Add eyes
    let eye_y = height / 2 - height / 8;
    let eye_offset = width / 8;
    for y_offset in -8i32..8 {
        for x_offset in -8i32..8 {
            let dist = ((x_offset * x_offset + y_offset * y_offset) as f32).sqrt();
            // Left eye
            let x = (face_center.0 as i32 - eye_offset as i32 + x_offset)
                .clamp(0, width as i32 - 1) as u32;
            let y = (eye_y as i32 + y_offset).clamp(0, height as i32 - 1) as u32;
            if dist < 6.0 {
                img.put_pixel(x, y, Rgba([40, 40, 40, 255])); // Dark pupil
            } else if dist < 8.0 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255])); // White of eye
            }
            // Right eye
            let x = (face_center.0 as i32 + eye_offset as i32 + x_offset)
                .clamp(0, width as i32 - 1) as u32;
            if dist < 6.0 {
                img.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            } else if dist < 8.0 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }

    // Add nose (simple triangle shadow)
    let nose_base_y = height / 2 + height / 10;
    for y in (height / 2)..nose_base_y {
        let width_factor = (y - height / 2) as f32 / (nose_base_y - height / 2) as f32 * 15.0;
        for x_offset in -(width_factor as i32)..(width_factor as i32 + 1) {
            let x = (face_center.0 as i32 + x_offset).clamp(0, width as i32 - 1) as u32;
            let shade = 200 - x_offset.abs() as u8 * 5;
            img.put_pixel(x, y, Rgba([shade, (shade - 20), (shade - 40), 255]));
        }
    }

    // Add mouth
    let mouth_y = height / 2 + height / 5;
    for x_offset in -20i32..20 {
        let x = (face_center.0 as i32 + x_offset).clamp(0, width as i32 - 1) as u32;
        if mouth_y > 0 && mouth_y < height {
            let curve = ((x_offset as f32 / 20.0).abs() * 5.0) as u32;
            for y_off in 0u32..3 {
                img.put_pixel(x, mouth_y + curve + y_off, Rgba([180, 100, 100, 255]));
            }
        }
    }

    // Add hair (dark area on top)
    for y in 0..(height / 3) {
        for x in 0..width {
            let dx = (x as f32 - face_center.0 as f32) / face_w;
            let dy = (y as f32 - face_center.1 as f32) / face_h;
            if dx * dx + dy * dy < 1.1 && y < height / 3 {
                let gradient = y as f32 / (height / 3) as f32;
                let darkness = (50.0 - gradient * 30.0) as u8;
                img.put_pixel(x, y, Rgba([darkness, darkness, (darkness + 10), 255]));
            }
        }
    }

    // Add background gradient
    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 - face_center.0 as f32) / face_w;
            let dy = (y as f32 - face_center.1 as f32) / face_h;
            if dx * dx + dy * dy >= 1.0 {
                let gradient = y as f32 / height as f32;
                img.put_pixel(x, y, Rgba([
                    (200.0 + gradient * 30.0) as u8,
                    (210.0 + gradient * 20.0) as u8,
                    (220.0 + gradient * 10.0) as u8,
                    255
                ]));
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
    filter: &SketchFilter,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<RgbaImage> {
    let buffer = create_test_image_for_sketch(width, height);

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
    println!("Sketch Filter Demo");
    println!("==================\n");

    // Create tmp directory
    let tmp_dir = "tmp/sketch_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for comparison
    println!("Saving original test image...");
    let original = create_test_image_for_sketch(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default sketch
    println!("\nExample 1: Default sketch (line_intensity=0.8, line_width=3.0)");
    let filter1 = SketchFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/sketch_default.png", tmp_dir))?;
    println!("  Saved: sketch_default.png");

    // Example 2: Light sketch (low intensity)
    println!("\nExample 2: Light sketch (line_intensity=0.4)");
    let filter2 = SketchFilter::new(0.4, 3.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/sketch_light.png", tmp_dir))?;
    println!("  Saved: sketch_light.png");

    // Example 3: Bold sketch (high intensity)
    println!("\nExample 3: Bold sketch (line_intensity=1.0)");
    let filter3 = SketchFilter::new(1.0, 3.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/sketch_bold.png", tmp_dir))?;
    println!("  Saved: sketch_bold.png");

    // Example 4: Thin lines (small line width)
    println!("\nExample 4: Thin lines (line_width=1.0)");
    let filter4 = SketchFilter::new(0.8, 1.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/sketch_thin.png", tmp_dir))?;
    println!("  Saved: sketch_thin.png");

    // Example 5: Thick lines (large line width)
    println!("\nExample 5: Thick lines (line_width=8.0)");
    let filter5 = SketchFilter::new(0.8, 8.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/sketch_thick.png", tmp_dir))?;
    println!("  Saved: sketch_thick.png");

    // Example 6: Line width variations
    println!("\nExample 6: Line width comparison (1.0 to 10.0)");
    for line_width in [1.0, 2.0, 3.0, 5.0, 7.0, 10.0] {
        let filter = SketchFilter::new(0.8, line_width);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/sketch_width_{:.0}.png", tmp_dir, line_width))?;
    }
    println!("  Saved: sketch_width_1.png to sketch_width_10.png");

    // Example 7: Detail level variations
    println!("\nExample 7: Detail level comparison (0.0 to 1.0)");
    for detail in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let filter = SketchFilter::new(0.8, 3.0).with_detail_level(detail);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/sketch_detail_{:.2}.png", tmp_dir, detail))?;
    }
    println!("  Saved: sketch_detail_0.00.png to sketch_detail_1.00.png");

    // Example 8: Colored paper backgrounds
    println!("\nExample 8: Colored paper backgrounds");
    let filter8_blue = SketchFilter::new(0.8, 3.0)
        .with_paper_color([230, 230, 250]); // Blue-tinted paper
    let img8_blue = apply_filter_to_image(&filter8_blue, width, height, fps)?;
    save_image(&img8_blue, &format!("{}/sketch_blue_paper.png", tmp_dir))?;

    let filter8_sepia = SketchFilter::new(0.8, 3.0)
        .with_paper_color([255, 245, 220])
        .with_pencil_color([100, 80, 60]); // Sepia-toned
    let img8_sepia = apply_filter_to_image(&filter8_sepia, width, height, fps)?;
    save_image(&img8_sepia, &format!("{}/sketch_sepia.png", tmp_dir))?;
    println!("  Saved: sketch_blue_paper.png, sketch_sepia.png");

    // Example 9: Colored pencil
    println!("\nExample 9: Colored pencil sketches");
    let filter9_blue = SketchFilter::new(0.8, 3.0)
        .with_pencil_color([50, 50, 150]);
    let img9_blue = apply_filter_to_image(&filter9_blue, width, height, fps)?;
    save_image(&img9_blue, &format!("{}/sketch_blue_pencil.png", tmp_dir))?;

    let filter9_red = SketchFilter::new(0.8, 3.0)
        .with_pencil_color([150, 50, 50]);
    let img9_red = apply_filter_to_image(&filter9_red, width, height, fps)?;
    save_image(&img9_red, &format!("{}/sketch_red_pencil.png", tmp_dir))?;
    println!("  Saved: sketch_blue_pencil.png, sketch_red_pencil.png");

    // Example 10: Soft sketch (low detail, medium intensity)
    println!("\nExample 10: Soft sketch (line_intensity=0.6, line_width=5.0, detail=0.3)");
    let filter10 = SketchFilter::new(0.6, 5.0).with_detail_level(0.3);
    let img10 = apply_filter_to_image(&filter10, width, height, fps)?;
    save_image(&img10, &format!("{}/sketch_soft.png", tmp_dir))?;
    println!("  Saved: sketch_soft.png");

    // Example 11: High detail sketch
    println!("\nExample 11: High detail sketch (line_intensity=0.9, line_width=2.0, detail=1.0)");
    let filter11 = SketchFilter::new(0.9, 2.0).with_detail_level(1.0);
    let img11 = apply_filter_to_image(&filter11, width, height, fps)?;
    save_image(&img11, &format!("{}/sketch_high_detail.png", tmp_dir))?;
    println!("  Saved: sketch_high_detail.png");

    println!("\n==================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - line_intensity: Line darkness (0.0-1.0), higher = darker lines");
    println!("  - line_width: Line thickness via blur radius (1.0-10.0), larger = thicker lines");
    println!("  - paper_color: Background paper color RGB");
    println!("  - pencil_color: Line/pencil color RGB");
    println!("  - detail_level: Detail sensitivity (0.0-1.0), higher = captures more fine details");

    Ok(())
}