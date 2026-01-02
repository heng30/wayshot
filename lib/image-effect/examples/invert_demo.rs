use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Create test image with various colors
    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Save original

    // Apply invert effect
    let effect = ImageEffect::Invert;
    effect.apply(&mut img)?;

    // Save result
    img.save(output_dir.join("invert_effect.png"))?;

    println!("✓ Invert effect applied successfully!");
    println!("  Effect:   tmp/invert_effect.png");

    Ok(())
}
