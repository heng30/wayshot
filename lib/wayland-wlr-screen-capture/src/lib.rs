mod backend;
mod capture;
mod error;
mod screen_info;

pub use capture::*;
pub use error::*;
pub use screen_info::*;

#[derive(Clone)]
pub struct WaylandWlrScreenCapture;

impl screen_capture::ScreenCapture for WaylandWlrScreenCapture {
    fn available_screens(
        &mut self,
    ) -> Result<Vec<screen_capture::ScreenInfo>, screen_capture::ScreenInfoError> {
        screen_info::available_screens()
    }

    fn capture_mean_time(
        &mut self,
        screen_name: &str,
        counts: u32,
    ) -> Result<std::time::Duration, screen_capture::ScreenCaptureError> {
        capture::capture_mean_time(screen_name, counts)
            .map_err(|e| screen_capture::ScreenCaptureError::Capture(e.to_string()))
    }

    fn capture_output_stream(
        &mut self,
        config: screen_capture::CaptureStreamConfig,
        cb: impl FnMut(screen_capture::CaptureStreamCallbackData),
    ) -> Result<screen_capture::CaptureStatus, screen_capture::ScreenCaptureError> {
        capture::capture_output_stream(config, cb)
            .map_err(|e| screen_capture::ScreenCaptureError::Capture(e.to_string()))
    }
}
