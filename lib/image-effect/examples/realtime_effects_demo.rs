use image::ImageReader;
use image_effect::realtime::RealtimeImageEffect;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp/realtime");
    fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let img_path = Path::new("data/test.png");
    let sample_image = ImageReader::open(img_path)?.decode()?.to_rgba8();

    println!("Processing real-time effects demo...");
    println!(
        "Test image size: {}x{}",
        sample_image.width(),
        sample_image.height()
    );
    println!("Output directory: {}\n", output_dir.display());
    println!("{}", "=".repeat(80));

    println!(
        "{:<25} {:>12} {:>12} {:>15}",
        "Effect", "Time (ms)", "Max FPS", "Status"
    );
    println!("{}\n", "-".repeat(80));

    for effect in RealtimeImageEffect::all_effects() {
        let start = Instant::now();
        let result = effect.apply(sample_image.clone());
        let elapsed = start.elapsed();

        let status = if let Some(output_image) = result {
            let filename = format!("{}.png", effect.name().replace(' ', "_").to_lowercase());
            let filepath = output_dir.join(&filename);

            match output_image.save(&filepath) {
                Ok(_) => format!("Saved: {}", filename),
                Err(e) => format!("Error: {}", e),
            }
        } else {
            "Failed".to_string()
        };

        let time_ms = elapsed.as_secs_f64() * 1000.0;
        let max_fps = 1000 / time_ms as u32;

        println!(
            "{:<25} {:>12.3} {:>12} {:>15}",
            effect.name(),
            time_ms,
            max_fps,
            status
        );
    }

    println!("\n{}", "=".repeat(80));
    println!("\nProcessing complete!");
    println!("Check the tmp/realtime/ directory to see all effects.\n");

    Ok(())
}
