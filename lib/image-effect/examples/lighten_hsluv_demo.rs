use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::LightenHsluv(
        image_effect::colour_space::LightenHsluvConfig::new().with_level(0.15),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("lighten_hsluv_effect.png"))?;

    println!("✓ Lighten Hsluv effect applied successfully!");
    println!("  Amount: 15.0");
    println!("  Effect: tmp/lighten_hsluv_effect.png");

    Ok(())
}
