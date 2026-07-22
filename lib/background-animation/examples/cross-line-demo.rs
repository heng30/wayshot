use background_animation::{Animation, AnimationRecordConfig, cross_line::CrossLineConfig};

fn main() {
    env_logger::init();

    let output_path = "output/cross_line_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut cross_line = CrossLineConfig::new()
        .with_lines_num(16)
        .with_speed_min(0.25)
        .with_speed_max(1.0)
        .with_line_color((200, 200, 200, 255))
        .with_point_color((255, 255, 255, 255))
        .with_bg_color((0, 0, 0));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating cross-line animation to {}", output_path);
    cross_line.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
