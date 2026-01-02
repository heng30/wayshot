use image::ImageReader;
use image_effect::base_effect::SaturationConfig;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let saturation_levels = [-0.8, -0.4, 0.0, 0.4, 0.8];

    for level in saturation_levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::Saturation(SaturationConfig::new().with_amount(level));

        effect.apply(&mut test_img)?;

        let filename = if level >= 0.0 {
            format!("saturation_+{}.png", level)
        } else {
            format!("saturation_{}.png", level)
        };

        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All saturation effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
