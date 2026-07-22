use background_animation::{Animation, AnimationRecordConfig, ink::{InkDissipationConfig, InkStyle}};

fn main() {
    env_logger::init();

    let output_path = "output/ink_dissipation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Simplified ink dissipation config
    let mut ink = InkDissipationConfig::new()
        .with_style(InkStyle::InkOnPaper)      // Traditional: white bg + black ink
        .with_source_count(15)                  // Number of ink sources
        .with_spawn_rate(0.05)                  // Drop spawn probability
        .with_source_lifetime(150)              // Source lifetime in frames
        .with_initial_radius(5.0)               // Initial drop size
        .with_max_radius(80.0)                  // Maximum diffusion radius
        .with_spread_rate(2.5)                  // How fast drops spread
        .with_fade_speed(0.02)                  // How fast ink fades (0.001-0.1)
        .with_diffusion_strength(0.02)          // Field diffusion blend
        .with_max_drops(300)                    // Maximum active drops
        .with_resolution_divisor(3);            // Higher resolution

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(6),
        output_path
    );

    log::info!("Generating ink drop diffusion animation to {}", output_path);
    ink.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}