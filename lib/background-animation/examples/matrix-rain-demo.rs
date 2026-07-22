use background_animation::{Animation, AnimationRecordConfig, matrix_rain::MatrixRainConfig};

fn main() {
    env_logger::init();

    let output_path = "output/matrix_rain_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut matrix_rain = MatrixRainConfig::new()
        .with_cell_size(16)
        .with_columns(80)
        .with_min_speed(0.3)       // Slower speed
        .with_max_speed(0.8)       // Slower speed
        .with_trail_length(25)     // Longer trail
        .with_fade_speed(0.03)     // Slower fade
        .with_color((0, 255, 70))
        .with_bg_color((0, 0, 0))
        .with_glow_intensity(2.0)
        .with_char_change_prob(0.15)  // More character changes
        .with_flicker_prob(0.08)      // More flickering
        .with_particle_density(10);   // More particles

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(10), output_path);

    log::info!("Generating matrix rain animation to {}", output_path);
    matrix_rain.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}