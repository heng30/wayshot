use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Lighten blue colors (0, 0, 255) by 30%
    let effect = ImageEffect::SelectiveLighten(
        image_effect::channel::SelectiveLightenConfig::new()
            .with_ref_r(0)
            .with_ref_g(0)
            .with_ref_b(255)
            .with_amt(0.3),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("selective_lighten_effect.png"))?;

    println!("✓ Selective lighten effect applied successfully!");
    println!("  Reference color: RGB(0, 0, 255) [Blue]");
    println!("  Lighten amount: 30%");
    println!("  Effect: tmp/selective_lighten_effect.png");

    Ok(())
}
