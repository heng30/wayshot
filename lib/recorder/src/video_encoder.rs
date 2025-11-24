#[cfg(feature = "x264-video-encoder")]
mod ve_x264;

#[cfg(feature = "openh264-video-encoder")]
mod ve_openh264;

use crate::{FPS, RecorderError, recorder::ResizedImageBuffer};
use derive_setters::Setters;

#[derive(Debug, Clone)]
pub enum EncodedFrame {
    Empty(u64),
    Frame((u64, Vec<u8>)),
    End,
}

impl Default for EncodedFrame {
    fn default() -> Self {
        EncodedFrame::Empty(0)
    }
}

pub trait VideoEncoder {
    fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame, RecorderError>;
    fn headers(&mut self) -> Result<Vec<u8>, RecorderError>;
    fn flush(self: Box<Self>, cb: Box<dyn FnMut(Vec<u8>) + 'static>) -> Result<(), RecorderError>;
}

#[derive(Clone, Debug, Setters)]
#[setters(prefix = "with_")]
pub struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: FPS,
    pub annexb: bool,
}

impl VideoEncoderConfig {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            fps: FPS::Fps25,
            annexb: false,
        }
    }
}

pub fn new(config: VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>, RecorderError> {
    #[cfg(feature = "x264-video-encoder")]
    let ve = ve_x264::X264VideoEncoder::new(config)?;

    #[cfg(feature = "openh264-video-encoder")]
    let ve = ve_openh264::OpenH264VideoEncoder::new(config)?;

    Ok(Box::new(ve))
}
