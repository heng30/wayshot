use crate::{Error, available_screens, backend};
use screen_capture::{Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig};
use spin_sleep::SpinSleeper;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub fn capture_output_stream(
    config: CaptureStreamConfig,
    mut cb: impl FnMut(CaptureStreamCallbackData),
) -> Result<CaptureStatus, Error> {
    let mut manager = backend::DXGIManager::new(config.name.to_string())?
        .with_include_cursor(config.include_cursor);

    let mut index = 0;
    let interval_ms = config.fps.map(|v| 1000.0 / v);
    let sleeper = SpinSleeper::default();
    let start_time = std::time::Instant::now();

    config.sync_sig.store(true, Ordering::Relaxed);

    loop {
        if config.cancel_sig.load(Ordering::Relaxed) {
            log::info!("Exit capture iter process after Stopped");
            return Ok(CaptureStatus::Stopped);
        }

        let capture_start = Instant::now();
        let (frame, (width, height)) = manager.capture_frame_rgba()?;

        let capture = Capture {
            width: width as u32,
            height: height as u32,
            pixel_data: frame,
        };

        cb(CaptureStreamCallbackData {
            frame_index: index,
            capture_time: capture_start.elapsed(),
            elapse: start_time.elapsed(),
            data: capture,
        });

        if let Some(interval) = interval_ms {
            let target_time =
                start_time + Duration::from_millis((interval * (index + 1) as f64) as u64);
            sleeper.sleep_until(target_time);
        }

        index += 1;
    }
}

pub fn capture_mean_time(screen_name: &str, counts: u32) -> Result<Option<Duration>, Error> {
    assert!(counts > 0);

    let screen_infos = available_screens()?;
    if screen_infos.is_empty() {
        return Err(crate::Error::NoOutput(
            "available screen no found".to_string(),
        ));
    }

    if !screen_infos.iter().any(|item| item.name == screen_name) {
        return Err(crate::Error::NoOutput(format!(
            "{screen_name} is not in available screen list"
        )));
    }

    let mut manager = backend::DXGIManager::new(screen_name.to_string())?;

    let start = Instant::now();
    for _ in 0..counts {
        _ = manager.capture_frame_rgba()
    }
    Ok(Some(start.elapsed() / counts))
}
