/// Frosted glass effect example
/// Demonstrates frosted glass transparency effect

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply frosted glass effect
    let effect = ImageEffect::FrostedGlass(
        image_effect::special_effect::FrostedGlassConfig::new(),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("frosted_glass_effect.png"))?;

    println!("✓ Frosted glass effect applied successfully!");
    println!("  Effect: tmp/frosted_glass_effect.png");

    Ok(())
}
