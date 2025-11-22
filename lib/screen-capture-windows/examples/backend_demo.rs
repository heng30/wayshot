use image::{ImageBuffer, Rgba};
use screen_capture_windows::DXGIManager;
use std::time::Instant;

fn main() {
    let mut manager = DXGIManager::new(300).unwrap();

    println!("Starting screen capture benchmark...");
    println!();

    for i in 0..1000 {
        let start_time = Instant::now();

        match manager.capture_frame_rgba() {
            Ok((data, (width, height))) => {
                let capture_time = start_time.elapsed();

                if i < 10 {
                    let filename = format!("capture_{}.png", i);
                    let save_start = Instant::now();

                    if let Err(e) = save_rgba_as_png(&data, width, height, &filename) {
                        println!("Failed to save {}: {}", filename, e);
                    }
                    let save_time = save_start.elapsed();
                    let total_time = start_time.elapsed();

                    println!(
                        "Frame {}: {}x{} | Capture: {:.2?} | Save: {:.2?} | Total: {:.2?}",
                        i, width, height, capture_time, save_time, total_time
                    );
                } else {
                    println!(
                        "Frame {}: {}x{} | Capture: {:.2?}",
                        i, width, height, capture_time,
                    );
                }
            }
            Err(e) => {
                let elapsed = start_time.elapsed();
                println!(
                    "Frame {}: Capture failed after {:.2?} - error: {:?}",
                    i, elapsed, e
                );
            }
        }
    }

    println!("\nBenchmark completed.");
}

fn save_rgba_as_png(
    rgba_data: &[u8],
    width: usize,
    height: usize,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, rgba_data)
        .ok_or("Failed to create image buffer from RGBA data")?;

    img.save(filename)?;
    Ok(())
}
