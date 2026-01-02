/// Dither effect example
/// Demonstrates Floyd-Steinberg dithering

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply dither effect (1-bit per channel)
    let effect = ImageEffect::Dither(
        image_effect::special_effect::DitherConfig::new().with_depth(1),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("dither_effect.png"))?;

    println!("✓ Dither effect applied successfully!");
    println!("  Depth: 1 bit per channel");
    println!("  Effect: tmp/dither_effect.png");

    Ok(())
}
