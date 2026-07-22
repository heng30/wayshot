use background_animation::{Animation, AnimationRecordConfig, triangle::TriangleConfig};

fn main() {
    env_logger::init();

    let output_path = "output/triangle_animation.mp4";
    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create triangle mesh animation matching the JS triangle.js (Thpace) reference:
    // - Delaunay triangulation of noisy grid points
    // - Gradient coloring from purple (top-left) to teal (bottom-right)
    // - Animated vertex positions with sinusoidal wave motion
    // - Anti-aliased white stroke lines between triangles
    // - Floating white particles for atmosphere
    let mut triangle = TriangleConfig::new()
        .with_triangle_size(130.0)
        .with_bleed(200.0)
        .with_noise(60.0)
        .with_color1((54, 0, 51))      // Deep purple #360033
        .with_color2((11, 135, 147))   // Teal #0b8793
        .with_stroke_color((255, 255, 255, 255)) // White stroke
        .with_stroke_width(1.0)
        .with_point_variation_x(20.0)
        .with_point_variation_y(35.0)
        .with_point_animation_speed(15.0)
        .with_particle_count(80);

    let config = AnimationRecordConfig::new(
        1280,
        720,
        25,
        std::time::Duration::from_secs(10),
        output_path,
    );

    log::info!("Generating triangle mesh animation to {}", output_path);
    triangle
        .animate_record(config)
        .expect("animation failed");
    log::info!("Done! Output saved to {}", output_path);
}
