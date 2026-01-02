/// Gaussian noise effect example
/// Demonstrates random noise addition to images

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply gaussian noise effect
    let effect = ImageEffect::GaussianNoise(
        image_effect::noise_effect::GaussianNoiseConfig::new(),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("gaussian_noise_effect.png"))?;

    println!("✓ Gaussian noise effect applied successfully!");
    println!("  Effect:   tmp/gaussian_noise_effect.png");

    Ok(())
}
