use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    filters::{
        traits::{VideoData, VideoFilter, VideoFilterConfig},
        video::ShadowFilter,
    },
    metadata::Metadata,
    tracks::{segment::Segment, video_frame_cache::VideoImage},
    Result,
};
use image::{Rgba, RgbaImage};

fn create_test_image_with_content(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 80, 80, 255]));

    let cx = width / 2;
    let cy = height / 2;
    let rect_w = width / 3;
    let rect_h = height / 3;
    let radius = 20u32;

    // Draw a rounded rectangle with solid color and transparent background
    for y in 0..height {
        for x in 0..width {
            let in_rect_x = x >= cx - rect_w / 2 && x < cx + rect_w / 2;
            let in_rect_y = y >= cy - rect_h / 2 && y < cy + rect_h / 2;

            if in_rect_x && in_rect_y {
                // Check corner radius
                let near_left = x < cx - rect_w / 2 + radius;
                let near_right = x >= cx + rect_w / 2 - radius;
                let near_top = y < cy - rect_h / 2 + radius;
                let near_bottom = y >= cy + rect_h / 2 - radius;

                let in_corner = (near_left || near_right) && (near_top || near_bottom);

                if in_corner {
                    let corner_cx = if near_left {
                        cx - rect_w / 2 + radius
                    } else {
                        cx + rect_w / 2 - radius
                    };
                    let corner_cy = if near_top {
                        cy - rect_h / 2 + radius
                    } else {
                        cy + rect_h / 2 - radius
                    };
                    let dx = x as i32 - corner_cx as i32;
                    let dy = y as i32 - corner_cy as i32;
                    if (dx * dx + dy * dy) as f32 <= (radius * radius) as f32 {
                        img.put_pixel(x, y, Rgba([255, 150, 100, 255]));
                    } else {
                        img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                    }
                } else {
                    img.put_pixel(x, y, Rgba([255, 150, 100, 255]));
                }
            } else {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
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
    Arc::new(Segment::new(Duration::ZERO, Duration::from_secs(10), metadata, 1.0))
}

fn apply_filter_to_image(
    filter: &ShadowFilter,
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
    println!("Shadow Filter Demo");
    println!("==================\n");

    let tmp_dir = "tmp/shadow_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Example 1: Default shadow
    println!("Example 1: Default shadow (black, opacity=0.8, blur=10, angle=135, distance=10)");
    let filter1 = ShadowFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/shadow_default.png", tmp_dir))?;
    println!("  Saved: shadow_default.png");

    // Example 2: Soft shadow (large blur, low opacity)
    println!("\nExample 2: Soft shadow (opacity=0.5, blur=30, distance=20)");
    let filter2 = ShadowFilter::new([0, 0, 0, 255], 0.5, 30.0, 135.0, 20.0);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/shadow_soft.png", tmp_dir))?;
    println!("  Saved: shadow_soft.png");

    // Example 3: Colored shadow
    println!("\nExample 3: Blue shadow (color=[50,100,255,255], blur=15)");
    let filter3 = ShadowFilter::new([50, 100, 255, 255], 0.7, 15.0, 135.0, 12.0);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/shadow_colored.png", tmp_dir))?;
    println!("  Saved: shadow_colored.png");

    // Example 4: Red shadow at different angle
    println!("\nExample 4: Red shadow (angle=45, distance=15)");
    let filter4 = ShadowFilter::new([255, 50, 50, 255], 0.6, 12.0, 45.0, 15.0);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/shadow_red_angle.png", tmp_dir))?;
    println!("  Saved: shadow_red_angle.png");

    // Example 5: Large expanded shadow
    println!("\nExample 5: Large expanded shadow (size=0.5, blur=25, distance=8)");
    let filter5 = ShadowFilter::new([0, 0, 0, 255], 0.7, 25.0, 135.0, 8.0).with_size(0.5);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/shadow_large.png", tmp_dir))?;
    println!("  Saved: shadow_large.png");

    // Example 6: Distance comparison
    println!("\nExample 6: Distance comparison (5 to 30)");
    for distance in [5.0, 10.0, 15.0, 20.0, 30.0] {
        let filter = ShadowFilter::new([0, 0, 0, 255], 0.7, 10.0, 135.0, distance);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/shadow_dist_{:.0}.png", tmp_dir, distance))?;
    }
    println!("  Saved: shadow_dist_5.png to shadow_dist_30.png");

    // Example 7: Angle comparison
    println!("\nExample 7: Angle comparison (0 to 315, step 45)");
    for angle in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
        let filter = ShadowFilter::new([0, 0, 0, 255], 0.7, 10.0, angle, 15.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/shadow_angle_{:.0}.png", tmp_dir, angle))?;
    }
    println!("  Saved: shadow_angle_0.png to shadow_angle_315.png");

    // Example 8: Blur comparison
    println!("\nExample 8: Blur comparison (0 to 40)");
    for blur in [0.0, 5.0, 10.0, 20.0, 40.0] {
        let filter = ShadowFilter::new([0, 0, 0, 255], 0.8, blur, 135.0, 10.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/shadow_blur_{:.0}.png", tmp_dir, blur))?;
    }
    println!("  Saved: shadow_blur_0.png to shadow_blur_40.png");

    // Example 9: Size comparison
    println!("\nExample 9: Size comparison (0.0 to 0.5)");
    for size in [0.0, 0.05, 0.1, 0.2, 0.5] {
        let filter = ShadowFilter::new([0, 0, 0, 255], 0.8, 10.0, 135.0, 10.0).with_size(size);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/shadow_size_{:.2}.png", tmp_dir, size))?;
    }
    println!("  Saved: shadow_size_0.00.png to shadow_size_0.50.png");

    // Example 10: Opacity comparison
    println!("\nExample 9: Opacity comparison (0.2 to 1.0)");
    for opacity in [0.2, 0.4, 0.6, 0.8, 1.0] {
        let filter = ShadowFilter::new([0, 0, 0, 255], opacity, 12.0, 135.0, 12.0);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/shadow_opacity_{:.1}.png", tmp_dir, opacity))?;
    }
    println!("  Saved: shadow_opacity_0.2.png to shadow_opacity_1.0.png");

    println!("\n==================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - color: Shadow color (RGBA)");
    println!("  - opacity: Shadow transparency (0.0 = invisible, 1.0 = fully opaque)");
    println!("  - size: Shadow spread/scale ratio (0.0-1.0, 0 = same size, 1 = 2x size)");
    println!("  - blur: Gaussian blur radius in pixels (0.0-100.0)");
    println!("  - angle: Shadow offset direction in degrees (0-360)");
    println!("  - distance: Shadow offset distance in pixels (0.0-200.0)");

    Ok(())
}
