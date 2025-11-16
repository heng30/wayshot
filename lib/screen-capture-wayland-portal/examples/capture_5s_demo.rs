use screen_capture::{CaptureStreamConfig, ScreenCapture};
use screen_capture_wayland_portal::{ScreenCaptureWaylandPortal, available_screens};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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

    let mut total_frames = 0;
    let total_frames_mut = &mut total_frames;
    let stop_sig = config.cancel_sig.clone();

    capturer.capture_output_stream(config, move |data| {
        // log::debug!("{:?} - {:.2?}", data.frame_index, data.elapse);
        *total_frames_mut = data.frame_index;

        if data.elapse > Duration::from_secs(5) {
            log::info!("5 seconds elapsed, stopping recording...");
            stop_sig.store(true, Ordering::Relaxed);
        }
    })?;

    println!("average fps: {}", total_frames / 5);

    Ok(())
}
