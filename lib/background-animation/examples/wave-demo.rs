use background_animation::{Animation, AnimationRecordConfig, wave::WaveConfig};

fn main() {
    env_logger::init();

    let output_path = "output/wave_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create wave animation matching the CSS wave.js reference:
    // - 3 wave layers with different speeds/directions
    // - Animated diagonal gradient background
    // - Semi-transparent white waves
    let mut wave = WaveConfig::new()
        .with_wave_count(3)
        .with_wave_height(200.0)
        .with_duration(10.0)
        .with_wave_color((255, 255, 255))
        .with_wave_opacity(0.25)
        .with_wave_opacities(vec![0.8, 0.8, 0.9])
        .with_gradient_colors(vec![
            (101, 0, 94),   // Purple 3%
            (60, 132, 206), // Blue 38%
            (48, 238, 226), // Cyan 68%
            (255, 25, 25),  // Red 98%
        ])
        .with_gradient_stops(vec![0.03, 0.38, 0.68, 0.98])
        .with_gradient_duration(15.0);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(10),
        output_path,
    );

    log::info!("Generating wave animation to {}", output_path);
    wave.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
