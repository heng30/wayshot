/// Hue rotation example
/// Demonstrates rotating the hue of an image


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Test different hue rotation angles
    use image_effect::base_effect::HueRotateConfig;

    let angles = [0, 45, 90, 135, 180, 225, 270, 315];

    for angle in angles {
        let mut test_img = img.clone();
        let effect = ImageEffect::HueRotate(
            HueRotateConfig::new().with_degrees(angle)
        );

        effect.apply(&mut test_img)?;

        let filename = format!("hue_rotate_{}.png", angle);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All hue rotation effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
