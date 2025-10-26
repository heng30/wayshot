use recorder::{FPS, VideoEncoder};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let img_path = PathBuf::from(format!("/tmp/screenshot.png"));

    if !img_path.exists() {
        log::warn!("Image not found: {}", img_path.display());
        return Ok(());
    }

    let img = image::open(&img_path)?;
    log::debug!("Loaded image {}x{}", img.width(), img.height());

    let mut encoder = VideoEncoder::new(img.width(), img.height(), FPS::Fps30)?;
    let now = std::time::Instant::now();
    encoder.encode_frame(img.into())?;
    log::info!("MP4 encoding time: {:.2?}", now.elapsed());

    Ok(())
}
