use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::WarmFilter(
        image_effect::filter::TemperatureConfig::new().with_amount(30.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("warm_filter_effect.png"))?;

    println!("✓ Warm filter effect applied successfully!");
    println!("  Temperature: +30");
    println!("  Effect: tmp/warm_filter_effect.png");

    Ok(())
}
