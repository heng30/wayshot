use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Capture {
    pub width: u32,
    pub height: u32,

    /// Raw pixel data in RGBA format
    pub pixel_data: Vec<u8>,
}

#[derive(Debug)]
pub enum CaptureStatus {
    Finished,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct CaptureStreamConfig {
    /// Name of the output to capture
    pub name: String,

    /// Whether to include the mouse cursor in the capture
    pub include_cursor: bool,

    /// Target frames per second for capture (None for maximum speed)
    pub fps: Option<f64>,

    /// Cancellation signal - when set to true, the capture loop will exit
    pub cancel_sig: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct CaptureStreamCallbackData {
    /// Index of the captured frame (starts from 0)
    pub frame_index: u64,

    /// Time taken to capture this frame
    pub capture_time: Duration,

    /// Total elapsed time since the capture started
    pub elapse: Duration,

    /// The captured image data
    pub data: Capture,
}
