use background_animation::{Animation, AnimationRecordConfig, flow_field::FlowFieldConfig};

fn main() {
    env_logger::init();

    let output_path = "output/flow_field_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut flow_field = FlowFieldConfig::new()
        .with_color((99, 102, 241, 255))
        .with_bg_color((0, 0, 0))
        .with_trail_opacity(0.15)
        .with_particle_count(2000)
        .with_speed(1.0)
        .with_pointer_enabled(true);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(5),
        output_path,
    );

    log::info!("Generating flow field animation to {}", output_path);
    flow_field.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
