#[cfg(feature = "x264")]
mod ve_x264;

#[cfg(feature = "openh264")]
mod ve_openh264;

#[cfg(feature = "ffmpeg")]
mod ve_ffmpeg;

use derive_setters::Setters;
use image::{ImageBuffer, Rgb};
use yuv::{
    YuvChromaSubsampling, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    rgb_to_yuv420,
};

// Standard video timescale (90kHz) for better compatibility
pub const VIDEO_TIMESCALE: u32 = 90000;

#[derive(thiserror::Error, Debug)]
pub enum EncoderError {
    #[error("Image processing failed: {0}")]
    ImageProcessingFailed(String),

    #[error("Video encoding failed: {0}")]
    VideoEncodingFailed(String),
}

pub type Result<T> = std::result::Result<T, EncoderError>;
pub type ResizedImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

#[derive(Debug, Clone)]
pub enum EncodedFrame {
    Empty(u64),
    Frame {
        timestamp: u64,
        data: Vec<u8>,
        is_keyframe: bool,
    },
    End,
}

impl Default for EncodedFrame {
    fn default() -> Self {
        EncodedFrame::Empty(0)
    }
}

pub trait VideoEncoder {
    fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame>;
    fn headers(&mut self) -> Result<Vec<u8>>;
    fn flush(self: Box<Self>, cb: Box<dyn FnMut(Vec<u8>, bool) + 'static>) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionPreset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    #[default]
    Medium,
    Slow,
    Slower,
    Veryslow,
}

impl CompressionPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ultrafast => "ultrafast",
            Self::Superfast => "superfast",
            Self::Veryfast => "veryfast",
            Self::Faster => "faster",
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::Slow => "slow",
            Self::Slower => "slower",
            Self::Veryslow => "veryslow",
        }
    }

    #[cfg(feature = "x264")]
    pub fn as_x264_preset(&self) -> x264::Preset {
        match self {
            Self::Ultrafast => x264::Preset::Ultrafast,
            Self::Superfast => x264::Preset::Superfast,
            Self::Veryfast => x264::Preset::Veryfast,
            Self::Faster => x264::Preset::Faster,
            Self::Fast => x264::Preset::Fast,
            Self::Medium => x264::Preset::Medium,
            Self::Slow => x264::Preset::Slow,
            Self::Slower => x264::Preset::Slower,
            Self::Veryslow => x264::Preset::Veryslow,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tune {
    #[default]
    None,
    Film,
    Animation,
    Grain,
    Stillimage,
    Fastdecode,
    Zerolatency,
}

impl Tune {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Film => "film",
            Self::Animation => "animation",
            Self::Grain => "grain",
            Self::Stillimage => "stillimage",
            Self::Fastdecode => "fastdecode",
            Self::Zerolatency => "zerolatency",
        }
    }

    #[cfg(feature = "x264")]
    pub fn as_x264_tune(&self) -> x264::Tune {
        match self {
            Self::None => x264::Tune::None,
            Self::Film => x264::Tune::Film,
            Self::Animation => x264::Tune::Animation,
            Self::Grain => x264::Tune::Grain,
            Self::Stillimage => x264::Tune::None,
            Self::Fastdecode => x264::Tune::None,
            Self::Zerolatency => x264::Tune::None,
        }
    }
}

#[derive(Clone, Debug, Default, Setters)]
#[setters(prefix = "with_")]
pub struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub annexb: bool,

    // CRF (Constant Rate Factor) - lower = better quality, larger file (0-51)
    pub crf: Option<u8>, // If None, uses encoder default (typically 23)
    pub preset: Option<CompressionPreset>,
    pub tune: Option<Tune>,
}

impl VideoEncoderConfig {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            fps: 25,
            annexb: false,
            crf: None,
            preset: None,
            tune: None,
        }
    }
}

#[cfg(any(feature = "x264", feature = "openh264", feature = "ffmpeg"))]
pub fn new(config: VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>> {
    #[cfg(feature = "x264")]
    let ve = ve_x264::X264VideoEncoder::new(config)?;

    #[cfg(feature = "openh264")]
    let ve = ve_openh264::OpenH264VideoEncoder::new(config)?;

    #[cfg(feature = "ffmpeg")]
    let ve = ve_ffmpeg::FfmpegVideoEncoder::new(config)?;

    Ok(Box::new(ve))
}

pub fn rgb_to_i420_yuv(rgb_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let frame_size = (width * height) as usize;

    let mut planar_image =
        YuvPlanarImageMut::<u8>::alloc(width, height, YuvChromaSubsampling::Yuv420);

    rgb_to_yuv420(
        &mut planar_image,
        rgb_data,
        width * 3,
        YuvRange::Limited,
        YuvStandardMatrix::Bt601,
        YuvConversionMode::Balanced,
    )
    .map_err(|e| {
        EncoderError::ImageProcessingFailed(format!("RGB to YUV conversion failed: {:?}", e))
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
