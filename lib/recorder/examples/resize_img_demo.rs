use capture::Capture;
use recorder::RecordingSession;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let img = image::open("target/screenshot-all.png")?;
    let data = Capture {
        width: img.width(),
        height: img.height(),
        pixel_data: img.into_bytes(),
    };

    let now = std::time::Instant::now();
    let resized_img = RecordingSession::resize_image(data, (1920, 1080))?;
    log::debug!("resize image time: {:.2?}", now.elapsed());

    let path = "target/resize-test.png";
    log::debug!("save path: {}", path);

    resized_img.save(path)?;

    Ok(())
}
