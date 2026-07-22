use background_animation::{Animation, AnimationRecordConfig, particle_life::ParticleLifeConfig};

fn main() {
    env_logger::init();

    let output_path = "output/particle_life_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut particle_life = ParticleLifeConfig::new()
        .with_particle_count(5000)
        .with_type_count(6)
        .with_rmax(0.04)
        .with_friction(0.85)
        .with_force(1.0)
        .with_dt(0.05)
        .with_wrap(true)
        .with_particle_size(4.0)
        .with_colors(vec![
            (255, 100, 100, 255),  // Red
            (100, 255, 100, 255),  // Green
            (100, 100, 255, 255),  // Blue
            (255, 255, 100, 255),  // Yellow
            (255, 100, 255, 255),  // Magenta
            (100, 255, 255, 255),  // Cyan
        ])
        .with_bg_color((10, 15, 25))
        .with_matrix_seed(42);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(10),
        output_path,
    );

    log::info!("Generating particle life animation to {}", output_path);
    particle_life.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}