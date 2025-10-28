use std::time::Instant;
use wayland_wlr_screen_capture as capture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let output = capture::capture_all_outputs(true)?;
    let capture_time = start.elapsed();
    println!("{} x {}", output.width, output.height);
    println!("Capture time: {:?}", capture_time);

    let temp_file = "/tmp/screenshot-all.png";

    let img =
        image::RgbaImage::from_raw(output.width as u32, output.height as u32, output.pixel_data)
            .unwrap();

    img.save(temp_file)?;
    println!("Screenshot saved to: {}", temp_file);

    Ok(())
}
