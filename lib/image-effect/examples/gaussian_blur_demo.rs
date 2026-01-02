/// Gaussian blur effect example
/// Demonstrates different blur radius values


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Test different blur radii
    use image_effect::blur_effect::GaussianBlurConfig;

    let radii = [1, 2, 3, 5, 7, 10];

    for radius in radii {
        let mut test_img = img.clone();
        let effect = ImageEffect::GaussianBlur(
            GaussianBlurConfig::new().with_radius(radius)
        );

        effect.apply(&mut test_img)?;

        let filename = format!("gaussian_blur_r{}.png", radius);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All gaussian blur effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
