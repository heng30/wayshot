use anyhow::Result;
use camera::{
    ShapeCircle,
    image_composition::{MixPositionWithPadding, Shape, ShapeBase, mix_images_rgb},
};
use image::RgbImage;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("=== Background Removal Demo (RGB) ===");

    log::info!("Loading camera image and mask...");
    let camera_image = image::open("./examples/test.png")
        .map_err(|e| anyhow::anyhow!("Failed to load camera image: {}", e))?
        .to_rgb8();

    let mask = image::open("./examples/mask.png")
        .map_err(|e| anyhow::anyhow!("Failed to load mask: {}", e))?
        .to_luma8();

    log::info!(
        "  ✓ Loaded: {}x{}",
        camera_image.width(),
        camera_image.height()
    );

    let background = RgbImage::from_fn(1920, 1080, |x, y| {
        let r = (x as f32 / 1920.0 * 180.0) as u8 + 40;
        let g = (y as f32 / 1080.0 * 180.0) as u8 + 40;
        let b = 150;
        image::Rgb([r, g, b])
    });

    log::info!("1. Rectangle with background removal (no zoom)");
    let circle_no_zoom = ShapeCircle::default().with_radius(250).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::BottomRight((80, 80)))
            .with_zoom(1.0)
            .with_clip_pos((0.3, 0.2)),
    );

    let result_no_zoom = mix_images_rgb(
        background.clone(),
        camera_image.clone(),
        Some(mask.clone()),
        Shape::Circle(circle_no_zoom),
    )?;
    result_no_zoom.save("tmp/bg_removal_no_zoom.png")?;
    log::info!("  ✓ Saved: tmp/bg_removal_no_zoom.png");

    log::info!("2. Rectangle with background removal (1.5x zoom)");
    let circle_zoom = ShapeCircle::default().with_radius(250).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::BottomRight((80, 80)))
            .with_zoom(1.5)
            .with_clip_pos((0.3, 0.2)),
    );

    let result_zoom = mix_images_rgb(
        background.clone(),
        camera_image.clone(),
        Some(mask.clone()),
        Shape::Circle(circle_zoom),
    )?;
    result_zoom.save("tmp/bg_removal_zoom.png")?;
    log::info!("  ✓ Saved: tmp/bg_removal_zoom.png");

    log::info!("=== All tests completed! ===");
    log::info!("Generated files:");
    log::info!("  - tmp/bg_removal_no_zoom.png  (No zoom, with mask)");
    log::info!("  - tmp/bg_removal_zoom.png     (1.5x zoom, with mask)");

    Ok(())
}
