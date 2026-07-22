use background_animation::{Animation, AnimationRecordConfig, chromatic::ChromaticConfig};

fn main() {
    env_logger::init();

    let output_path = "output/chromatic_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut chromatic = ChromaticConfig::new()
        .with_offset_x(5.0)
        .with_intensity(1.2)
        .with_radial_enabled(true)
        .with_animation_speed(0.8)
        .with_bg_color((10, 10, 15))
        .with_fg_color((255, 255, 255));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating chromatic aberration animation to {}", output_path);
    chromatic.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}