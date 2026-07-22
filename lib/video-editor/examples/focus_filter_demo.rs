// Example demonstrating the focus (bokeh) filter
// Simulates camera aperture depth-of-field effect with bokeh blur.
// Generates test images and saves them to tmp/ directory

use std::{fs, path::PathBuf, time::Duration};
use video_editor::{
    filters::{
        keyframe::{Keyframe, KeyframeTracks, KeyframeValue, PropertyTrack},
        traits::{VideoData, VideoFilter, VideoFilterConfig},
        video::FocusFilter,
    },
    metadata::Metadata,
    tracks::video_frame_cache::VideoImage,
    Result,
};
use image::{Rgba, RgbaImage};

/// Create a test image with diverse visual content:
/// - Bright colored circles (to show bokeh highlights)
/// - A grid of shapes (to show blur detail)
/// - Dark and bright areas (to show highlight boost effect)
fn create_test_image(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::from_fn(width, height, |_, _| Rgba([40, 45, 55, 255]));

    // Draw colored circles scattered across the image to demonstrate bokeh highlights
    let circles = [
        (0.15, 0.20, 30.0, [255, 200, 80, 255]),   // Warm yellow
        (0.80, 0.15, 25.0, [255, 100, 100, 255]),   // Red
        (0.70, 0.75, 35.0, [100, 200, 255, 255]),   // Light blue
        (0.25, 0.80, 20.0, [150, 255, 150, 255]),   // Green
        (0.50, 0.50, 40.0, [255, 180, 120, 255]),   // Orange (center subject)
        (0.90, 0.50, 22.0, [200, 150, 255, 255]),   // Purple
        (0.10, 0.50, 18.0, [255, 255, 150, 255]),   // Light yellow
        (0.50, 0.10, 28.0, [120, 255, 255, 255]),   // Cyan
        (0.50, 0.90, 26.0, [255, 150, 200, 255]),   // Pink
    ];

    for (cx, cy, r, color) in &circles {
        let px = (*cx * width as f32) as i32;
        let py = (*cy * height as f32) as i32;
        let ri = *r as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if dx * dx + dy * dy <= (ri * ri) {
                    let x = (px + dx).clamp(0, width as i32 - 1) as u32;
                    let y = (py + dy).clamp(0, height as i32 - 1) as u32;
                    img.put_pixel(x, y, Rgba(*color));
                }
            }
        }
    }

    // Draw a grid of small dots to show blur detail
    let grid_spacing = 40;
    for gy in 0..(height / grid_spacing) {
        for gx in 0..(width / grid_spacing) {
            let x = gx * grid_spacing + grid_spacing / 2;
            let y = gy * grid_spacing + grid_spacing / 2;
            if x < width && y < height {
                // Small bright dot
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let px = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                        let py = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                        img.put_pixel(px, py, Rgba([200, 210, 220, 255]));
                    }
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
    filter: &FocusFilter,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<RgbaImage> {
    let buffer = create_test_image(width, height);

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
    println!("Focus (Bokeh) Filter Demo");
    println!("==========================\n");

    // Create tmp directory
    let tmp_dir = "tmp/focus_demo";
    fs::create_dir_all(tmp_dir)?;

    let (width, height) = (640, 480);
    let fps = 30.0;

    // Also save the original test image for comparison
    let original = create_test_image(width, height);
    save_image(&original, &format!("{}/focus_original.png", tmp_dir))?;
    println!("Saved original test image: focus_original.png\n");

    // Example 1: Default — center focus with moderate bokeh
    println!("Example 1: Default focus (center, moderate blur)");
    let filter1 = FocusFilter::default();
    let img1 = apply_filter_to_image(&filter1, width, height, fps)?;
    save_image(&img1, &format!("{}/focus_default.png", tmp_dir))?;
    println!("  Saved: focus_default.png");

    // Example 2: Portrait depth-of-field — large focus area, soft bokeh
    println!("\nExample 2: Portrait DOF (center subject, soft bokeh)");
    let filter2 = FocusFilter::new(0.5, 0.5, 150, 20)
        .with_feather(80)
        .with_aperture_blades(8)
        .with_highlight_boost(1.2);
    let img2 = apply_filter_to_image(&filter2, width, height, fps)?;
    save_image(&img2, &format!("{}/focus_portrait.png", tmp_dir))?;
    println!("  Saved: focus_portrait.png");

    // Example 3: Tilt-shift miniature — small focus, strong blur
    println!("\nExample 3: Tilt-shift miniature (small focus, strong blur)");
    let filter3 = FocusFilter::new(0.5, 0.5, 80, 35)
        .with_feather(40)
        .with_aperture_blades(6)
        .with_highlight_boost(1.5);
    let img3 = apply_filter_to_image(&filter3, width, height, fps)?;
    save_image(&img3, &format!("{}/focus_tilt_shift.png", tmp_dir))?;
    println!("  Saved: focus_tilt_shift.png");

    // Example 4: Hexagonal bokeh — 6 aperture blades
    println!("\nExample 4: Hexagonal bokeh (6 blades, cinematic look)");
    let filter4 = FocusFilter::new(0.5, 0.5, 120, 25)
        .with_feather(60)
        .with_aperture_blades(6)
        .with_highlight_boost(1.8);
    let img4 = apply_filter_to_image(&filter4, width, height, fps)?;
    save_image(&img4, &format!("{}/focus_hexagonal_bokeh.png", tmp_dir))?;
    println!("  Saved: focus_hexagonal_bokeh.png");

    // Example 5: Pentagonal bokeh — 5 aperture blades (classic lens look)
    println!("\nExample 5: Pentagonal bokeh (5 blades, classic lens)");
    let filter5 = FocusFilter::new(0.5, 0.5, 100, 20)
        .with_feather(50)
        .with_aperture_blades(5)
        .with_highlight_boost(1.6);
    let img5 = apply_filter_to_image(&filter5, width, height, fps)?;
    save_image(&img5, &format!("{}/focus_pentagonal_bokeh.png", tmp_dir))?;
    println!("  Saved: focus_pentagonal_bokeh.png");

    // Example 6: Off-center focus — subject at 1/3 position
    println!("\nExample 6: Off-center focus (subject at left third)");
    let filter6 = FocusFilter::new(0.3, 0.5, 120, 22)
        .with_feather(70)
        .with_aperture_blades(8)
        .with_highlight_boost(1.3);
    let img6 = apply_filter_to_image(&filter6, width, height, fps)?;
    save_image(&img6, &format!("{}/focus_off_center.png", tmp_dir))?;
    println!("  Saved: focus_off_center.png");

    // Example 7: Hard edge — no feather, sharp focus boundary
    println!("\nExample 7: Hard edge (feather=0, sharp focus boundary)");
    let filter7 = FocusFilter::new(0.5, 0.5, 120, 20).with_feather(0);
    let img7 = apply_filter_to_image(&filter7, width, height, fps)?;
    save_image(&img7, &format!("{}/focus_hard_edge.png", tmp_dir))?;
    println!("  Saved: focus_hard_edge.png");

    // Example 8: Strong highlight boost — exaggerated bokeh balls
    println!("\nExample 8: Strong highlight boost (boost=2.0, exaggerated bokeh balls)");
    let filter8 = FocusFilter::new(0.5, 0.5, 100, 25)
        .with_feather(60)
        .with_aperture_blades(8)
        .with_highlight_boost(2.0);
    let img8 = apply_filter_to_image(&filter8, width, height, fps)?;
    save_image(&img8, &format!("{}/focus_highlight_boost.png", tmp_dir))?;
    println!("  Saved: focus_highlight_boost.png");

    // Example 9: Aperture blade comparison — same settings, different blade counts
    println!("\nExample 9: Aperture blade comparison (3, 5, 6, 8, 12 blades)");
    for blades in [3, 5, 6, 8, 12] {
        let filter = FocusFilter::new(0.5, 0.5, 100, 20)
            .with_feather(50)
            .with_aperture_blades(blades)
            .with_highlight_boost(1.5);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/focus_blades_{}.png", tmp_dir, blades))?;
    }
    println!("  Saved: focus_blades_3.png to focus_blades_12.png");

    // Example 10: Blur radius comparison — different blur strengths
    println!("\nExample 10: Blur radius comparison (5, 10, 20, 35, 50)");
    for blur in [5, 10, 20, 35, 50] {
        let filter = FocusFilter::new(0.5, 0.5, 120, blur)
            .with_feather(60)
            .with_aperture_blades(8)
            .with_highlight_boost(1.3);
        let img = apply_filter_to_image(&filter, width, height, fps)?;
        save_image(&img, &format!("{}/focus_blur_{}.png", tmp_dir, blur))?;
    }
    println!("  Saved: focus_blur_5.png to focus_blur_50.png");

    // Example 11: Rack focus with keyframes — animate focus from left to right
    // This is the most cinematic use case: the camera "pulls focus" between subjects
    println!("\nExample 11: Rack focus keyframes (focus moves left → center → right)");
    let mut filter11 = FocusFilter::new(0.3, 0.5, 120, 22)
        .with_feather(60)
        .with_aperture_blades(8)
        .with_highlight_boost(1.3);

    let mut tracks = KeyframeTracks::default();
    // center_x: left(0.3) → center(0.5) → right(0.7)
    tracks.tracks.push(PropertyTrack::with_keyframes(
        "center_x",
        vec![
            Keyframe::new(0, KeyframeValue::Float(0.3)),
            Keyframe::new(1500, KeyframeValue::Float(0.5)),
            Keyframe::new(3000, KeyframeValue::Float(0.7)),
        ],
    ));
    // center_y: stay centered
    tracks.tracks.push(PropertyTrack::with_keyframes(
        "center_y",
        vec![
            Keyframe::new(0, KeyframeValue::Float(0.5)),
            Keyframe::new(3000, KeyframeValue::Float(0.5)),
        ],
    ));
    // focus_radius: narrow → wide (breathing DOF)
    tracks.tracks.push(PropertyTrack::with_keyframes(
        "focus_radius",
        vec![
            Keyframe::new(0, KeyframeValue::Float(80.0)),
            Keyframe::new(1500, KeyframeValue::Float(120.0)),
            Keyframe::new(3000, KeyframeValue::Float(160.0)),
        ],
    ));
    filter11.set_keyframe_tracks(tracks);

    // Render frames at different times to show the rack focus animation
    for (time_ms, label) in [(0, "start"), (750, "q1"), (1500, "mid"), (2250, "q3"), (3000, "end")]
    {
        let buffer = create_test_image(width, height);
        let mut video_data = VideoData {
            config: VideoFilterConfig::new(width, height, fps),
            frames: vec![VideoImage::Image { buffer }],
            from_segment: create_dummy_segment(),
            relative_timeline_offset: Duration::from_millis(time_ms as u64),
        };
        filter11.apply(&mut video_data)?;
        if let Some(VideoImage::Image { buffer, .. }) = video_data.frames.first() {
            save_image(buffer, &format!("{}/focus_rack_{}.png", tmp_dir, label))?;
        }
    }
    println!("  Saved: focus_rack_start.png, q1.png, mid.png, q3.png, end.png");

    println!("\n==========================");
    println!("All examples generated successfully!");
    println!("\nImages saved to: {}", tmp_dir);
    println!("\nParameter explanations:");
    println!("  - center_x/y: Focus region center, normalized (0.0-1.0)");
    println!("  - focus_radius: Sharp focus region radius in pixels (at 1080p)");
    println!("  - feather: Transition zone width from sharp to blurred");
    println!("  - blur_radius: Bokeh blur strength in pixels (at 1080p)");
    println!("  - aperture_blades: Number of aperture blades (3-12)");
    println!("    → Low (5-6): visible polygonal bokeh highlights");
    println!("    → High (8+): nearly circular bokeh highlights");
    println!("  - highlight_boost: Enhances bright out-of-focus areas (0.0-2.0)");
    println!("    → 1.0: no boost");
    println!("    → >1.0: brighter bokeh 'balls'");

    Ok(())
}
