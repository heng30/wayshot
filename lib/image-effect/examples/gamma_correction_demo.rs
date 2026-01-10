/// Gamma correction effect example
/// Demonstrates gamma correction for color adjustment

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply gamma correction effect
    let effect = ImageEffect::GammaCorrection(
        image_effect::colour_space::GammaCorrectionConfig::new()
            .with_red(2.2)
            .with_green(2.2)
            .with_blue(2.2),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("gamma_correction_effect.png"))?;

    println!("✓ Gamma correction effect applied successfully!");
    println!("  Gamma: RGB(2.2, 2.2, 2.2)");
    println!("  Effect: tmp/gamma_correction_effect.png");

    Ok(())
}
