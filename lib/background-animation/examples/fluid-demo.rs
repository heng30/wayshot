use background_animation::{Animation, AnimationRecordConfig, fluid::FluidConfig};

fn main() {
    env_logger::init();

    let output_path = "output/fluid_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut fluid = FluidConfig::new()
        .with_resolution_divisor(6)
        .with_viscosity(0.00005)
        .with_diffusion(0.00005)
        .with_force_source(background_animation::fluid::ForceSource::Vortices)
        .with_num_sources(5)
        .with_steps_per_frame(5)
        .with_color_injection(true)
        .with_colors(vec![
            (10, 20, 40),
            (30, 60, 100),
            (60, 100, 150),
            (100, 150, 200),
            (150, 200, 230),
            (200, 240, 255),
        ])
        .with_bg_color((5, 10, 20));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating fluid dynamics animation to {}", output_path);
    fluid.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}