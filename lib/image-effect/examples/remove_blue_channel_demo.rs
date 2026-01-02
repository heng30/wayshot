use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::RemoveBlueChannel(
        image_effect::channel_effect::RemoveBlueChannelConfig::new().with_min_filter(200),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("remove_blue_channel_effect.png"))?;

    println!("✓ Remove blue channel effect applied successfully!");
    println!("  Min filter: 200");
    println!("  Effect: tmp/remove_blue_channel_effect.png");

    Ok(())
}
