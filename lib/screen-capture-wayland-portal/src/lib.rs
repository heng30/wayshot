mod backend;
mod capture;
mod error;
mod screen_info;

pub use backend::*;
pub use capture::*;
pub use error::*;
use screen_capture::CaptureStatus;
pub use screen_info::*;

// #[derive(Clone, Default)]
// pub struct ScreenCaptureWaylandPortal;
//
// impl screen_capture::ScreenCapture for ScreenCaptureWaylandPortal {
//     fn available_screens(
//         &mut self,
//     ) -> std::result::Result<Vec<screen_capture::ScreenInfo>, screen_capture::ScreenInfoError> {
//         screen_info::available_screens()
//     }
//
//     fn capture_mean_time(
//         &mut self,
//         screen_name: &str,
//         counts: u32,
//     ) -> std::result::Result<std::time::Duration, screen_capture::ScreenCaptureError> {
//         capture::capture_mean_time(screen_name, counts)
//             .map_err(|e| screen_capture::ScreenCaptureError::Capture(e.to_string()))
//     }
//
//     fn capture_output_stream(
//         self,
//         config: screen_capture::CaptureStreamConfig,
//         mut cb: impl FnMut(screen_capture::CaptureStreamCallbackData),
//     ) -> Result<screen_capture::CaptureStatus, screen_capture::ScreenCaptureError> {
//         // Since we can't use Send + 'static callbacks, we'll implement a simple synchronous capture
//         // that captures a single frame and calls the callback
//
//         // Create a temporary directory if needed
//         // std::fs::create_dir_all("/tmp/portal").ok();
//         //
//         todo!()
//
//         // Try to capture a single frame using our backend
//         // match capture::capture_output_by_name(&config.name) {
//         //     Ok(capture) => {
//         //         // Call the callback with the captured data
//         //         let callback_data = screen_capture::CaptureStreamCallbackData {
//         //             frame_index: 0,
//         //             capture_time: std::time::Duration::from_millis(100), // Simulated capture time
//         //             elapse: std::time::Duration::from_millis(100),
//         //             data: capture,
//         //         };
//         //
//         //         cb(callback_data);
//         //
//         //         // Return success status
//         //         Ok(screen_capture::CaptureStatus::Stopped)
//         //     }
//         //     Err(e) => {
//         //         Err(screen_capture::ScreenCaptureError::Capture(format!("Failed to capture frame: {}", e)))
//         //     }
//         // }
//     }
//
//     fn monitor_cursor_position(
//         &mut self,
//         _config: screen_capture::MonitorCursorPositionConfig,
//         _callback: impl FnMut(screen_capture::CursorPosition) + Send + 'static,
//     ) -> std::result::Result<(), screen_capture::CursorError> {
//         Err(screen_capture::CursorError::ConnectionFailed(
//             "Cursor position monitoring is not supported with XDG Portal".to_string(),
//         ))
//     }
// }
