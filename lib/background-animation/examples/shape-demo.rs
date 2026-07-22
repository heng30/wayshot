use background_animation::{Animation, AnimationRecordConfig, shape::ShapeConfig};

fn main() {
    env_logger::init();

    let output_path = "output/shape_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut shape = ShapeConfig::new()
        .with_max_circles(12)
        .with_bg_color((38, 43, 46));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating shape animation to {}", output_path);
    shape.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
