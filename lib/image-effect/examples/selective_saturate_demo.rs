use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Saturate yellow colors (255, 255, 0) by 50%
    let effect = ImageEffect::SelectiveSaturate(
        image_effect::channel_effect::SelectiveSaturateConfig::new()
            .with_ref_r(255)
            .with_ref_g(255)
            .with_ref_b(0)
            .with_amt(0.5),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("selective_saturate_effect.png"))?;

    println!("✓ Selective saturate effect applied successfully!");
    println!("  Reference color: RGB(255, 255, 0) [Yellow]");
    println!("  Saturate amount: 50%");
    println!("  Effect: tmp/selective_saturate_effect.png");

    Ok(())
}
