use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::DarkenLch(
        image_effect::colour_space::DarkenLchConfig::new().with_level(0.2),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("darken_lch_effect.png"))?;

    println!("✓ Darken LCh effect applied successfully!");
    println!("  Amount: 20.0");
    println!("  Effect: tmp/darken_lch_effect.png");

    Ok(())
}
