use camera::{
    Rgba,
    image_composition::{Shape, ShapeBase, ShapeCircle, ShapeRectangle, mix_images},
};
use image::RgbaImage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Creating background image...");
    let background = RgbaImage::from_fn(800, 600, |x, y| {
        let r = (x as f32 / 800.0 * 255.0) as u8;
        let b = (y as f32 / 600.0 * 255.0) as u8;
        image::Rgba([r, 100, b, 255])
    });

    log::info!("Creating simulated camera image...");
    let camera_image = RgbaImage::from_fn(640, 480, |x, y| {
        let g = ((x as f32 / 640.0) * 255.0) as u8;
        let b = ((y as f32 / 480.0) * 255.0) as u8;
        image::Rgba([255, g, b, 255])
    });

    log::info!("Compositing with rectangle shape...");
    let rect = ShapeRectangle::default().with_size((300, 225)).with_base(
        ShapeBase::default()
            .with_pos((0.3, 0.3))
            .with_border_width(10)
            .with_border_color(Rgba([255, 255, 255, 255])),
    );
    let result_rect = mix_images(
        background.clone(),
        camera_image.clone(),
        Shape::Rectangle(rect),
    )?;
    result_rect.save("tmp/compositing_rectangle.png")?;
    log::info!("Saved output to: tmp/compositing_rectangle.png");

    log::info!("Compositing with circle shape...");
    let circle = ShapeCircle::default().with_radius(150).with_base(
        ShapeBase::default()
            .with_pos((0.7, 0.6))
            .with_border_width(8)
            .with_border_color(Rgba([255, 255, 255, 255])),
    );
    let result_circle = mix_images(
        background.clone(),
        camera_image.clone(),
        Shape::Circle(circle),
    )?;
    result_circle.save("tmp/composition_circle.png")?;
    log::info!("Saved output to: tmp/composition_circle.png");

    log::info!("Compositing with plain rectangle...");
    let plain_rect = ShapeRectangle::default().with_size((250, 200)).with_base(
        ShapeBase::default()
            .with_pos((0.5, 0.5))
            .with_border_width(0)
            .with_border_color(Rgba([0, 0, 0, 0])),
    );
    let result_plain = mix_images(
        background.clone(),
        camera_image.clone(),
        Shape::Rectangle(plain_rect),
    )?;
    result_plain.save("tmp/composition_rect_pain.png")?;
    log::info!("Saved output to: tmp/composition_rect_pain.png");

    log::info!("Compositing with plain circle...");
    let circle = ShapeCircle::default().with_radius(150).with_base(
        ShapeBase::default()
            .with_border_width(0)
            .with_border_color(Rgba([0, 0, 0, 0])),
    );
    let result_circle = mix_images(
        background.clone(),
        camera_image.clone(),
        Shape::Circle(circle),
    )?;
    result_circle.save("tmp/composition_plain_circle.png")?;
    log::info!("Saved output to: tmp/composition_plain_circle.png");

    log::info!("");
    log::info!("All examples completed successfully!");
    Ok(())
}
