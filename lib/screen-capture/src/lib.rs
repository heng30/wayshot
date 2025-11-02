mod capture;
mod cursor;
mod screen_info;

pub use capture::*;
pub use cursor::*;
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

    fn monitor_cursor_position(
        &mut self,
        config: MonitorCursorPositionConfig,
        callback: impl FnMut(CursorPosition) + Send + 'static,
    ) -> Result<(), CursorError>;
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

impl From<Rectangle> for LogicalSize {
    fn from(value: Rectangle) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
    pub output_x: i32,
    pub output_y: i32,
    pub output_width: i32,
    pub output_height: i32,
}
