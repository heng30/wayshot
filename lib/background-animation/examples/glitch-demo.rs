use background_animation::{Animation, AnimationRecordConfig, glitch::GlitchConfig};

fn main() {
    env_logger::init();

    let output_path = "output/glitch_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create a subtle glitch effect suitable for cyberpunk/tech backgrounds
    let mut glitch = GlitchConfig::new()
        .with_intensity(0.5) // Overall intensity - subtle but visible
        .with_scan_lines_enabled(true)
        .with_scan_line_spacing(3) // Scan line density
        .with_rgb_split_enabled(true)
        .with_rgb_split_offset(3) // RGB channel separation distance
        .with_block_shift_enabled(true)
        .with_block_shift_max_offset(10) // Max horizontal strip shift
        .with_noise_enabled(true)
        .with_animation_speed(0.6) // How fast glitch effects vary
        .with_bg_color((12, 18, 32)); // Dark blue-gray background

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating glitch animation to {}", output_path);
    glitch.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}

