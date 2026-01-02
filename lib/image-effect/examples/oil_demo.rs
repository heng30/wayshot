/// Oil painting effect example
/// Demonstrates artistic oil painting effect

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply oil painting effect
    let effect = ImageEffect::Oil(
        image_effect::special_effect::OilConfig::new()
            .with_radius(4)
            .with_intensity(55.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("oil_effect.png"))?;

    println!("✓ Oil painting effect applied successfully!");
    println!("  Radius: 4");
    println!("  Intensity: 55.0");
    println!("  Effect: tmp/oil_effect.png");

    Ok(())
}
