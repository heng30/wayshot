use image::RgbaImage;

fn main() {
    let mut img = RgbaImage::new(800, 600);
    for y in 0..600 {
        for x in 0..800 {
            let r = (x * 255 / 800) as u8;
            let g = (y * 255 / 600) as u8;
            let b = ((x + y) * 255 / 1400) as u8;
            img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }
    img.save("data/test.png").unwrap();
    println!("Created data/test.png");
}
