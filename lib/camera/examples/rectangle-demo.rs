use anyhow::Result;
use camera::{
    Rgba,
    image_composition::{Shape, ShapeBase, ShapeRectangle, MixPositionWithPadding, mix_images},
};
use image::RgbaImage;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Testing zoom functionality...\n");

    let test_image = RgbaImage::from_fn(800, 600, |x, y| {
        let r = ((x as f32) / 800.0 * 255.0) as u8;
        let g = ((y as f32) / 600.0 * 255.0) as u8;
        let b = 128;
        image::Rgba([r, g, b, 255])
    });

    log::info!("1. Creating test with no zoom (1.0x)...");
    let bg1 = RgbaImage::from_fn(400, 300, |_, _| image::Rgba([240, 240, 240, 255]));
    let rect1 = ShapeRectangle::default().with_size((100, 50)).with_base(
        ShapeBase::default()
            .with_border_width(3)
            .with_pos(MixPositionWithPadding::TopLeft((40, 30)))
            .with_zoom(2.0)
            .with_border_color(Rgba([255, 0, 0, 255]))
            .with_clip_pos((0.1, 0.1)),
    );
    let result1 = mix_images(bg1, test_image.clone(), Shape::Rectangle(rect1))?;
    result1.save("tmp/rect_zoom_1.0x.png")?;
    log::info!("   ✓ Saved zoom_1.0x.png (original size)");

    log::info!("2. Creating test with 2x zoom...");
    let bg2 = RgbaImage::from_fn(400, 300, |_, _| image::Rgba([240, 240, 240, 255]));
    let rect2 = ShapeRectangle::default().with_size((100, 50)).with_base(
        ShapeBase::default()
            .with_border_width(3)
            .with_pos(MixPositionWithPadding::TopLeft((240, 180)))
            .with_zoom(2.0)
            .with_border_color(Rgba([255, 0, 0, 255]))
            .with_clip_pos((0.25, 0.25)),
    );
    let result2 = mix_images(bg2, test_image.clone(), Shape::Rectangle(rect2))?;
    result2.save("tmp/rect_zoom_2.0x.png")?;
    log::info!("   ✓ Saved zoom_2.0x.png (center cropped, 2x magnified)");

    log::info!("3. Creating test with 0.5x zoom (shrink)...");
    let bg3 = RgbaImage::from_fn(400, 300, |_, _| image::Rgba([240, 240, 240, 255]));
    let rect3 = ShapeRectangle::default().with_size((100, 50)).with_base(
        ShapeBase::default()
            .with_border_width(3)
            .with_zoom(0.5)
            .with_border_color(Rgba([255, 0, 0, 255]))
            .with_clip_pos((0.30, 0.30)),
    );
    let result3 = mix_images(bg3, test_image.clone(), Shape::Rectangle(rect3))?;
    result3.save("tmp/rect_zoom_0.5x.png")?;
    log::info!("   ✓ Saved zoom_0.5x.png (shrunk with black padding)");

    log::info!("");
    log::info!("✓ All zoom tests completed!");

    Ok(())
}
