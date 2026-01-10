/// Halftone effect example
/// Demonstrates halftone printing effect

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply halftone effect
    let effect = ImageEffect::Halftone(image_effect::special::HalftoneConfig::new());
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("halftone_effect.png"))?;

    println!("✓ Halftone effect applied successfully!");
    println!("  Effect: tmp/halftone_effect.png");

    Ok(())
}
