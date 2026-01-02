use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::DarkenHsluv(
        image_effect::colour_space_effect::DarkenHsluvConfig::new().with_level(0.15),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("darken_hsluv_effect.png"))?;

    println!("✓ Darken Hsluv effect applied successfully!");
    println!("  Amount: 15.0");
    println!("  Effect: tmp/darken_hsluv_effect.png");

    Ok(())
}
