use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::IncBrightness(
        image_effect::special::IncBrightnessConfig::new(),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("inc_brightness_effect.png"))?;

    println!("✓ Increase brightness effect applied successfully!");
    println!("  Effect: tmp/inc_brightness_effect.png");

    Ok(())
}
