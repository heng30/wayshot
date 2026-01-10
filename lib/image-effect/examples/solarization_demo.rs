/// Solarization effect demo
/// Demonstrates solarization with different modes and thresholds

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::monochrome::{SolarizationConfig, SolarizationMode};

    // Test different solarization modes
    let modes = [
        SolarizationMode::RG,
        SolarizationMode::RB,
        SolarizationMode::GB,
        SolarizationMode::RGB,
    ];

    for mode in modes {
        let mut test_img = img.clone();
        let effect = ImageEffect::Solarization(
            SolarizationConfig::new()
                .with_mode(mode)
                .with_threshold(128)
        );

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("solarization_{:?}_128.png", mode).to_lowercase();
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    // Test different thresholds
    let thresholds = [64, 128, 192];
    for threshold in thresholds {
        let mut test_img = img.clone();
        let effect = ImageEffect::Solarization(
            SolarizationConfig::new()
                .with_mode(SolarizationMode::RGB)
                .with_threshold(threshold)
        );

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("solarization_rgb_{}.png", threshold);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All solarization effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
