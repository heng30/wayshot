/// Posterize effect example
/// Demonstrates different posterization levels


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::stylized_effect::PosterizeConfig;

    let levels = [2, 3, 4, 5, 6, 8];

    for level in levels {
        let mut test_img = img.clone();
        let effect = ImageEffect::Posterize(
            PosterizeConfig::new().with_levels(level)
        );

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("posterize_l{}.png", level);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All posterize effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
