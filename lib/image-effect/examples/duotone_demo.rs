/// Duotone effect demo
/// Demonstrates duotone with different color combinations

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::monochrome_effect::DuotoneConfig;

    // Different duotone combinations
    let duotones = [
        ((0, 0, 255), (128, 128, 128), "blue_gray"),
        ((255, 0, 0), (255, 255, 0), "red_yellow"),
        ((128, 0, 255), (255, 200, 0), "purple_gold"),
        ((0, 100, 50), (200, 50, 0), "teal_orange"),
        ((255, 100, 0), (50, 100, 255), "orange_blue"),
    ];

    for (primary, secondary, name) in duotones {
        let mut test_img = img.clone();
        let effect = ImageEffect::Duotone(
            DuotoneConfig::from_primary_rgb(primary.0, primary.1, primary.2)
                .with_secondary_rgb(secondary.0, secondary.1, secondary.2)
        );

        effect.apply(&mut test_img)?;

        let filename = format!("duotone_{}.png", name);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All duotone effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
