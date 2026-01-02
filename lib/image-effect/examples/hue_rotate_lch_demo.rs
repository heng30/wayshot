use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::HueRotateLch(
        image_effect::colour_space_effect::HueRotateLchConfig::new().with_degrees(60.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("hue_rotate_lch_effect.png"))?;

    println!("✓ Hue rotate LCh effect applied successfully!");
    println!("  Degrees: 60.0");
    println!("  Effect: tmp/hue_rotate_lch_effect.png");

    Ok(())
}
