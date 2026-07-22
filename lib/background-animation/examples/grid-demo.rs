use background_animation::{Animation, AnimationRecordConfig, grid::GridConfig};

fn main() {
    env_logger::init();

    let output_path = "output/grid_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut grid = GridConfig::new()
        .with_rows(18)
        .with_cols(32)
        .with_amplitude(10.0)
        .with_node_amplitude(5.0)
        .with_frequency(0.1)
        .with_node_radius(4)
        .with_line_color((255, 255, 255, 255))
        .with_bg_color((0, 0, 0))
        .with_node_color((255, 255, 255, 255))
        .with_line_width(1.0)
        .with_segments_per_edge(20);

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating grid animation to {}", output_path);
    grid.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}

