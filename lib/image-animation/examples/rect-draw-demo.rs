use image_animation::{DashStyle, LineStyle, RectDrawConfig, RectStyle};

fn main() {
    env_logger::init();

    std::fs::create_dir_all("output").expect("failed to create output directory");

    // 1. Solid white rectangle with sharp corners
    let mut solid = RectDrawConfig::new()
        .with_line_style(LineStyle {
            color: (255, 255, 255, 255),
            width: 4.0,
            dash: DashStyle::Solid,
        })
        .with_rect_style(RectStyle {
            width: 300.0,
            height: 200.0,
            corner_radius: 0.0,
        })
        .with_duration_ms(800)
        .with_end_pause(1.5)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    solid
        .record_webp("output/rect_solid.webp")
        .expect("Solid rect recording failed");
    println!("Solid rectangle saved to output/rect_solid.webp");

    // 2. Dashed red rectangle with sharp corners
    let mut dashed = RectDrawConfig::new()
        .with_line_style(LineStyle {
            color: (255, 80, 80, 255),
            width: 4.0,
            dash: DashStyle::Dash(15.0),
        })
        .with_rect_style(RectStyle {
            width: 300.0,
            height: 200.0,
            corner_radius: 0.0,
        })
        .with_duration_ms(800)
        .with_end_pause(1.5)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    dashed
        .record_webp("output/rect_dashed.webp")
        .expect("Dashed rect recording failed");
    println!("Dashed rectangle saved to output/rect_dashed.webp");

    // 3. Solid green rectangle with rounded corners
    let mut rounded = RectDrawConfig::new()
        .with_line_style(LineStyle {
            color: (80, 255, 80, 255),
            width: 5.0,
            dash: DashStyle::Solid,
        })
        .with_rect_style(RectStyle {
            width: 300.0,
            height: 200.0,
            corner_radius: 30.0,
        })
        .with_duration_ms(1000)
        .with_end_pause(1.5)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    rounded
        .record_webp("output/rect_rounded.webp")
        .expect("Rounded rect recording failed");
    println!("Rounded rectangle saved to output/rect_rounded.webp");

    // 4. Dashed blue rectangle with rounded corners and thick line
    let mut dashed_rounded = RectDrawConfig::new()
        .with_line_style(LineStyle {
            color: (80, 150, 255, 255),
            width: 6.0,
            dash: DashStyle::Custom(vec![20.0, 10.0, 5.0, 10.0]), // dash-dot pattern
        })
        .with_rect_style(RectStyle {
            width: 280.0,
            height: 180.0,
            corner_radius: 25.0,
        })
        .with_duration_ms(1200)
        .with_end_pause(2.0)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    dashed_rounded
        .record_webp("output/rect_dashed_rounded.webp")
        .expect("Dashed rounded rect recording failed");
    println!("Dashed rounded rectangle saved to output/rect_dashed_rounded.webp");

    // 5. Yellow square with large corner radius (almost circle)
    let mut almost_circle = RectDrawConfig::new()
        .with_line_style(LineStyle {
            color: (255, 220, 50, 255),
            width: 5.0,
            dash: DashStyle::Dash(8.0),
        })
        .with_rect_style(RectStyle {
            width: 250.0,
            height: 250.0,
            corner_radius: 125.0, // half of width/height → circle
        })
        .with_duration_ms(1500)
        .with_end_pause(1.0)
        .with_position((0.5, 0.5))
        .with_width(400)
        .with_height(400);

    almost_circle
        .record_webp("output/rect_almost_circle.webp")
        .expect("Almost circle recording failed");
    println!("Almost circle saved to output/rect_almost_circle.webp");

    println!("All rectangle animations complete!");
}
