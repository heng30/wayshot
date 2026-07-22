use background_animation::{Animation, AnimationRecordConfig, boids::BoidsConfig};

fn main() {
    env_logger::init();

    let output_path = "output/boids_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut boids = BoidsConfig::new()
        .with_boid_count(150)
        .with_max_speed(5.0)
        .with_visual_range(60.0)
        .with_separation_factor(0.05)
        .with_alignment_factor(0.05)
        .with_cohesion_factor(0.008)
        .with_min_separation(25.0)
        .with_turn_factor(0.4)
        .with_boid_size(10.0)
        .with_colors(vec![
            (255, 200, 100),
            (100, 200, 255),
            (255, 255, 200),
            (200, 255, 200),
            (255, 150, 150),
        ])
        .with_trails_enabled(true)
        .with_trail_length(8)
        .with_bg_color((10, 15, 25));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating boids animation to {}", output_path);
    boids.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}