// Example demonstrating the device frame filter
// Generates test images and saves them to tmp/device_frame_demo/ directory

use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    filters::{
        traits::{VideoData, VideoFilter, VideoFilterConfig},
        video::DeviceFrameFilter,
    },
    metadata::Metadata,
    tracks::{segment::Segment, video_frame_cache::VideoImage},
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a test image with colorful content so the device frame effect is visible.
fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([30, 30, 60, 255]));

    // Sky gradient (top half)
    for y in 0..height / 2 {
        let t = y as f32 / (height / 2) as f32;
        let r = (100.0 + 155.0 * t) as u8;
        let g = (150.0 + 105.0 * t) as u8;
        let b = 255;
        for x in 0..width {
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    // Ground gradient (bottom half)
    for y in height / 2..height {
        let t = (y - height / 2) as f32 / (height / 2) as f32;
        let r = (80.0 - 30.0 * t) as u8;
        let g = (160.0 - 60.0 * t) as u8;
        let b = (80.0 - 30.0 * t) as u8;
        for x in 0..width {
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    // Sun circle
    let sun_cx = width * 3 / 4;
    let sun_cy = height / 4;
    let sun_r = (height / 10) as f32;
    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 - sun_cx as f32).powi(2);
            let dy = (y as f32 - sun_cy as f32).powi(2);
            if dx + dy < sun_r * sun_r {
                img.put_pixel(x, y, Rgba([255, 230, 100, 255]));
            }
        }
    }

    // Mountains
    for x in 0..width {
        let peak_y = height / 2
            - (40.0 * (x as f32 * 0.01).sin() + 30.0 * (x as f32 * 0.023).sin()) as u32;
        for y in peak_y.min(height)..height / 2 {
            let existing = img.get_pixel(x, y);
            if existing[0] < 120 && existing[1] < 120 {
                let shade = (100 + (x / 5) % 30) as u8;
                img.put_pixel(x, y, Rgba([shade, shade + 20, shade, 255]));
            }
        }
    }

    // House
    let house_x = width / 4;
    let house_y = height * 3 / 5;
    let house_w = width / 6;
    let house_h = height / 6;
    for y in house_y..house_y + house_h {
        for x in house_x..house_x + house_w {
            img.put_pixel(x, y, Rgba([180, 120, 80, 255]));
        }
    }
    // Roof
    for y in (house_y - house_h / 3)..house_y {
        for x in (house_x - house_w / 6)..(house_x + house_w + house_w / 6) {
            let roof_top = house_y - house_h / 3;
            let center_x = house_x + house_w / 2;
            let dist_from_center = (x as i32 - center_x as i32).unsigned_abs();
            let max_dist = (house_w / 2 + house_w / 6) as u32;
            let y_range = (house_y - roof_top) as f32;
            let max_x_at_y = (max_dist as f32 * (y - roof_top) as f32 / y_range) as u32;
            if dist_from_center <= max_x_at_y {
                img.put_pixel(x, y, Rgba([160, 60, 50, 255]));
            }
        }
    }

    // Text-like area
    let text_x = width / 2 - width / 5;
    let text_y = height * 2 / 3;
    let text_w = width * 2 / 5;
    let text_h = height / 5;
    for y in text_y..text_y + text_h {
        for x in text_x..text_x + text_w {
            img.put_pixel(x, y, Rgba([255, 255, 255, 200]));
        }
    }
    for i in 0..4 {
        let line_y = text_y + 10 + i * (text_h / 5);
        let line_w = text_w - 20 - (i * 30).min(text_w / 3);
        for x in text_x + 10..text_x + 10 + line_w {
            if line_y < text_y + text_h - 5 {
                img.put_pixel(x, line_y, Rgba([80, 80, 80, 255]));
            }
        }
    }

    img
}

fn create_dummy_segment() -> Arc<Segment> {
    let metadata = Arc::new(Metadata {
        path: PathBuf::from("dummy.mp4"),
        size: 0,
        bitrate: 0,
        duration: Duration::from_secs(10),
        format: vec![],
        videos: vec![],
        audios: vec![],
        subtitles: vec![],
    });
    Arc::new(Segment::new(
        Duration::ZERO,
        Duration::from_secs(10),
        metadata,
        1.0,
    ))
}

