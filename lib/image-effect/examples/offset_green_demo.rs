use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::OffsetGreen(
        image_effect::special::OffsetGreenConfig::new().with_offset_amt(12),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("offset_green_effect.png"))?;

    println!("✓ Offset green effect applied successfully!");
    println!("  Offset: 12 pixels");
    println!("  Effect: tmp/offset_green_effect.png");

    Ok(())
}
