// Example demonstrating the Live2D filter
// Renders a Live2D model onto a test image and saves the result

use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    filters::{
        traits::{VideoData, VideoFilter, VideoFilterConfig},
        video::Live2dFilter,
    },
    metadata::Metadata,
    tracks::{segment::Segment, video_frame_cache::VideoImage},
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([30, 30, 60, 255]));

    // Sky gradient
    for y in 0..height / 2 {
        let t = y as f32 / (height / 2) as f32;
        let r = (100.0 + 155.0 * t) as u8;
        let g = (150.0 + 105.0 * t) as u8;
        for x in 0..width {
            img.put_pixel(x, y, Rgba([r, g, 255, 255]));
        }
    }

    // Ground gradient
    for y in height / 2..height {
        let t = (y - height / 2) as f32 / (height / 2) as f32;
        let r = (80.0 - 30.0 * t) as u8;
        let g = (160.0 - 60.0 * t) as u8;
        let b = (80.0 - 30.0 * t) as u8;
        for x in 0..width {
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
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
    filter: &Live2dFilter,
    width: u32,
    height: u32,
    fps: f32,
    time_offset: Duration,
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height);

    let mut video_data = VideoData {
        config: VideoFilterConfig::new(width, height, fps),
        frames: vec![VideoImage::Image { buffer }],
        from_segment: create_dummy_segment(),
        relative_timeline_offset: time_offset,
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
    println!("Live2D Filter Demo");
    println!("==================\n");

    let tmp_dir = "tmp/live2d_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (512, 512);
    let fps = 30.0;

    // Save original image for comparison
    println!("Saving original test image...");
    let original = create_test_image(width, height);
    save_image(&original, &format!("{}/original.png", tmp_dir))?;
    println!("  Saved: original.png");

    // Find a model directory from lib/live2d-rs/models/
    let model_dirs = [
        "lib/live2d-rs/models/Haru",
        "../lib/live2d-rs/models/Haru",
        "../../lib/live2d-rs/models/Haru",
    ];

    let model_path = model_dirs
        .iter()
        .find_map(|dir| {
            let path = PathBuf::from(dir);
            if path.is_dir() {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            println!("\nNo built-in model found. To use a custom model, pass the model directory as argument:");
            println!("  cargo run --example live2d_filter_demo -- /path/to/model_directory\n");
            std::process::exit(0);
        });

    // Override with CLI argument if provided
    let args: Vec<String> = std::env::args().collect();
    let final_model_path = if args.len() > 1 {
        let p = &args[1];
        if PathBuf::from(p).exists() {
            println!("Using custom model: {}", p);
            p.clone()
        } else {
            println!("Warning: custom model not found: {}, using default", p);
            model_path
        }
    } else {
        println!("Using model: {}", model_path);
        model_path
    };

    // 1. Static pose (no motion, no expression)
    println!("\n1. Static pose...");
    let filter = Live2dFilter {
        model_dir: final_model_path.clone(),
        ..Live2dFilter::new()
    };
    let img = apply_filter_to_image(&filter, width, height, fps, Duration::ZERO)?;
    save_image(&img, &format!("{}/static.png", tmp_dir))?;
    println!("  Saved: static.png");

    // 2. Different fill values
    for fill in [1.0, 1.85, 3.0] {
        println!("2. Fill = {}...", fill);
        let filter = Live2dFilter {
            model_dir: final_model_path.clone(),
            model_view_fill: fill,
            ..Live2dFilter::new()
        };
        let img = apply_filter_to_image(&filter, width, height, fps, Duration::ZERO)?;
        save_image(&img, &format!("{}/fill_{}.png", tmp_dir, fill))?;
        println!("  Saved: fill_{}.png", fill);
    }

    // 3. With custom background color
    println!("\n3. With background color...");
    let filter = Live2dFilter {
        model_dir: final_model_path.clone(),
        background: [80, 120, 200, 255],
        ..Live2dFilter::new()
    };
    let img = apply_filter_to_image(&filter, width, height, fps, Duration::ZERO)?;
    save_image(&img, &format!("{}/bg_color.png", tmp_dir))?;
    println!("  Saved: bg_color.png");

    // 4. Different resolutions
    for (w, h) in [(256, 256), (1024, 1024)] {
        println!("4. Resolution {}x{}...", w, h);
        let filter = Live2dFilter {
            model_dir: final_model_path.clone(),
            ..Live2dFilter::new()
        };
        let img = apply_filter_to_image(&filter, w, h, fps, Duration::ZERO)?;
        save_image(&img, &format!("{}/{}x{}.png", tmp_dir, w, h))?;
        println!("  Saved: {}x{}.png", w, h);
    }

    println!("\n==================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);

    Ok(())
}
