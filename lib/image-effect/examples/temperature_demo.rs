/// Temperature filter example
/// Demonstrates warm and cool color temperature adjustments


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::filter_effect::TemperatureConfig;

    // Test warm filters
    let warm_levels = [0.2, 0.5, 0.8];
    for level in warm_levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::WarmFilter(
            TemperatureConfig::new().with_amount(level)
        );

test_img = effect.apply(test_img).expect("Effect failed");
        let filename = format!("warm_filter_{:.1}.png", level);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    // Test cool filters
    let cool_levels = [0.2, 0.5, 0.8];
    for level in cool_levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::CoolFilter(
            TemperatureConfig::new().with_amount(level)
        );

test_img = effect.apply(test_img).expect("Effect failed");
        let filename = format!("cool_filter_{:.1}.png", level);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All temperature effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
