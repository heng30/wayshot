/// Sepia tone effect example
/// Demonstrates vintage sepia effect


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply sepia effect
    use image_effect::filter::SepiaConfig;

    let effect = ImageEffect::Sepia(SepiaConfig::new());
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("sepia_effect.png"))?;

    println!("✓ Sepia effect applied successfully!");
    println!("  Effect:   tmp/sepia_effect.png");

    Ok(())
}
