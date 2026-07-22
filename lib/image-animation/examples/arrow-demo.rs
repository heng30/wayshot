use image_animation::{ArrowDashStyle, ArrowDrawConfig, ArrowLineStyle, ArrowStyle};

fn main() {
    env_logger::init();

    std::fs::create_dir_all("output").expect("failed to create output directory");

    // 1. Solid white arrow pointing right
    let mut right_arrow = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color: (255, 255, 255, 255),
            width: 4.0,
            dash: ArrowDashStyle::Solid,
        })
        .with_arrow_style(ArrowStyle {
            length: 200.0,
            head_length: 40.0,
            head_width: 30.0,
            direction: 0.0, // 0° = right
        })
        .with_duration_ms(800)
        .with_end_pause(1.5)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    right_arrow
        .record_webp("output/arrow_right.webp")
        .expect("Right arrow recording failed");
    println!("Right arrow saved to output/arrow_right.webp");

    // 2. Red arrow pointing up
    let mut up_arrow = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color: (255, 80, 80, 255),
            width: 5.0,
            dash: ArrowDashStyle::Solid,
        })
        .with_arrow_style(ArrowStyle {
            length: 180.0,
            head_length: 45.0,
            head_width: 35.0,
            direction: 270.0, // 270° = up
        })
        .with_duration_ms(600)
        .with_end_pause(1.5)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    up_arrow
        .record_webp("output/arrow_up.webp")
        .expect("Up arrow recording failed");
    println!("Up arrow saved to output/arrow_up.webp");

    // 3. Dashed green arrow pointing diagonally (45°)
    let mut diagonal_arrow = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color: (80, 255, 80, 255),
            width: 4.0,
            dash: ArrowDashStyle::Dash(12.0),
        })
        .with_arrow_style(ArrowStyle {
            length: 250.0,
            head_length: 50.0,
            head_width: 40.0,
            direction: 45.0, // 45° = down-right
        })
        .with_duration_ms(1000)
        .with_end_pause(1.5)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    diagonal_arrow
        .record_webp("output/arrow_diagonal.webp")
        .expect("Diagonal arrow recording failed");
    println!("Diagonal arrow saved to output/arrow_diagonal.webp");

    // 4. Blue arrow pointing left with thick line and large head
    let mut left_arrow = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color: (80, 150, 255, 255),
            width: 6.0,
            dash: ArrowDashStyle::Custom(vec![20.0, 10.0, 5.0, 10.0]), // dash-dot pattern
        })
        .with_arrow_style(ArrowStyle {
            length: 220.0,
            head_length: 55.0,
            head_width: 50.0,
            direction: 180.0, // 180° = left
        })
        .with_duration_ms(1200)
        .with_end_pause(2.0)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    left_arrow
        .record_webp("output/arrow_left.webp")
        .expect("Left arrow recording failed");
    println!("Left arrow saved to output/arrow_left.webp");

    // 5. Yellow arrow pointing down with short shaft and big head
    let mut down_arrow = ArrowDrawConfig::new()
        .with_line_style(ArrowLineStyle {
            color: (255, 220, 50, 255),
            width: 5.0,
            dash: ArrowDashStyle::Solid,
        })
        .with_arrow_style(ArrowStyle {
            length: 120.0,
            head_length: 60.0,
            head_width: 60.0,
            direction: 90.0, // 90° = down
        })
        .with_duration_ms(800)
        .with_end_pause(1.0)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    down_arrow
        .record_webp("output/arrow_down.webp")
        .expect("Down arrow recording failed");
    println!("Down arrow saved to output/arrow_down.webp");

    println!("All arrow animations complete!");
}
