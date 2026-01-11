/// Pixelate effect example
/// Demonstrates different pixelation levels
use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::stylized::PixelateConfig;

    let block_sizes = [4, 8, 12, 16, 20, 30];

    for block_size in block_sizes {
        let mut test_img = img.clone();
        let effect = ImageEffect::Pixelate(PixelateConfig::new().with_block_size(block_size));

        test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("pixelate_b{}.png", block_size);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All pixelate effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
