use background_animation::{Animation, AnimationRecordConfig, galaxy::GalaxyConfig};

fn main() {
    env_logger::init();

    let output_path = "output/galaxy_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create galaxy animation matching the CSS galaxy reference:
    // - 200 stars distributed on a 3D sphere
    // - Each star has a mirrored counterpart on the opposite side
    // - The whole galaxy rotates around the (1,1,1) axis
    // - Appear animation scales from 0 to 1 over 10 seconds
    // - Stars have colored glow effects
    let mut galaxy = GalaxyConfig::new()
        .with_star_count(200)
        .with_rotation_period(10.0)
        .with_appear_duration(10.0)
        .with_perspective(100.0)
        .with_glow_intensity(1.0);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(20),
        output_path,
    );

    log::info!("Generating galaxy animation to {}", output_path);
    galaxy
        .animate_record(config)
        .expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
