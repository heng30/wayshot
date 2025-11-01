mod capture;
mod screen_info;

pub use capture::*;
pub use screen_info::*;

#[derive(thiserror::Error, Debug, Clone)]
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
        self,
        config: CaptureStreamConfig,
        cb: impl FnMut(CaptureStreamCallbackData),
    ) -> Result<CaptureStatus, ScreenCaptureError>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rectangle {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
