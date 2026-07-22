// Example demonstrating the grid filter
// Generates test images and saves them to tmp/grid_demo/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::video::GridFilter,
    filters::traits::{VideoData, VideoFilter, VideoFilterConfig},
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |x, y| {
        let gradient_x = (x as f32 / width as f32 * 180.0) as u8;
        let gradient_y = (y as f32 / height as f32 * 75.0) as u8;
        let r = gradient_x.min(255);
        let g = (gradient_x as u16 + 40).min(255) as u8;
        let b = (gradient_x as u16 + gradient_y as u16 + 40).min(255) as u8;
        Rgba([r, g, b, 255])
    });

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
    filter: &GridFilter,
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
    println!("Grid Filter Demo");
    println!("=================\n");

    let tmp_dir = "tmp/grid_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Default grid (3x3 white lines)
    println!("Example 1: Default grid (3x3, white lines, size=2)");
    let filter1 = GridFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/grid_default.png", tmp_dir))?;
    println!("  Saved: grid_default.png");

    // Example 2: Rule of thirds (3x3)
    println!("\nExample 2: Rule of thirds (3x3, thin yellow lines)");
    let filter2 = GridFilter::new(3, 3, [255, 255, 0, 200], 1);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/grid_thirds.png", tmp_dir))?;
    println!("  Saved: grid_thirds.png");

    // Example 3: Different grid densities
    println!("\nExample 3: Grid density comparison");
    for (rows, cols) in [(2, 2), (4, 4), (6, 6), (8, 8)] {
        let filter = GridFilter::new(rows, cols, [255, 255, 255, 255], 1);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grid_{}x{}.png", tmp_dir, rows, cols))?;
    }
    println!("  Saved: grid_2x2.png, grid_4x4.png, grid_6x6.png, grid_8x8.png");

    // Example 4: Different line sizes
    println!("\nExample 4: Line size comparison");
    for line_size in [1, 2, 4, 8, 16] {
        let filter = GridFilter::new(3, 3, [255, 255, 255, 255], line_size);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grid_line_size_{}.png", tmp_dir, line_size))?;
    }
    println!("  Saved: grid_line_size_1.png to grid_line_size_16.png");

    // Example 5: Different line colors
    println!("\nExample 5: Line color variations");
    let colors: [(&str, [u8; 4]); 5] = [
        ("red", [255, 0, 0, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [0, 100, 255, 255]),
        ("cyan", [0, 255, 255, 255]),
        ("magenta", [255, 0, 255, 255]),
    ];
    for (name, color) in colors {
        let filter = GridFilter::new(4, 4, color, 2);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grid_color_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: grid_color_red.png, grid_color_green.png, grid_color_blue.png, grid_color_cyan.png, grid_color_magenta.png");

    // Example 6: Semi-transparent lines
    println!("\nExample 6: Line opacity comparison");
    for alpha in [50, 100, 150, 200, 255] {
        let filter = GridFilter::new(4, 4, [255, 255, 255, alpha], 2);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grid_alpha_{}.png", tmp_dir, alpha))?;
    }
    println!("  Saved: grid_alpha_50.png to grid_alpha_255.png");

    // Example 7: Non-square grids
    println!("\nExample 7: Non-square grids");
    let non_square: [(&str, u32, u32); 3] = [
        ("horizontal", 6, 2),
        ("vertical", 2, 6),
        ("wide", 2, 8),
    ];
    for (name, rows, cols) in non_square {
        let filter = GridFilter::new(rows, cols, [255, 255, 255, 255], 2);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/grid_{}.png", tmp_dir, name))?;
    }
    println!("  Saved: grid_horizontal.png, grid_vertical.png, grid_wide.png");

    // Example 8: Large line size for bold grid
    println!("\nExample 8: Bold grid (3x3, red lines, size=8)");
    let filter8 = GridFilter::new(3, 3, [255, 50, 50, 255], 8);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/grid_bold.png", tmp_dir))?;
    println!("  Saved: grid_bold.png");

    println!("\n=================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - rows: Number of row divisions (grid has rows-1 horizontal lines)");
    println!("  - columns: Number of column divisions (grid has columns-1 vertical lines)");
    println!("  - line_color: RGBA color of grid lines");
    println!("  - line_size: Width of grid lines in pixels (scaled for frame height)");

    Ok(())
}
