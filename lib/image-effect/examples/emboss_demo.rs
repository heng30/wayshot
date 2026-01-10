/// Emboss effect example
/// Demonstrates emboss relief effect


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::stylized::EmbossConfig;

    let effect = ImageEffect::Emboss(EmbossConfig::new());
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("emboss_effect.png"))?;

    println!("✓ Emboss effect applied successfully!");
    println!("  Effect:   tmp/emboss_effect.png");

    Ok(())
}
