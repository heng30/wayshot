/// Contrast adjustment example
/// Demonstrates increasing and decreasing contrast


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Test different contrast levels
    use image_effect::base_effect::ContrastConfig;

    let contrast_levels = [-30.0, -10.0, 0.0, 10.0, 30.0];

    for level in contrast_levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::Contrast(
            ContrastConfig::new().with_contrast(level)
        );

        effect.apply(&mut test_img)?;

        let filename = if level >= 0.0 {
            format!("contrast_+{}.png", level)
        } else {
            format!("contrast_{}.png", level)
        };

        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All contrast effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
