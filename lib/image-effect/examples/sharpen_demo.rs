/// Sharpen effect example
/// Demonstrates image sharpening


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::stylized_effect::SharpenConfig;

    let effect = ImageEffect::Sharpen(SharpenConfig::new());
    effect.apply(&mut img)?;

    img.save(output_dir.join("sharpen_effect.png"))?;

    println!("✓ Sharpen effect applied successfully!");
    println!("  Effect:   tmp/sharpen_effect.png");

    Ok(())
}
