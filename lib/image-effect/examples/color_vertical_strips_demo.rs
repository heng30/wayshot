use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::ColorVerticalStrips(
        image_effect::special::ColorVerticalStripsConfig::new(),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("color_vertical_strips_effect.png"))?;

    println!("✓ Color vertical strips effect applied successfully!");
    println!("  Effect: tmp/color_vertical_strips_effect.png");

    Ok(())
}
