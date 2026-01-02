use image::ImageReader;
use image_effect::{
    Effect, ImageEffect,
    monochrome_effect::{GrayscaleConfig, GrayscaleMode},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let modes = [
        GrayscaleMode::Average,
        GrayscaleMode::Luminance,
        GrayscaleMode::RedChannel,
        GrayscaleMode::GreenChannel,
        GrayscaleMode::BlueChannel,
    ];

    for mode in modes {
        let mut test_img = img.clone();
        let effect = ImageEffect::Grayscale(GrayscaleConfig::new().with_mode(mode));

        test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("grayscale_{:?}.png", mode)
            .to_lowercase()
            .replace("::", "_");
        test_img.save(output_dir.join(&filename))?;

        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All grayscale effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
