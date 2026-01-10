use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::AlterTwoChannels(
        image_effect::channel::AlterTwoChannelsConfig::new()
            .with_channel1(0)
            .with_channel2(2)
            .with_amt1(250)
            .with_amt2(40),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("alter_two_channel_effect.png"))?;

    println!("✓ Alter two channel effect applied successfully!");
    println!("  Effect: tmp/alter_two_channel_effect.png");

    Ok(())
}
