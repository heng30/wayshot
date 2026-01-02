use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    // Rotate hue for red colors (255, 0, 0) by 90 degrees
    let effect = ImageEffect::SelectiveHueRotate(
        image_effect::channel_effect::SelectiveHueRotateConfig::new()
            .with_ref_r(255)
            .with_ref_g(0)
            .with_ref_b(0)
            .with_degrees(90.0),
    );
    img = effect.apply(img).expect("Effect failed");

    img.save(output_dir.join("selective_hue_rotate_effect.png"))?;

    println!("✓ Selective hue rotate effect applied successfully!");
    println!("  Reference color: RGB(255, 0, 0) [Red]");
    println!("  Rotation: 90 degrees");
    println!("  Effect: tmp/selective_hue_rotate_effect.png");

    Ok(())
}
