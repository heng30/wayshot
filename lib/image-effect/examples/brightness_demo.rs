/// Brightness adjustment example
/// Demonstrates increasing and decreasing brightness


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Test different brightness levels
    use image_effect::base_effect::BrightnessConfig;

    let brightness_levels = [-50, -20, 0, 20, 50];

    for level in brightness_levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::Brightness(
            BrightnessConfig::new().with_brightness(level)
        );

        effect.apply(&mut test_img)?;

        let filename = if level >= 0 {
            format!("brightness_+{}.png", level)
        } else {
            format!("_brightness_{}.png", level)
        };

        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All brightness effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
