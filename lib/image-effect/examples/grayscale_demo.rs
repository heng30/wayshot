/// Grayscale effect example
/// Demonstrates different grayscale modes


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create output directory
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Create test image with colorful patterns
    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Save original

    // Test different grayscale modes
    use image_effect::base_effect::{GrayscaleConfig, GrayscaleMode};

    let modes = [
        GrayscaleMode::Average,
        GrayscaleMode::Luminance,
        GrayscaleMode::RedChannel,
        GrayscaleMode::GreenChannel,
        GrayscaleMode::BlueChannel,
    ];

    for mode in modes {
        let mut test_img = img.clone();
        let effect = ImageEffect::Grayscale(
            GrayscaleConfig::new().with_mode(mode)
        );

        effect.apply(&mut test_img)?;

        let filename = format!("grayscale_{:?}.png", mode).to_lowercase().replace("::", "_");
        test_img.save(output_dir.join(&filename))?;

        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All grayscale effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
