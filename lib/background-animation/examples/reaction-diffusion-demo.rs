use background_animation::{Animation, AnimationRecordConfig, reaction_diffusion::ReactionDiffusionConfig};

fn main() {
    env_logger::init();

    let output_path = "output/reaction_diffusion_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    let mut reaction_diffusion = ReactionDiffusionConfig::new()
        .with_pattern(background_animation::reaction_diffusion::ReactionPattern::Mitosis)
        .with_resolution_divisor(4)
        .with_iterations_per_frame(15)
        .with_scale(1.0)
        .with_colors(vec![
            (0, 0, 30),
            (20, 40, 100),
            (50, 100, 150),
            (100, 150, 200),
            (150, 200, 230),
            (200, 230, 255),
            (255, 255, 255),
        ])
        .with_bg_color((0, 0, 0));

    let config = AnimationRecordConfig::new(1280, 720, 25, std::time::Duration::from_secs(5), output_path);

    log::info!("Generating reaction-diffusion animation to {}", output_path);
    reaction_diffusion.animate_record(config).expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}