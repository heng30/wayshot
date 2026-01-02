use image::ImageReader;
use image_effect::special_effect::BrightnessConfig;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let brightness_levels = [-50, -20, 0, 20, 50];

    for level in brightness_levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::Brightness(BrightnessConfig::new().with_brightness(level));

        let filename = if level >= 0 {
            format!("brightness_+{}.png", level)
        } else {
            format!("brightness_{}.png", level)
        };

        test_img = effect.apply(test_img).expect("Effect failed");
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All brightness effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
