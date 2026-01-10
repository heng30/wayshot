/// LCh saturation effect example
/// Demonstrates saturation adjustment in LCh color space

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply LCh saturate effect
    let effect = ImageEffect::SaturateLch(
        image_effect::colour_space::SaturateLchConfig::new().with_level(0.3),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("saturate_lch_effect.png"))?;

    println!("✓ LCh saturate effect applied successfully!");
    println!("  Level: 0.3");
    println!("  Effect: tmp/saturate_lch_effect.png");

    Ok(())
}
