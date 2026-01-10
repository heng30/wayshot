/// HSV hue rotate effect example
/// Demonstrates hue rotation in HSV color space

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply HSV hue rotate effect
    let effect = ImageEffect::HueRotateHsv(
        image_effect::colour_space::HueRotateHsvConfig::new().with_degrees(90.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("hue_rotate_hsv_effect.png"))?;

    println!("✓ HSV hue rotate effect applied successfully!");
    println!("  Degrees: 90.0");
    println!("  Effect:  tmp/hue_rotate_hsv_effect.png");

    Ok(())
}
