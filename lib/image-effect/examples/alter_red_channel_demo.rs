use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::AlterRedChannel(
        image_effect::channel::AlterRedChannelConfig::new().with_amount(50),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("alter_red_channel_effect.png"))?;

    println!("✓ Alter red channel effect applied successfully!");
    println!("  Amount: +50");
    println!("  Effect: tmp/alter_red_channel_effect.png");

    Ok(())
}
