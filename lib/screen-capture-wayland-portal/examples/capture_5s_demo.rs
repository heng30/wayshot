use screen_capture::{CaptureStreamConfig, ScreenCapture};
use screen_capture_wayland_portal::{ScreenCaptureWaylandPortal, available_screens};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let capturer = ScreenCaptureWaylandPortal::default();

    let screens = available_screens()?;
    assert!(!screens.is_empty());

    let config = CaptureStreamConfig {
        name: screens[0].name.clone(),
        include_cursor: true,
        fps: Some(25.0),
        cancel_sig: Arc::new(AtomicBool::new(false)),
    };

    let stop_sig = config.cancel_sig.clone();

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        log::info!("5 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    let mut total_frames = 0;
    capturer.capture_output_stream(config, |data| {
        log::debug!("{:?}", data.frame_index);
        total_frames = data.frame_index;
    })?;

    println!("average fps: {}", total_frames / 5);

    Ok(())
}
