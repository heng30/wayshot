/// Color tint effect example
/// Demonstrates applying different color tints to images


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::filter_effect::ColorTintConfig;

    // Test different color tints
    let tints = [
        (255, 0, 0, "red"),
        (0, 255, 0, "green"),
        (0, 0, 255, "blue"),
        (255, 255, 0, "yellow"),
        (255, 0, 255, "magenta"),
        (0, 255, 255, "cyan"),
        (255, 128, 0, "orange"),
    ];

    for (r, g, b, name) in tints {
        let mut test_img = img.clone();
        let effect = ImageEffect::ColorTint(
            ColorTintConfig::from_rgb(r, g, b)
        );

        effect.apply(&mut test_img)?;
        let filename = format!("color_tint_{}.png", name);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All color tint effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
