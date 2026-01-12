use image::ImageReader;
use image_effect::filter::SnowNoiseConfig;
use image_effect::{Effect, ImageEffect};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("tmp");
    std::fs::create_dir_all(output_dir)?;

    let img_path = Path::new("data/test.png");
    let mut img = ImageReader::open(img_path)?.decode()?.to_rgba8();

    let effect = ImageEffect::SnowNoise(SnowNoiseConfig::new());
    img = effect.apply(img).expect("Effect failed");
    img.save(output_dir.join("snow_noise_effect.png"))?;

    println!("✓ Snow noise effect applied successfully!");
    println!("  Effect:   tmp/snow_noise_effect.png");

    let mut img2 = ImageReader::open(img_path)?.decode()?.to_rgba8();
    let effect2 = ImageEffect::SnowNoise(
        SnowNoiseConfig::new()
            .with_intensity(0.005)
            .with_min_brightness(50),
    );
    img2 = effect2.apply(img2).expect("Effect failed");
    img2.save(output_dir.join("snow_noise_heavy.png"))?;
    println!("  Heavy effect: tmp/snow_noise_heavy.png");

    // Apply without grayscale
    let mut img3 = ImageReader::open(img_path)?.decode()?.to_rgba8();
    let effect3 = ImageEffect::SnowNoise(
        SnowNoiseConfig::new()
            .with_intensity(0.005)
            .with_grayscale(false),
    );
    img3 = effect3.apply(img3).expect("Effect failed");
    img3.save(output_dir.join("snow_noise_color.png"))?;
    println!("  Color effect: tmp/snow_noise_color.png");

    Ok(())
}
