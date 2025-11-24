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

pub fn rgb_to_i420_yuv(rgb_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, RecorderError> {
    use yuv::{
        YuvChromaSubsampling, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
        rgb_to_yuv420,
    };
    let frame_size = (width * height) as usize;

    // Allocate YUV planar image
    let mut planar_image =
        YuvPlanarImageMut::<u8>::alloc(width, height, YuvChromaSubsampling::Yuv420);

    // Convert RGB to YUV420
    rgb_to_yuv420(
        &mut planar_image,
        rgb_data,
        width * 3, // RGB stride (3 bytes per pixel)
        YuvRange::Limited,
        YuvStandardMatrix::Bt601,
        YuvConversionMode::Balanced,
    )
    .map_err(|e| {
        RecorderError::ImageProcessingFailed(format!("RGB to YUV conversion failed: {:?}", e))
    })?;

    // Extract the YUV data from the planar image
    let mut yuv_data = vec![0u8; frame_size * 3 / 2];

    // Copy Y plane
    yuv_data[0..frame_size].copy_from_slice(planar_image.y_plane.borrow());

    // Copy U plane
    let u_plane_end = frame_size + frame_size / 4;
    yuv_data[frame_size..u_plane_end].copy_from_slice(planar_image.u_plane.borrow());

    // Copy V plane
    yuv_data[u_plane_end..].copy_from_slice(planar_image.v_plane.borrow());

    Ok(yuv_data)
}
