use camera::{
    Rgba,
    image_composition::{
        MixPositionWithPadding, Shape, ShapeBase, ShapeCircle, ShapeRectangle, mix_images,
        mix_images_rgb,
    },
};
use image::{RgbImage, RgbaImage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // ===== RGBA Examples =====
    log::info!("=== RGBA Image Composition ===");

    log::info!("Creating RGBA background image...");
    let background_rgba = RgbaImage::from_fn(800, 600, |x, y| {
        let r = (x as f32 / 800.0 * 255.0) as u8;
        let b = (y as f32 / 600.0 * 255.0) as u8;
        image::Rgba([r, 100, b, 255])
    });

    log::info!("Creating RGBA simulated camera image...");
    let camera_image_rgba = RgbaImage::from_fn(640, 480, |x, y| {
        let g = ((x as f32 / 640.0) * 255.0) as u8;
        let b = ((y as f32 / 480.0) * 255.0) as u8;
        image::Rgba([255, g, b, 255])
    });

    log::info!("RGBA: Compositing with rectangle shape...");
    let rect = ShapeRectangle::default().with_size((300, 225)).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::TopLeft((240, 180)))
            .with_border_width(10)
            .with_border_color(Rgba([255, 255, 255, 255])),
    );
    let result_rect = mix_images(
        background_rgba.clone(),
        camera_image_rgba.clone(),
        None,
        Shape::Rectangle(rect),
    )?;
    result_rect.save("tmp/composition_rectangle_rgba.png")?;
    log::info!("Saved RGBA output to: tmp/composition_rectangle_rgba.png");

    log::info!("RGBA: Compositing with circle shape...");
    let circle = ShapeCircle::default().with_radius(150).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::TopRight((210, 90)))
            .with_border_width(8)
            .with_border_color(Rgba([255, 255, 255, 255])),
    );
    let result_circle = mix_images(
        background_rgba.clone(),
        camera_image_rgba.clone(),
        None,
        Shape::Circle(circle),
    )?;
    result_circle.save("tmp/composition_circle_rgba.png")?;
    log::info!("Saved RGBA output to: tmp/composition_circle_rgba.png");

    // ===== RGB Examples =====
    log::info!("");
    log::info!("=== RGB Image Composition ===");

    log::info!("Creating RGB background image...");
    let background_rgb = RgbImage::from_fn(800, 600, |x, y| {
        let r = (x as f32 / 800.0 * 255.0) as u8;
        let b = (y as f32 / 600.0 * 255.0) as u8;
        image::Rgb([r, 100, b])
    });

    log::info!("Creating RGB simulated camera image...");
    let camera_image_rgb = RgbImage::from_fn(640, 480, |x, y| {
        let g = ((x as f32 / 640.0) * 255.0) as u8;
        let b = ((y as f32 / 480.0) * 255.0) as u8;
        image::Rgb([255, g, b])
    });

    log::info!("RGB: Compositing with rectangle shape...");
    let rect_rgb = ShapeRectangle::default().with_size((300, 225)).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::TopLeft((240, 180)))
            .with_border_width(10)
            .with_border_color(Rgba([255, 255, 255, 255])),
    );
    let result_rect_rgb = mix_images_rgb(
        background_rgb.clone(),
        camera_image_rgb.clone(),
        None,
        Shape::Rectangle(rect_rgb),
    )?;
    result_rect_rgb.save("tmp/composition_rectangle_rgb.png")?;
    log::info!("Saved RGB output to: tmp/composition_rectangle_rgb.png");

    log::info!("RGB: Compositing with circle shape...");
    let circle_rgb = ShapeCircle::default().with_radius(150).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::TopRight((210, 90)))
            .with_border_width(8)
            .with_border_color(Rgba([255, 255, 255, 255])),
    );
    let result_circle_rgb = mix_images_rgb(
        background_rgb.clone(),
        camera_image_rgb.clone(),
        None,
        Shape::Circle(circle_rgb),
    )?;
    result_circle_rgb.save("tmp/composition_circle_rgb.png")?;
    log::info!("Saved RGB output to: tmp/composition_circle_rgb.png");

    log::info!("RGB: Compositing with plain rectangle (no border)...");
    let plain_rect_rgb = ShapeRectangle::default().with_size((250, 200)).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::TopLeft((400, 300)))
            .with_border_width(0)
            .with_border_color(Rgba([0, 0, 0, 0])),
    );
    let result_plain_rgb = mix_images_rgb(
        background_rgb.clone(),
        camera_image_rgb.clone(),
        None,
        Shape::Rectangle(plain_rect_rgb),
    )?;
    result_plain_rgb.save("tmp/composition_rect_plain_rgb.png")?;
    log::info!("Saved RGB output to: tmp/composition_rect_plain_rgb.png");

    log::info!("RGB: Compositing with plain circle (no border)...");
    let plain_circle_rgb = ShapeCircle::default().with_radius(100).with_base(
        ShapeBase::default()
            .with_pos(MixPositionWithPadding::TopLeft((200, 300)))
            .with_border_width(0)
            .with_border_color(Rgba([0, 0, 0, 0])),
    );
    let result_plain_rgb = mix_images_rgb(
        background_rgb.clone(),
        camera_image_rgb.clone(),
        None,
        Shape::Circle(plain_circle_rgb),
    )?;
    result_plain_rgb.save("tmp/composition_circle_plain_rgb.png")?;
    log::info!("Saved RGB output to: tmp/composition_circle_plain_rgb.png");

    log::info!("");
    log::info!("All RGBA and RGB examples completed successfully!");
    Ok(())
}
