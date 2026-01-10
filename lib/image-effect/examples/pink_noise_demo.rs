use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::PinkNoise(image_effect::noise::PinkNoiseConfig::new());
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("pink_noise_effect.png"))?;

    println!("✓ Pink noise effect applied successfully!");
    println!("  Effect:   tmp/pink_noise_effect.png");

    Ok(())
}
