#[macro_use]
extern crate derivative;

mod backend;
mod capture;
mod cursor;
mod error;
mod screen_info;

pub use capture::*;
pub use cursor::*;
pub use error::*;
pub use screen_info::*;

#[derive(Clone, Default)]
pub struct ScreenCaptureWaylandWlr;

impl screen_capture::ScreenCapture for ScreenCaptureWaylandWlr {
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
        self,
        config: screen_capture::CaptureStreamConfig,
        cb: impl FnMut(screen_capture::CaptureStreamCallbackData),
    ) -> Result<screen_capture::CaptureStatus, screen_capture::ScreenCaptureError> {
        capture::capture_output_stream(config, cb)
            .map_err(|e| screen_capture::ScreenCaptureError::Capture(e.to_string()))
    }
}
