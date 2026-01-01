use anyhow::Result;
use camera::{
    Rgba,
    image_composition::{Shape, ShapeBase, ShapeCircle, mix_images},
};
use image::RgbaImage;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Testing circle zoom functionality...\n");

    let test_image = RgbaImage::from_fn(800, 600, |x, y| {
        let r = ((x as f32) / 800.0 * 255.0) as u8;
        let g = ((y as f32) / 600.0 * 255.0) as u8;

        let (center_x, center_y) = (400, 300);
        let dx = x as i32 - center_x;
        let dy = y as i32 - center_y;
        let dist = ((dx * dx + dy * dy) as f32).sqrt();
        let b = if dist < 50.0 { 255 } else { 128 };
        image::Rgba([r, g, b, 255])
    });

    log::info!("1. Creating test with no zoom (1.0x)...");
    let bg1 = RgbaImage::from_fn(400, 300, |_, _| image::Rgba([240, 240, 240, 255]));
    let circle1 = ShapeCircle::default().with_radius(100).with_base(
        ShapeBase::default()
            .with_border_width(3)
            .with_pos((0.1, 0.1))
            .with_zoom(1.0)
            .with_border_color(Rgba([255, 0, 0, 255]))
            .with_clip_pos((0.5, 0.5)),
    );

    let result1 = mix_images(bg1, test_image.clone(), Shape::Circle(circle1))?;
    result1.save("tmp/circle_zoom_1.0x.png")?;
    log::info!("   ✓ Saved circle_zoom_1.0x.png");

    log::info!("2. Creating test with 2x zoom...");
    let bg2 = RgbaImage::from_fn(400, 300, |_, _| image::Rgba([240, 240, 240, 255]));

    let circle2 = ShapeCircle::default().with_radius(100).with_base(
        ShapeBase::default()
            .with_border_width(3)
            .with_zoom(2.0)
            .with_clip_pos((0.2, 0.3)),
    );

    let result2 = mix_images(bg2, test_image.clone(), Shape::Circle(circle2))?;
    result2.save("tmp/circle_zoom_2.0x.png")?;
    log::info!("   ✓ Saved circle_zoom_2.0x.png");

    log::info!("3. Creating test with 0.5x zoom...");
    let bg3 = RgbaImage::from_fn(400, 300, |_, _| image::Rgba([240, 240, 240, 255]));
    let circle3 = ShapeCircle::default()
        .with_radius(100)
        .with_base(ShapeBase::default().with_border_width(3).with_zoom(0.5));
    let result3 = mix_images(bg3, test_image.clone(), Shape::Circle(circle3))?;
    result3.save("tmp/circle_zoom_0.5x.png")?;
    log::info!("   ✓ Saved circle_zoom_0.5x.png");

    log::info!("");
    log::info!("✓ All circle zoom tests completed!");

    Ok(())
}
