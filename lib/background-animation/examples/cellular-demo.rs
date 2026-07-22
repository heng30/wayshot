use background_animation::{Animation, AnimationRecordConfig, cellular::CellularConfig};

fn main() {
    env_logger::init();

    let output_path = "output/cellular_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut cellular = CellularConfig::new()
        .with_rule(background_animation::cellular::CellularRule::Conway)
        .with_cell_size(8)
        .with_age_gradient(true)
        .with_age_colors(vec![
            (50, 50, 200),
            (100, 150, 255),
            (150, 200, 255),
            (200, 255, 255),
            (255, 255, 255),
        ])
        .with_evolution_speed(2)
        .with_initial_density(0.3)
        .with_bg_color((10, 10, 20));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating cellular automata animation to {}", output_path);
    cellular.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}