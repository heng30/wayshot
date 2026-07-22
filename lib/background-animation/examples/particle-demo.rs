use background_animation::{Animation, AnimationRecordConfig, particle::ParticleConfig};

fn main() {
    env_logger::init();

    let output_path = "output/particle_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut particle = ParticleConfig::new()
        .with_density(5)
        .with_line_color((100, 180, 255, 120))
        .with_particle_color((100, 180, 255, 80))
        .with_bg_color((10, 15, 25))
        .with_pointer_enabled(true)
        .with_pointer_count(3);

    let config = AnimationRecordConfig::new(
        1920,
        1080,
        25,
        std::time::Duration::from_secs(10),
        output_path,
    );

    log::info!("Generating particle network animation to {}", output_path);
    particle.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}

