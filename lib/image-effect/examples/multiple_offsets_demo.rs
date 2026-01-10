use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::MultipleOffsets(
        image_effect::special::MultipleOffsetsConfig::new()
            .with_channel_index(0)
            .with_channel_index2(2)
            .with_offset(10),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("multiple_offsets_effect.png"))?;

    println!("✓ Multiple offsets effect applied successfully!");
    println!("  Channel 1: Red (0)");
    println!("  Channel 2: Blue (2)");
    println!("  Offset: 10 pixels");
    println!("  Effect: tmp/multiple_offsets_effect.png");

    Ok(())
}
