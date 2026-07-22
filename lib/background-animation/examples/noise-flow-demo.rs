use background_animation::{Animation, AnimationRecordConfig, noise_flow::NoiseFlowConfig};

fn main() {
    env_logger::init();

    let output_path = "output/noise_flow_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut noise_flow = NoiseFlowConfig::new()
        .with_animation_speed(0.01)
        .with_noise_scale(0.005);

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating noise flow animation to {}", output_path);
    noise_flow.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}

