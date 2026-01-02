use camera::camera_info::{query_available_cameras, query_camera_id};
use camera::{
    CameraResult,
    camera_client::{CameraClient, CameraConfig, PixelFormat},
};
use std::{thread, time::Duration};

fn main() -> CameraResult<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    camera::init();

    let fps = 25;
    let cameras = query_available_cameras();
    if cameras.is_empty() {
        log::warn!("No working cameras found!");
        return Ok(());
    }

    log::info!(
        "Found {} camera(s), using first camera: {}",
        cameras.len(),
        cameras[0].name
    );

    let config = CameraConfig::default()
        .with_pixel_format(PixelFormat::RGBA)
        .with_width(1280)
        .with_height(720)
        .with_fps(fps);

    let camera_id = query_camera_id(&cameras[0].name)?;
    let mut client = CameraClient::new(camera_id, config)?;

    log::info!("Starting camera capture...");

    let mut frame_count = 0;
    let mut empty_frame_count = 0;
    client.start()?;

    for _ in 0..100 {
        thread::sleep(Duration::from_millis(1000 / fps as u64));

        match client.last_frame() {
            Ok(frame) => {
                if frame.is_empty() {
                    empty_frame_count += 1;
                } else {
                    if frame_count % 10 == 0 {
                        log::info!(
                            "Frame #{}: {}x{} ({} pixels)",
                            frame_count,
                            frame.width(),
                            frame.height(),
                            frame.len()
                        );

                        frame.save(format!("tmp/camera-{frame_count}.png"))?;
                    }
                }
                frame_count += 1;
            }
            Err(e) => log::warn!("{e}"),
        }
    }

    log::info!("Stopping camera...");
    client.stop()?;

    log::info!(
        "Captured {frame_count} frames, {empty_frame_count} ({}%) empty frames",
        empty_frame_count * 100 / frame_count
    );
    Ok(())
}
