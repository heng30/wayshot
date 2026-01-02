use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let img = ImageEffect::Invert.apply(img).expect("Effect failed");
    img.save(output_dir.join("invert_effect.png"))?;

    println!("✓ Invert effect applied successfully!");
    println!("  Effect:   tmp/invert_effect.png");

    Ok(())
}