fn apply_filter_to_image(
    filter: &DeviceFrameFilter,
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
    image
        .save(path)
        .map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

fn main() -> Result<()> {
    println!("Device Frame Filter Demo");
    println!("========================\n");

    let tmp_dir = "tmp/device_frame_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (720, 1280);
    let fps = 30.0;

    // Save original image for comparison
    println!("Saving original test image...");
    let original = create_test_image_with_content(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Phones
    let phones = [
        ("Apple iPhone X Black", "iphone_x", 720, 1280),
        ("Apple iPhone 7 Jet Black", "iphone_7", 750, 1334),
        ("Google Pixel Very Silver", "pixel", 1080, 1920),
        ("Samsung Galaxy S5 Black", "galaxy_s5", 1080, 1920),
        ("Nexus 5x", "nexus_5x", 1080, 1920),
    ];

    println!("\n--- Phones ---");
    for (name, file, w, h) in &phones {
        println!("  {} ({}x{})...", name, w, h);
        let filter = DeviceFrameFilter::new().with_device_name(*name);
        let img = apply_filter_to_image(&filter, *w, *h, fps)?;
        save_image(&img, &format!("{}/{}.png", tmp_dir, file))?;
    }

    // Tablets
    let tablets = [
        ("Apple iPad Air 2 Silver", "ipad_air_2", 1536, 2048),
        ("Apple iPad Pro Silver", "ipad_pro", 2048, 2732),
        ("Apple iPad Mini 4 Silver", "ipad_mini_4", 1536, 2048),
        ("Microsoft Surface Pro 4", "surface_pro_4", 2736, 1824),
        ("Nexus 9", "nexus_9", 1536, 2048),
    ];

    println!("\n--- Tablets ---");
    for (name, file, w, h) in &tablets {
        println!("  {} ({}x{})...", name, w, h);
        let filter = DeviceFrameFilter::new().with_device_name(*name);
        let img = apply_filter_to_image(&filter, *w, *h, fps)?;
        save_image(&img, &format!("{}/{}.png", tmp_dir, file))?;
    }

    // Computers
    let computers = [
        ("Apple-Macbook-Space-Grey", "macbook", 2304, 1440),
        ("Apple Macbook Air 13\"", "macbook_air_13", 1440, 900),
        ("Apple iMac", "imac", 2560, 1440),
        ("Dell XPS 13\"", "dell_xps_13", 3200, 1800),
        ("Microsoft Surface Book", "surface_book", 3000, 2000),
    ];

    println!("\n--- Computers ---");
    for (name, file, w, h) in &computers {
        println!("  {} ({}x{})...", name, w, h);
        let filter = DeviceFrameFilter::new().with_device_name(*name);
        let img = apply_filter_to_image(&filter, *w, *h, fps)?;
        save_image(&img, &format!("{}/{}.png", tmp_dir, file))?;
    }

    // Displays
    let displays = [
        ("Apple Thunderbolt Display", "thunderbolt", 2560, 1440),
        ("Dell UltraSharp 27\"", "dell_27", 2560, 1440),
        ("Dell UltraSharp 24\"", "dell_24", 1920, 1200),
        ("Dell UltraSharp 5K Monitor 27\"", "dell_5k", 3930, 2880),
        ("Sony W850C", "sony_w850c", 1280, 721),
    ];

    println!("\n--- Displays ---");
    for (name, file, w, h) in &displays {
        println!("  {} ({}x{})...", name, w, h);
        let filter = DeviceFrameFilter::new().with_device_name(*name);
        let img = apply_filter_to_image(&filter, *w, *h, fps)?;
        save_image(&img, &format!("{}/{}.png", tmp_dir, file))?;
    }

    println!("\n========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nSupported devices: 5 phones, 5 tablets, 5 computers, 5 displays");

    Ok(())
}
