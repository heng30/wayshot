use image::ImageReader;
use image_effect::colour_space_effect::HueRotateConfig;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let angles = [0, 45, 90, 135, 180, 225, 270, 315];

    for angle in angles {
        let mut test_img = img.clone();
        let effect = ImageEffect::HueRotate(HueRotateConfig::new().with_degrees(angle));

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("hue_rotate_{}.png", angle);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All hue rotation effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
