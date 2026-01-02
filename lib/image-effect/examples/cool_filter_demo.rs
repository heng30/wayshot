use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::CoolFilter(
        image_effect::filter_effect::TemperatureConfig::new().with_amount(-30.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("cool_filter_effect.png"))?;

    println!("✓ Cool filter effect applied successfully!");
    println!("  Temperature: -30");
    println!("  Effect: tmp/cool_filter_effect.png");

    Ok(())
}
