use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::DesaturateHsv(
        image_effect::colour_space_effect::DesaturateHsvConfig::new().with_level(0.5),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("desaturate_hsv_effect.png"))?;

    println!("✓ Desaturate HSV effect applied successfully!");
    println!("  Amount: 0.5");
    println!("  Effect: tmp/desaturate_hsv_effect.png");

    Ok(())
}
