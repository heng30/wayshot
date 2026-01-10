use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::DesaturateHsluv(
        image_effect::colour_space::DesaturateHsluvConfig::new().with_level(0.3),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("desaturate_hsluv_effect.png"))?;

    println!("✓ Desaturate Hsluv effect applied successfully!");
    println!("  Amount: 0.3");
    println!("  Effect: tmp/desaturate_hsluv_effect.png");

    Ok(())
}
