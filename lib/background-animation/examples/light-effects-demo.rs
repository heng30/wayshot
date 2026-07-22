use background_animation::{Animation, AnimationRecordConfig, light_effects::LightEffectsConfig};

fn main() {
    env_logger::init();

    let output_path = "output/light_effects_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut light_effects = LightEffectsConfig::new()
        .with_flare_count(8)
        .with_min_size(80.0)
        .with_max_size(300.0)
        .with_movement_speed(0.2)
        .with_colors(vec![
            (255, 200, 100, 100),
            (255, 150, 50, 80),
            (255, 100, 50, 60),
            (200, 255, 150, 90),
            (150, 200, 255, 70),
        ])
        .with_elliptical_enabled(true)
        .with_bands_enabled(true)
        .with_bg_color((15, 15, 25));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating light effects animation to {}", output_path);
    light_effects.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}