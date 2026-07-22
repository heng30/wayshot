use background_animation::{Animation, AnimationRecordConfig, black_hole::BlackHoleConfig};

fn main() {
    env_logger::init();

    let output_path = "output/black_hole_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create black hole animation matching the JS black-hole.js reference:
    // - 200 star particles drawn toward the center gravity point
    // - Stars accelerate and fade as they reach the event horizon
    // - Dark black hole core with radial gradient
    // - Hue rotation creates evolving color trails
    let mut black_hole = BlackHoleConfig::new()
        .with_star_count(200)
        .with_black_hole_size(40.0)
        .with_event_horizon_offset(16.0)
        .with_max_consume_frames(50)
        .with_hue_speed(36.0)
        .with_trail_alpha(0.3)
        .with_center_x(0.5)
        .with_center_y(0.5);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(10),
        output_path,
    );

    log::info!("Generating black hole animation to {}", output_path);
    black_hole
        .animate_record(config)
        .expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
