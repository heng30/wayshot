use image_animation::{GradeMarkConfig, GradeMarkType};

fn main() {
    env_logger::init();

    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create circle animation (red) with brush effect and end pause
    let mut circle = GradeMarkConfig::new(GradeMarkType::Circle)
        .with_color((255, 80, 80, 255)) // Red
        .with_size(80.0)
        .with_line_width(6.0)         // Base width (will vary with brush effect)
        .with_duration_ms(500)        // Drawing duration
        .with_end_pause(1.5)          // Hold for 1.5 seconds after complete
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    circle
        .record_webp("output/grade_circle.webp")
        .expect("Circle recording failed");
    println!("Circle animation saved to output/grade_circle.webp");

    // Create checkmark animation (green) with brush effect and end pause
    let mut checkmark = GradeMarkConfig::new(GradeMarkType::Checkmark)
        .with_color((100, 255, 100, 255)) // Green
        .with_size(100.0)
        .with_line_width(8.0)         // Larger brush for more visible effect
        .with_duration_ms(600)        // Drawing duration
        .with_end_pause(2.0)          // Hold for 2 seconds after complete
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    checkmark
        .record_webp("output/grade_checkmark.webp")
        .expect("Checkmark recording failed");
    println!("Checkmark animation saved to output/grade_checkmark.webp");

    // Create cross animation (red) with brush effect and end pause
    let mut cross = GradeMarkConfig::new(GradeMarkType::Cross)
        .with_color((255, 80, 80, 255)) // Red
        .with_size(90.0)
        .with_line_width(7.0)         // Medium brush
        .with_duration_ms(400)        // Faster drawing
        .with_end_pause(1.0)          // Shorter pause
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    cross
        .record_webp("output/grade_cross.webp")
        .expect("Cross recording failed");
    println!("Cross animation saved to output/grade_cross.webp");

    println!("All grade mark animations complete!");
    println!("Brush effect: strokes start/end thin, thick in middle (毛笔效果)");
    println!("End pause: completed mark stays visible for extra seconds");
}
