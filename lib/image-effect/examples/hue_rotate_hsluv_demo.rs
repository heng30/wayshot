use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::HueRotateHsluv(
        image_effect::colour_space::HueRotateHsluvConfig::new().with_degrees(30.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("hue_rotate_hsluv_effect.png"))?;

    println!("✓ Hue rotate Hsluv effect applied successfully!");
    println!("  Degrees: 30.0");
    println!("  Effect: tmp/hue_rotate_hsluv_effect.png");

    Ok(())
}
