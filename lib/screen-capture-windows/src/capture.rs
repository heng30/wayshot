use crate::{Error, backend};
use crossbeam::channel::bounded;
use screen_capture::{Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig};
use spin_sleep::SpinSleeper;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

// backend only support one instance
pub fn capture_output_stream(
    config: CaptureStreamConfig,
    mut cb: impl FnMut(CaptureStreamCallbackData),
) -> Result<CaptureStatus, Error> {
    let (sender, receiver) = bounded(128);
    let stop_sig = config.cancel_sig.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps.unwrap_or(25.0));

    config.sync_sig.store(true, Ordering::Relaxed);

    std::thread::spawn(move || {
        let mut index = 0;
        let sleeper = SpinSleeper::default();
        let start_time = std::time::Instant::now();

        let mut manager = match backend::DXGIManager::new(config.name.to_string()) {
            Ok(m) => m.with_include_cursor(config.include_cursor),

            Err(e) => {
                log::warn!("New DXGIManager failed: {e}");
                config.cancel_sig.store(true, Ordering::Relaxed);
                return;
            }
        };

        loop {
            if config.cancel_sig.load(Ordering::Relaxed) {
                log::info!("Exit capture iter process after Stopped (sender)");
                break;
            }

            let capture_now = Instant::now();
            match manager.capture_frame_rgba() {
                Ok(item) => {
                    if let Err(e) = sender.try_send((capture_now.elapsed(), item)) {
                        log::warn!("capture try send frame failed: {e}");
                    }
                }
                Err(e) => {
                    log::warn!("capture frame failed: {e}");
                }
            }

            index += 1;
            let next_frame_time = start_time + frame_interval * index as u32;
            sleeper.sleep_until(next_frame_time);
        }
    });

    let mut index = 0;
    let mut last_frame = None;
    let sleeper = SpinSleeper::default();
    let mut start_time = std::time::Instant::now();

    while !stop_sig.load(Ordering::Relaxed) {
        let mut capture_time = None;

        while let Ok((time, frame)) = receiver.try_recv() {
            if last_frame.is_none() {
                start_time = Instant::now();
            }

            capture_time = Some(time);
            last_frame = Some(frame);
        }

        if last_frame.is_none() {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }

        let capture = Capture {
            width: last_frame.as_ref().unwrap().1.0 as u32,
            height: last_frame.as_ref().unwrap().1.1 as u32,
            pixel_data: last_frame.clone().unwrap().0,
        };

        cb(CaptureStreamCallbackData {
            frame_index: index,
            capture_time: capture_time.unwrap_or_default(),
            elapse: start_time.elapsed(),
            data: capture,
        });

        index += 1;
        let next_frame_time = start_time + frame_interval * index as u32;
        sleeper.sleep_until(next_frame_time);
    }

    log::info!("exit capture receiver thread...");
    Ok(CaptureStatus::Finished)
}

pub fn capture_mean_time(_screen_name: &str, _counts: u32) -> Result<Option<Duration>, Error> {
    Ok(None)
}
