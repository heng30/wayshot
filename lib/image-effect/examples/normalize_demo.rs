/// Normalize effect example
/// Demonstrates contrast stretching normalization

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply normalize effect
    let effect = ImageEffect::Normalize(image_effect::special::NormalizeConfig::new());
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("normalize_effect.png"))?;

    println!("✓ Normalize effect applied successfully!");
    println!("  Effect: tmp/normalize_effect.png");

    Ok(())
}
