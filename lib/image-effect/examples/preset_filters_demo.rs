/// Preset filters demo
/// Demonstrates all 15 preset filters from photon-rs

use image::ImageReader;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    // Load test image
    let img_path = Path::new("data/test.png");
    let img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    use image_effect::preset_filter_effect::{PresetFilterConfig, PresetFilter};

    let filters = [
        PresetFilter::Oceanic,
        PresetFilter::Islands,
        PresetFilter::Marine,
        PresetFilter::Seagreen,
        PresetFilter::Flagblue,
        PresetFilter::Liquid,
        PresetFilter::Diamante,
        PresetFilter::Radio,
        PresetFilter::Twenties,
        PresetFilter::Rosetint,
        PresetFilter::Mauve,
        PresetFilter::Bluechrome,
        PresetFilter::Vintage,
        PresetFilter::Perfume,
        PresetFilter::Serenity,
    ];

    for filter in filters {
        let mut test_img = img.clone();
        let effect = ImageEffect::PresetFilter(
            PresetFilterConfig::new().with_filter(filter)
        );

test_img = effect.apply(test_img).expect("Effect failed");

        let filename = format!("preset_{:?}.png", filter).to_lowercase();
        test_img.save(output_dir.join(&filename))?;
        println!("✓ Generated {}", filename);
    }

    println!("\n✓ All preset filters applied successfully!");
    println!("  Images saved to: tmp/");

    Ok(())
}
