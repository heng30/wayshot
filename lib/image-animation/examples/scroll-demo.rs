use image::{DynamicImage, Rgba, RgbaImage};
use image_animation::scroll::ImageScrollConfig;

fn main() {
    env_logger::init();

    // Generate test image dynamically
    let test_image = generate_test_image(1920, 5000);
    let temp_path = std::env::temp_dir().join("image_scroll_test.png");
    test_image
        .save(&temp_path)
        .expect("Failed to save test image");

    std::fs::create_dir_all("output").expect("failed to create output directory");

    // Create scroll config and record
    let mut scroll = ImageScrollConfig::new(temp_path.clone())
        .with_output_height(1080)
        .with_fps(25)
        .with_start_pause(2.0)
        .with_end_pause(3.0)
        .with_scroll_speed(0.2);

    scroll
        .record("output/scroll_animation.mp4")
        .expect("Recording failed");

    println!("Animation saved to output/scroll_animation.mp4");

    // Cleanup
    std::fs::remove_file(&temp_path).ok();
}

/// Generate a tall test image dynamically with gradient stripes
fn generate_test_image(width: u32, height: u32) -> DynamicImage {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        let r = (y % 256) as u8;
        let g = ((y * 2) % 256) as u8;
        let b = ((y * 3) % 256) as u8;
        for x in 0..width {
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }
    DynamicImage::from(img)
}

