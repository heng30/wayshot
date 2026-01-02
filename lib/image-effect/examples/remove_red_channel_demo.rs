use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::RemoveRedChannel(
        image_effect::channel_effect::RemoveRedChannelConfig::new().with_min_filter(200),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("remove_red_channel_effect.png"))?;

    println!("✓ Remove red channel effect applied successfully!");
    println!("  Min filter: 200");
    println!("  Effect: tmp/remove_red_channel_effect.png");

    Ok(())
}
