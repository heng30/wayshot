/// Color balance effect demo
/// Demonstrates color balance adjustments

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::monochrome::ColorBalanceConfig;

    // Test different color balance adjustments
    let balances = [
        ((50, 0, 0), "red_plus"),
        ((-50, 0, 0), "red_minus"),
        ((0, 50, 0), "green_plus"),
        ((0, 0, 50), "blue_plus"),
        ((30, 30, 0), "red_green_plus"),
        ((0, -30, 30), "green_minus_blue_plus"),
        ((-40, -20, 60), "all_shifted"),
    ];

    for ((r, g, b), name) in balances {
        let mut test_img = img.clone();
        let effect = ImageEffect::ColorBalance(
            ColorBalanceConfig::new()
                .with_red_shift(r)
                .with_green_shift(g)
                .with_blue_shift(b)
        );

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("color_balance_{}.png", name);
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All color balance effects applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
