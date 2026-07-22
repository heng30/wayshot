use background_animation::{Animation, AnimationRecordConfig, kaleidoscope::KaleidoscopeConfig};

fn main() {
    env_logger::init();

    let output_path = "output/kaleidoscope_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut kaleidoscope = KaleidoscopeConfig::new()
        .with_segments(12)
        .with_rotation_speed(0.3)
        .with_center((0.5, 0.5))
        .with_scale(1.5)
        .with_complexity(8)
        .with_colors(vec![
            (255, 50, 100),
            (255, 150, 50),
            (50, 255, 150),
            (50, 150, 255),
            (200, 50, 255),
        ])
        .with_bg_color((10, 10, 20));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating kaleidoscope animation to {}", output_path);
    kaleidoscope.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}