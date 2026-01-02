use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::AlterChannels(
        image_effect::channel_effect::AlterChannelsConfig::new()
            .with_r_amt(30)
            .with_g_amt(-20)
            .with_b_amt(40),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("alter_channels_effect.png"))?;

    println!("✓ Alter channels effect applied successfully!");
    println!("  Red: +30, Green: -20, Blue: +40");
    println!("  Effect: tmp/alter_channels_effect.png");

    Ok(())
}
