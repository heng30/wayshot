use crate::{
    available_screens, backend,
    error::{Error, Result},
};
use screen_capture::{
    Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig, LogicalSize, Position,
};
use spin_sleep::SpinSleeper;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

// pub fn capture_output_stream(
//     config: CaptureStreamConfig,
//     mut cb: impl FnMut(CaptureStreamCallbackData),
// ) -> Result<CaptureStatus> {
// let streams = backend::request_screen_access(config.include_cursor)?;
// if streams.is_empty() {
//     return Err(crate::Error::NoCaptures);
// }
//
// let stream = streams
//     .iter()
//     .find(|s| s.screen_info.name == config.name)
//     .unwrap_or(&streams[0]);
//
// let mut capturer =
//     backend::PipeWireCapturer::new().map_err(|e| crate::Error::PipeWire(e.to_string()))?;
//
// capturer
//     .create_stream(stream.node_id, stream.width, stream.height)
//     .map_err(|e| crate::Error::PipeWire(e.to_string()))?;
//
// let mut index = 0;
// let interval_ms = config.fps.map(|v| 1000.0 / v);
// let sleeper = SpinSleeper::default();
// let start_time = std::time::Instant::now();
// let mut last_cleanup = std::time::Instant::now();
// const CLEANUP_INTERVAL: Duration = Duration::from_secs(5); // Clean every 5 seconds
//
// log::info!(
//     "Starting XDG Portal screen capture stream for screen: {} at {} FPS",
//     config.name,
//     config.fps.unwrap_or(30.0)
// );
//
// // Main capture loop
// loop {
//     // Check for cancellation signal
//     if config.cancel_sig.load(Ordering::Relaxed) {
//         log::info!("Exit capture iter process after Stopped");
//         return Ok(CaptureStatus::Stopped);
//     }
//
//     // Periodically clean up resources
//     if last_cleanup.elapsed() > CLEANUP_INTERVAL {
//         // Clean up any pending operations
//         last_cleanup = std::time::Instant::now();
//     }
//
//     // Perform the actual capture
//     let start = Instant::now();
//     let frame_data = capturer
//         .capture_frame(stream.node_id)
//         .map_err(|e| crate::Error::PipeWire(e.to_string()))?;
//
//     let capture = Capture {
//         width: stream.width,
//         height: stream.height,
//         pixel_data: frame_data,
//     };
//
//     cb(CaptureStreamCallbackData {
//         frame_index: index,
//         capture_time: start.elapsed(),
//         elapse: start_time.elapsed(),
//         data: capture,
//     });
//
//     // Maintain target frame rate if specified
//     if let Some(interval) = interval_ms {
//         let target_time =
//             start_time + Duration::from_millis((interval * (index + 1) as f64) as u64);
//         sleeper.sleep_until(target_time);
//     }
//
//     index += 1;
// }
// }

pub fn capture_mean_time(screen_name: &str, counts: u32) -> Result<Duration> {
    assert!(counts > 0);

    let screen_infos = available_screens().map_err(|e| Error::ScreenInfoError(e.to_string()))?;
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

    let start = Instant::now();
    for _ in 0..counts {
        // _ = capture_output(screen_name, true)?;
    }
    Ok(start.elapsed() / counts)
}
