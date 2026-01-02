/// Vignette effect example
/// Demonstrates different vignette strengths


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::filter_effect::VignetteConfig;

    // Test different vignette strengths
    let strengths = [0.1, 0.3, 0.5, 0.7];

    for strength in strengths {
        let mut test_img = img.clone();
        let effect = ImageEffect::Vignette(
            VignetteConfig::new().with_strength(strength)
        );

test_img = effect.apply(test_img).expect("Effect failed");
        let filename = format!("vignette_{:.1}.png", strength);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All vignette effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
