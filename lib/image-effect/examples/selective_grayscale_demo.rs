use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Convert purple colors (128, 0, 128) to grayscale
    let effect = ImageEffect::SelectiveGrayscale(
        image_effect::channel::SelectiveGrayscaleConfig::new()
            .with_ref_r(128)
            .with_ref_g(0)
            .with_ref_b(128),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("selective_grayscale_effect.png"))?;

    println!("✓ Selective grayscale effect applied successfully!");
    println!("  Reference color: RGB(128, 0, 128) [Purple]");
    println!("  Effect: tmp/selective_grayscale_effect.png");

    Ok(())
}
