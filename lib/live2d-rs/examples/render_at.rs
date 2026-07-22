//! Demonstrates the idempotent `render_at(fps, current_time)` API.
//!
//! Usage:
//!   cargo run --example render_at -- <model_path>
//!   cargo run --example render_at -- models/Haru/Haru.model3.json

use std::path::PathBuf;

use live2d_rs::Live2dRenderer;

fn main() {
    let model_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "models/Haru/Haru.model3.json".to_string());
    let model_path = PathBuf::from(&model_path);

    if !model_path.exists() {
        eprintln!("Model file not found: {}", model_path.display());
        std::process::exit(1);
    }

    // Create renderer at 256×256.
    let mut renderer = Live2dRenderer::new(&model_path, 256, 256)
        .unwrap_or_else(|e| {
            eprintln!("Failed to load model: {e}");
            std::process::exit(1);
        });

    // Play the first motion if available.
    if let Some(path) = renderer.motion_paths().first().cloned() {
        renderer.play_motion(&path).unwrap();
        eprintln!("Playing motion: {}", path.display());
    }

    let fps = 30.0;
    let duration = 2.0; // seconds
    let total_frames = (duration * fps) as usize;

    // --- Demo 1: render_at is idempotent ---
    // Calling render_at with the same (fps, time) always returns the same bytes,
    // regardless of what was rendered before.

    let frame_0_first = renderer.render_at(fps, 0.0);
    let _frame_5 = renderer.render_at(fps, 5.0 / fps);
    let frame_0_again = renderer.render_at(fps, 0.0);

    assert_eq!(frame_0_first, frame_0_again, "render_at must be idempotent");
    eprintln!("✓ render_at(30, 0.0) is idempotent: {} bytes, identical after rendering other frames", frame_0_first.len());

    // --- Demo 2: frame boundary snapping ---
    // At 30fps, t=0.03 and t=0.01 both snap to frame 0.

    let f_0_00 = renderer.render_at(fps, 0.0);
    let f_0_01 = renderer.render_at(fps, 0.01);
    let f_0_03 = renderer.render_at(fps, 0.03);

    assert_eq!(f_0_00, f_0_01, "t=0.01 should snap to frame 0");
    assert_eq!(f_0_00, f_0_03, "t=0.03 should snap to frame 0");
    eprintln!("✓ Frame boundary snapping works: t=0.0, 0.01, 0.03 all produce frame 0");

    // --- Demo 3: render a full sequence and save to PNG ---
    let output_dir = PathBuf::from("output_render_at");
    std::fs::create_dir_all(&output_dir).unwrap();

    for i in 0..total_frames {
        let time_s = i as f32 / fps;
        let rgba = renderer.render_at(fps, time_s);
        let path = output_dir.join(format!("frame_{:04}.png", i));
        let img = image::RgbaImage::from_raw(256, 256, rgba).expect("invalid frame buffer");
        img.save(&path).unwrap();
    }
    eprintln!("✓ Saved {} frames to {}", total_frames, output_dir.display());

    // --- Demo 4: out-of-order rendering ---
    // You can render frames in any order — each call is independent.

    let _frame_last = renderer.render_at(fps, (total_frames - 1) as f32 / fps);
    let frame_first = renderer.render_at(fps, 0.0);

    assert_eq!(frame_0_first, frame_first, "out-of-order render_at(0.0) matches earlier render_at(0.0)");
    eprintln!("✓ Out-of-order rendering works: frame 0 after frame {} is still correct", total_frames - 1);

    eprintln!("\nAll demos passed! render_at is fully idempotent.");
}
