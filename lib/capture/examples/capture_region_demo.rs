use capture::{LogicalSize, Position};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let screen_infos = capture::available_screens()?;
    assert!(!screen_infos.is_empty());

    let start = Instant::now();
    let output = capture::capture_region(
        &screen_infos[0].name,
        Position::new(100, 100),
        LogicalSize::new(200, 200),
        true,
    )?;
    let capture_time = start.elapsed();

    println!("{} x {}", output.width, output.height);
    println!("Capture time: {:?}", capture_time);

    let temp_file = "target/screenshot-region.png";

    let img =
        image::RgbaImage::from_raw(output.width as u32, output.height as u32, output.pixel_data)
            .unwrap();

    img.save(temp_file)?;
    println!("Screenshot saved to: {}", temp_file);

    Ok(())
}
