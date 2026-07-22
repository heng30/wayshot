use background_animation::{Animation, AnimationRecordConfig, bokeh::BokehConfig};

fn main() {
    env_logger::init();

    let output_path = "output/bokeh_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut bokeh = BokehConfig::new()
        .with_spot_count(40)
        .with_min_size(15.0)
        .with_max_size(120.0)
        .with_animation_speed(0.08)
        .with_colors(vec![
            (255, 220, 180, 180),
            (255, 200, 100, 150),
            (255, 180, 80, 120),
            (200, 255, 220, 100),
            (255, 150, 150, 110),
            (150, 200, 255, 90),
        ])
        .with_hexagonal_enabled(true)
        .with_bg_color((5, 10, 20));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating bokeh animation to {}", output_path);
    bokeh.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}