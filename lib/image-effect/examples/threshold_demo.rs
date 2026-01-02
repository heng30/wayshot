/// Threshold effect demo
/// Demonstrates thresholding with different threshold values

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::monochrome_effect::ThresholdConfig;

    // Test different threshold levels
    let thresholds = [64, 96, 128, 160, 192];

    for threshold in thresholds {
        let mut test_img = img.clone();
        let effect = ImageEffect::Threshold(
            ThresholdConfig::new().with_threshold(threshold)
        );

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("threshold_{}.png", threshold);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All threshold effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
