use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::OffsetBlue(
        image_effect::special::OffsetBlueConfig::new().with_offset_amt(8),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("offset_blue_effect.png"))?;

    println!("✓ Offset blue effect applied successfully!");
    println!("  Offset: 8 pixels");
    println!("  Effect: tmp/offset_blue_effect.png");

    Ok(())
}
