// use crate::{
//     backend,
//     error::{Error, Result},
// };
// use screen_capture::{
//     Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig, LogicalSize, Position,
// };
// use spin_sleep::SpinSleeper;
// use std::sync::atomic::Ordering;
// use std::thread;
use std::time::{Duration, Instant};

// pub fn capture_screenshot_async(_screen_name: &str) -> Result<Capture> {
//     thread::spawn(move || capture_all_outputs_sync())
//         .join()
//         .map_err(|e| Error::ThreadJoin(format!("{:?}", e)))?
// }
//
// pub fn capture_output(name: &str, include_cursor: bool) -> Result<Capture> {
//     // Note: cursor embedding is not directly supported without portal
//     // We'll capture the screen regardless of cursor setting
//     if include_cursor {
//         log::info!("Cursor embedding requested, but not supported without portal");
//     }
//
//     let name_owned = name.to_string();
//     let (tx, rx) = std::sync::mpsc::channel();
//
//     thread::spawn(move || {
//         let result = request_screen_access_sync(0, 0); // cursor_mode, source_types simplified
//
//         match result {
//             Ok(streams) => {
//                 if streams.is_empty() {
//                     let _ = tx.send(Err(Error::Other("No screens available".to_string())));
//                 } else {
//                     // Try to find the requested screen, otherwise use the first available one
//                     let stream = streams
//                         .iter()
//                         .find(|s| s.screen_info.name == name_owned)
//                         .unwrap_or(&streams[0]);
//                     let capture_result = capture_single_frame_sync(stream);
//                     let _ = tx.send(capture_result);
//                 }
//             }
//             Err(e) => {
//                 let _ = tx.send(Err(Error::Other(format!(
//                     "Failed to request screen access: {}",
//                     e
//                 ))));
//             }
//         }
//     });
//
//     rx.recv()
//         .map_err(|e| Error::Other(format!("Channel receive failed: {}", e)))?
// }
//
pub fn capture_mean_time(screen_name: &str, counts: u32) -> crate::error::Result<Duration> {
    assert!(counts > 0);

    let start = Instant::now();
    for _ in 0..counts {
        // _ = capture_output(screen_name, true)?;
    }
    Ok(start.elapsed() / counts)
}
//
// pub fn capture_output_stream(
//     config: CaptureStreamConfig,
//     cb: impl FnMut(CaptureStreamCallbackData) + Send + 'static,
// ) -> crate::error::Result<CaptureStatus> {
//     let (tx, rx) = std::sync::mpsc::channel();
//
//     thread::spawn(move || {
//         let result = streaming_capture_sync(config, cb);
//         let _ = tx.send(result);
//     });
//
//     rx.recv()
//         .map_err(|e| Error::Other(format!("Channel receive failed: {}", e)))?
// }
//
// pub fn capture_all_outputs_sync() -> Result<Capture> {
//     let streams =
//         request_screen_access_sync(0, 0) // simplified parameters
//             .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;
//
//     if streams.is_empty() {
//         return Err(Error::Other("No screens available".to_string()));
//     }
//
//     // Capture from the first screen
//     let stream = &streams[0];
//     capture_single_frame_sync(stream)
// }
//
// pub fn capture_output_by_name(name: &str) -> Result<Capture> {
//     let streams =
//         request_screen_access_sync(0, 0) // simplified parameters
//             .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;
//
//     let stream = if streams.is_empty() {
//         return Err(Error::Other("No screens available".to_string()));
//     } else {
//         // Try to find the requested screen, otherwise use the first available one
//         streams
//             .iter()
//             .find(|s| s.screen_info.name == name)
//             .unwrap_or(&streams[0])
//     };
//
//     capture_single_frame_sync(stream)
// }
//
// pub fn capture_region(
//     name: &str,
//     _region_position: Position,
//     _region_size: LogicalSize,
// ) -> Result<Capture> {
//     // Region capture not directly supported without portal
//     // We'll capture the full screen and let the caller crop if needed
//     capture_output_by_name(name)
// }
//
// fn capture_single_frame_sync(stream: &StreamHandle) -> Result<Capture> {
//     log::info!(
//         "Capturing frame from screen: {} (node_id: {}, size: {}x{})",
//         stream.screen_info.name,
//         stream.node_id,
//         stream.width,
//         stream.height
//     );
//
//     // Create PipeWire capturer
//     let mut capturer = PipeWireCapturer::new()
//         .map_err(|e| Error::Other(format!("Failed to create PipeWire capturer: {}", e)))?;
//
//     // Create PipeWire stream
//     capturer
//         .create_stream(stream.node_id, stream.width, stream.height)
//         .map_err(|e| Error::Other(format!("Failed to create PipeWire stream: {}", e)))?;
//
//     // Capture frame data (already in RGBA format)
//     let frame_data = capturer
//         .capture_frame(stream.node_id)
//         .map_err(|e| Error::Other(format!("Failed to capture frame: {}", e)))?;
//
//     log::info!(
//         "Successfully captured frame: {}x{}, {} bytes",
//         stream.width,
//         stream.height,
//         frame_data.len()
//     );
//
//     Ok(Capture {
//         width: stream.width,
//         height: stream.height,
//         pixel_data: frame_data,
//     })
// }
//
// fn streaming_capture_sync(
//     config: CaptureStreamConfig,
//     mut cb: impl FnMut(CaptureStreamCallbackData) + Send + 'static,
// ) -> Result<CaptureStatus> {
//     // Note: cursor mode not directly supported without portal
//     log::info!(
//         "Cursor setting ({}) noted but not supported without portal",
//         config.include_cursor
//     );
//
//     let streams =
//         request_screen_access_sync(0, 0) // simplified parameters
//             .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;
//
//     let stream = if streams.is_empty() {
//         return Err(Error::Other("No screens available".to_string()));
//     } else {
//         // Try to find the requested screen, otherwise use the first available one
//         streams
//             .iter()
//             .find(|s| s.screen_info.name == config.name)
//             .unwrap_or(&streams[0])
//     };
//
//     // Create PipeWire capturer for streaming
//     let mut capturer = PipeWireCapturer::new()
//         .map_err(|e| Error::Other(format!("Failed to create PipeWire capturer: {}", e)))?;
//
//     capturer
//         .create_stream(stream.node_id, stream.width, stream.height)
//         .map_err(|e| Error::Other(format!("Failed to create PipeWire stream: {}", e)))?;
//
//     let mut index = 0;
//     let interval_ms = config.fps.map(|v| 1000.0 / v);
//     let sleeper = SpinSleeper::default();
//     let start_time = std::time::Instant::now();
//
//     log::info!(
//         "Starting PipeWire screen capture stream for screen: {} at {} FPS",
//         config.name,
//         config.fps.unwrap_or(30.0)
//     );
//
//     // Main capture loop using PipeWire
//     loop {
//         // Check for cancellation signal
//         if config.cancel_sig.load(Ordering::Relaxed) {
//             log::info!("Stream capture stopped by user after {} frames", index);
//             return Ok(CaptureStatus::Stopped);
//         }
//
//         // Capture frame from PipeWire
//         match capturer.capture_frame(stream.node_id) {
//             Ok(frame_data) => {
//                 let start = Instant::now();
//
//                 let capture = Capture {
//                     width: stream.width,
//                     height: stream.height,
//                     pixel_data: frame_data,
//                 };
//
//                 cb(CaptureStreamCallbackData {
//                     frame_index: index,
//                     capture_time: start.elapsed(),
//                     elapse: start_time.elapsed(),
//                     data: capture,
//                 });
//             }
//             Err(e) => {
//                 log::error!("Failed to capture frame: {}", e);
//                 return Err(Error::Other(format!("Frame capture failed: {}", e)));
//             }
//         }
//
//         // Maintain target frame rate
//         if let Some(interval) = interval_ms {
//             let target_time =
//                 start_time + Duration::from_millis((interval * (index + 1) as f64) as u64);
//             sleeper.sleep_until(target_time);
//         }
//
//         index += 1;
//
//         // Log progress every 100 frames
//         if index % 100 == 0 {
//             log::info!(
//                 "PipeWire stream capture progress: {} frames captured",
//                 index
//             );
//         }
//
//         // For demo purposes, stop after a few frames
//         if index >= 3 {
//             log::info!("Demo: stopping after {} frames", index);
//             return Ok(CaptureStatus::Stopped);
//         }
//     }
// }
//

