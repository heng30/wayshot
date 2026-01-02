/// Edge detection effect example
/// Demonstrates various edge detection modes


use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::stylized_effect::{EdgeDetectionConfig, EdgeDetectionMode};

    let modes = [
        EdgeDetectionMode::Standard,
        EdgeDetectionMode::SobelHorizontal,
        EdgeDetectionMode::SobelVertical,
        EdgeDetectionMode::SobelGlobal,
    ];

    for mode in modes {
        let mut test_img = img.clone();
        let effect = ImageEffect::EdgeDetection(
            EdgeDetectionConfig::new().with_mode(mode)
        );

        test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("edge_detection_{:?}.png", mode).to_lowercase();
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All edge detection effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
