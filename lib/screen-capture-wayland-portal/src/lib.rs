mod backend;
mod capture;
mod error;
mod screen_info;

pub use backend::*;
pub use capture::*;
pub use error::*;
pub use screen_info::*;

#[derive(Clone, Default)]
pub struct ScreenCaptureWaylandPortal;

impl screen_capture::ScreenCapture for ScreenCaptureWaylandPortal {
    fn available_screens(
        &mut self,
    ) -> std::result::Result<Vec<screen_capture::ScreenInfo>, screen_capture::ScreenInfoError> {
        screen_info::available_screens()
    }

    fn capture_mean_time(
        &mut self,
        screen_name: &str,
        counts: u32,
    ) -> std::result::Result<std::time::Duration, screen_capture::ScreenCaptureError> {
        capture::capture_mean_time(screen_name, counts)
            .map_err(|e| screen_capture::ScreenCaptureError::Capture(e.to_string()))
    }

    fn capture_output_stream(
        self,
        config: screen_capture::CaptureStreamConfig,
        cb: impl FnMut(screen_capture::CaptureStreamCallbackData),
    ) -> std::result::Result<screen_capture::CaptureStatus, screen_capture::ScreenCaptureError> {
        capture::capture_output_stream(config, cb)
            .map_err(|e| screen_capture::ScreenCaptureError::Capture(e.to_string()))
    }

    fn monitor_cursor_position(
        &mut self,
        _config: screen_capture::MonitorCursorPositionConfig,
        _callback: impl FnMut(screen_capture::CursorPosition) + Send + 'static,
    ) -> std::result::Result<(), screen_capture::CursorError> {
        Err(screen_capture::CursorError::ConnectionFailed(
            "Cursor position monitoring is not supported with XDG Portal".to_string(),
        ))
    }
}
