use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::AlterGreenChannel(
        image_effect::channel_effect::AlterGreenChannelConfig::new().with_amount(50),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("alter_green_channel_effect.png"))?;

    println!("✓ Alter green channel effect applied successfully!");
    println!("  Amount: +50");
    println!("  Effect: tmp/alter_green_channel_effect.png");

    Ok(())
}
