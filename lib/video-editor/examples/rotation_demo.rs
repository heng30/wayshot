// Example demonstrating the rotation global filter
// Generates test images with different rotation angles and saves them to tmp/ directory

use image::{Rgba, RgbaImage};
use std::{fs, time::Duration};
use video_editor::{
    Result,
    filters::{
        global::RotationGlobalFilter,
        traits::{GlobalFilter, GlobalFilterData},
    },
};

fn create_test_image(width: u32, height: u32) -> RgbaImage {
    // Create a colorful test image with distinct patterns to show rotation
    RgbaImage::from_fn(width, height, |x, y| {
        let x_f = x as f32 / width as f32;
        let y_f = y as f32 / height as f32;

        // Create a gradient with diagonal stripes
        let stripe = ((x_f * 10.0 + y_f * 10.0) as i32 % 2) == 0;

        // Add a colored circle in the center
        let cx = (x as f32 - width as f32 / 2.0).powi(2);
        let cy = (y as f32 - height as f32 / 2.0).powi(2);
        let in_circle = (cx + cy).sqrt() < width as f32 * 0.2;

        if in_circle {
            Rgba([255, 100, 100, 255]) // Red circle in center
        } else if stripe {
            // Blue gradient stripes
            let b = (x_f * 255.0) as u8;
            Rgba([50, 100, b, 255])
        } else {
            // Green gradient stripes
            let g = (y_f * 255.0) as u8;
            Rgba([50, g, 100, 255])
        }
    })
}

fn apply_rotation(
    filter: &RotationGlobalFilter,
    image: &RgbaImage,
    timeline_offset: Duration,
    total_duration: Duration,
) -> Result<RgbaImage> {
    let mut data = GlobalFilterData {
        image: image.clone(),
        timeline_offset,
        total_duration,
    };

    filter.apply(&mut data)?;
    Ok(data.image)
}

fn save_image(image: &RgbaImage, path: &str) -> Result<()> {
    image
        .save(path)
        .map_err(|e| video_editor::Error::InvalidConfig(format!("Failed to save image: {}", e)))
}

fn main() -> Result<()> {
    println!("Rotation Global Filter Demo");
    println!("============================\n");

    // Create tmp directory
    let tmp_dir = "tmp/rotation_demo";
    fs::create_dir_all(tmp_dir)?;

    // Test with small image first for visual verification, then 1080p
    let (width, height) = (1920, 1080);
    println!("Creating test image: {}x{}", width, height);
    let test_image = create_test_image(width, height);

    // Save original
    let original_path = format!("{}/original.png", tmp_dir);
    save_image(&test_image, &original_path)?;
    println!("Saved original: {}", original_path);

    let total_duration = Duration::from_secs(10);

    // Test each rotation angle
    let angles = [0.0, 90.0, 180.0, -90.0];

    for &angle in &angles {
        let filter = RotationGlobalFilter::new(angle);
        let result = apply_rotation(&filter, &test_image, Duration::from_secs(0), total_duration)?;

        let filename = format!("{}/rotation_{}.png", tmp_dir, angle as i32);
        save_image(&result, &filename)?;

        println!(
            "  Rotation {}°: input {}x{} -> output {}x{} (scaled to {}x{})",
            angle,
            width,
            height,
            if angle.abs() == 90.0 { height } else { width },
            if angle.abs() == 90.0 { width } else { height },
            result.width(),
            result.height()
        );
    }

    // Test with portrait orientation (1080x1920)
    println!("\nTesting portrait orientation (1080x1920):");
    let (p_width, p_height) = (1080, 1920);
    let portrait_image = create_test_image(p_width, p_height);

    let portrait_dir = format!("{}/portrait", tmp_dir);
    fs::create_dir_all(&portrait_dir)?;

    let portrait_original = format!("{}/original.png", portrait_dir);
    save_image(&portrait_image, &portrait_original)?;

    for &angle in &angles {
        let filter = RotationGlobalFilter::new(angle);
        let result = apply_rotation(&filter, &portrait_image, Duration::from_secs(0), total_duration)?;

        let filename = format!("{}/rotation_{}.png", portrait_dir, angle as i32);
        save_image(&result, &filename)?;

        println!(
            "  Rotation {}°: {}x{} -> {}x{}",
            angle,
            p_width,
            p_height,
            result.width(),
            result.height()
        );
    }

    println!("\n============================");
    println!("Demo completed successfully!");
    println!("\nImages saved to: {}/", tmp_dir);
    println!("\nKey behaviors:");
    println!("- 0°: No change");
    println!("- 90°: Rotated clockwise, scaled to fit (contain mode)");
    println!("- 180°: Rotated upside down, same dimensions");
    println!("- -90°: Rotated counter-clockwise, scaled to fit");
    println!("\nFor 1920x1080 rotated 90°:");
    println!("  - Rotated image is 1080x1920");
    println!("  - Scaled to 608x1080 (contain in 1920x1080 canvas)");
    println!("  - Centered with black bars on left/right");

    Ok(())
}
