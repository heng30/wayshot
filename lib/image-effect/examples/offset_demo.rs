/// Offset effect example
/// Demonstrates RGB channel offset for chromatic aberration effect

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Apply offset effect (red channel)
    let effect = ImageEffect::Offset(
        image_effect::special_effect::OffsetConfig::new()
            .with_channel_index(0)
            .with_offset(15),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("offset_effect.png"))?;

    println!("✓ Offset effect applied successfully!");
    println!("  Channel: Red (0)");
    println!("  Offset: 15 pixels");
    println!("  Effect: tmp/offset_effect.png");

    Ok(())
}
