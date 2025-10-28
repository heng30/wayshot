mod capture;
mod screen_info;

pub use capture::*;
pub use screen_info::*;

#[derive(thiserror::Error, Debug)]
pub enum ScreenCaptureError {
    #[error("{0}")]
    Capture(String),
}

pub trait ScreenCapture {
    fn available_screens(&mut self) -> Result<Vec<ScreenInfo>, ScreenInfoError>;

    fn capture_mean_time(
        &mut self,
        screen_name: &str,
        counts: u32,
    ) -> Result<std::time::Duration, ScreenCaptureError>;

    fn capture_output_stream(
        &mut self,
        config: CaptureStreamConfig,
        cb: impl FnMut(CaptureStreamCallbackData),
    ) -> Result<CaptureStatus, ScreenCaptureError>;
}
