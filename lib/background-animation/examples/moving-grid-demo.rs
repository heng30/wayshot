use background_animation::{Animation, AnimationRecordConfig, moving_grid::MovingGridConfig};

fn main() {
    env_logger::init();

    let output_path = "output/moving_grid.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut grid = MovingGridConfig::new()
        .with_rows(10)
        .with_cols(24)
        .with_speed(60.0)
        .with_line_color((80, 80, 80, 255))
        .with_line_width(1.0)
        .with_bg_color((5, 5, 15))
        .with_supersample(2);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(5),
        output_path,
    );

    log::info!("Generating moving grid animation to {}", output_path);
    grid.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}