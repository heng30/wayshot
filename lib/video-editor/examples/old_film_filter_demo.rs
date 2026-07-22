// Example demonstrating the old film filter
// Generates test images and saves them to tmp/old_film_demo directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::OldFilmFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        // Vintage-style gradient background
        let gradient = (x as f32 / width as f32 * 80.0 + y as f32 / height as f32 * 80.0) as u8;
        Rgba([
            (gradient as u16 + 120).min(255) as u8,
            (gradient as u16 + 100).min(255) as u8,
            (gradient as u16 + 90).min(255) as u8,
            255
        ])
    });

    // Add a person-like shape (good for showing film effects)
    let center_x = width / 2;
    let center_y = height / 2;

    // Head (circle)
    let head_radius = 40;
    let head_y = center_y.saturating_sub(60);
    for y in head_y.saturating_sub(head_radius)..(head_y + head_radius).min(height) {
        for x in center_x.saturating_sub(head_radius)..(center_x + head_radius).min(width) {
            if x < width && y < height {
                let dx = x as i32 - center_x as i32;
                let dy = y as i32 - head_y as i32;
                if dx * dx + dy * dy < (head_radius * head_radius) as i32 {
                    img.put_pixel(x, y, Rgba([220, 180, 160, 255]));
                }
            }
        }
    }

    // Body (rectangle)
    let body_w = 60;
    let body_h = 100;
    for y in center_y..(center_y + body_h) {
        for x in (center_x - body_w/2)..(center_x + body_w/2) {
            if x < width && y < height {
                img.put_pixel(x, y, Rgba([100, 80, 150, 255]));
            }
        }
    }

    // Background elements
    // Trees (vertical rectangles)
    for i in 0..3 {
        let tree_x = width / 4 + i * width / 4;
        let tree_w = 20;
        let tree_h = 150;
        for y in (height - tree_h)..height {
            for x in (tree_x - tree_w/2)..(tree_x + tree_w/2) {
                if x < width && y < height {
                    img.put_pixel(x, y, Rgba([60, 100, 60, 255]));
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
    filter: &OldFilmFilter,
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
    println!("Old Film Filter Demo");
    println!("====================\n");

    // Create tmp directory
    let tmp_dir = "tmp/old_film_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Save original image for reference
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Example 1: Default old film (all effects moderate)
    println!("\nExample 1: Default old film (moderate all effects)");
    let filter1 = OldFilmFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/old_film_default.png", tmp_dir))?;
    println!("  Saved: old_film_default.png");

    // Example 2: Heavy scratches (high scratch intensity)
    println!("\nExample 2: Heavy scratches (scratch_intensity=0.8)");
    let filter2 = OldFilmFilter::default().with_scratch_intensity(0.8);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/old_film_heavy_scratches.png", tmp_dir))?;
    println!("  Saved: old_film_heavy_scratches.png");

    // Example 3: Sepia only (other effects disabled)
    println!("\nExample 3: Sepia only (sepia_intensity=0.5, other effects off)");
    let filter3 = OldFilmFilter::default()
        .with_sepia_intensity(0.5)
        .with_scratch_intensity(0.0)
        .with_dust_intensity(0.0)
        .with_flicker_intensity(0.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/old_film_sepia_only.png", tmp_dir))?;
    println!("  Saved: old_film_sepia_only.png");

    // Example 4: Damaged film (all effects strong)
    println!("\nExample 4: Damaged film (all effects strong)");
    let filter4 = OldFilmFilter::default()
        .with_seed(42)
        .with_scratch_intensity(0.7)
        .with_dust_intensity(0.6)
        .with_flicker_intensity(0.25)
        .with_sepia_intensity(0.6);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/old_film_damaged.png", tmp_dir))?;
    println!("  Saved: old_film_damaged.png");

    // Example 5: Dust only
    println!("\nExample 5: Dust only (dust_intensity=0.5, other effects off)");
    let filter5 = OldFilmFilter::default()
        .with_dust_intensity(0.5)
        .with_scratch_intensity(0.0)
        .with_sepia_intensity(0.0)
        .with_flicker_intensity(0.0);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/old_film_dust_only.png", tmp_dir))?;
    println!("  Saved: old_film_dust_only.png");

    // Example 6: Flicker only (brightness variation)
    println!("\nExample 6: Flicker only (flicker_intensity=0.2)");
    let filter6 = OldFilmFilter::default()
        .with_flicker_intensity(0.2)
        .with_scratch_intensity(0.0)
        .with_dust_intensity(0.0)
        .with_sepia_intensity(0.0);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/old_film_flicker_only.png", tmp_dir))?;
    println!("  Saved: old_film_flicker_only.png");

    // Example 7: Scratch intensity comparison
    println!("\nExample 7: Scratch intensity comparison");
    for intensity in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
        let filter = OldFilmFilter::default()
            .with_seed(10)
            .with_scratch_intensity(intensity)
            .with_dust_intensity(0.0)
            .with_sepia_intensity(0.0)
            .with_flicker_intensity(0.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/old_film_scratch_{:.1}.png", tmp_dir, intensity))?;
    }
    println!("  Saved: old_film_scratch_0.0.png through old_film_scratch_1.0.png");

    // Example 8: Sepia intensity comparison
    println!("\nExample 8: Sepia intensity comparison");
    for intensity in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
        let filter = OldFilmFilter::default()
            .with_sepia_intensity(intensity)
            .with_scratch_intensity(0.0)
            .with_dust_intensity(0.0)
            .with_flicker_intensity(0.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/old_film_sepia_{:.1}.png", tmp_dir, intensity))?;
    }
    println!("  Saved: old_film_sepia_0.0.png through old_film_sepia_1.0.png");

    // Example 9: Dust intensity comparison
    println!("\nExample 9: Dust intensity comparison");
    for intensity in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
        let filter = OldFilmFilter::default()
            .with_seed(20)
            .with_dust_intensity(intensity)
            .with_scratch_intensity(0.0)
            .with_sepia_intensity(0.0)
            .with_flicker_intensity(0.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/old_film_dust_{:.1}.png", tmp_dir, intensity))?;
    }
    println!("  Saved: old_film_dust_0.0.png through old_film_dust_1.0.png");

    // Example 10: Combined effects with different seeds
    println!("\nExample 10: Different random seeds (same settings)");
    for seed in [0, 42, 100, 256, 1000] {
        let filter = OldFilmFilter::default()
            .with_seed(seed)
            .with_scratch_intensity(0.5)
            .with_dust_intensity(0.4)
            .with_sepia_intensity(0.3);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/old_film_seed_{}.png", tmp_dir, seed))?;
    }
    println!("  Saved: old_film_seed_0.png through old_film_seed_1000.png");

    println!("\n====================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - scratch_intensity (0-1): Intensity of vertical scratch lines");
    println!("  - dust_intensity (0-1): Intensity of dust particles");
    println!("  - flicker_intensity (0-0.3): Brightness variation amount");
    println!("  - sepia_intensity (0-1): Sepia tone color shift");
    println!("  - jitter_intensity (0-10): Frame position displacement");
    println!("  - vertical_lines_intensity (0-1): Static artifact lines");
    println!("  - seed: Random seed for reproducible effects");

    Ok(())
}