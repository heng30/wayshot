use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Desaturate green colors (0, 255, 0) by 40%
    let effect = ImageEffect::SelectiveDesaturate(
        image_effect::channel::SelectiveDesaturateConfig::new()
            .with_ref_r(0)
            .with_ref_g(255)
            .with_ref_b(0)
            .with_amt(0.4),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("selective_desaturate_effect.png"))?;

    println!("✓ Selective desaturate effect applied successfully!");
    println!("  Reference color: RGB(0, 255, 0) [Green]");
    println!("  Desaturate amount: 40%");
    println!("  Effect: tmp/selective_desaturate_effect.png");

    Ok(())
}
