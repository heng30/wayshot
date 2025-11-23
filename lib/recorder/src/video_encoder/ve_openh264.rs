use crate::{FPS, RecorderError, recorder::ResizedImageBuffer};
use yuv::{
    YuvChromaSubsampling, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    rgb_to_yuv420,
};

use openh264::{
    encoder::Encoder,
    formats::{RgbSliceU8, YUVBuffer},
};

#[cfg(feature = "openh264-video-encoder")]
pub struct OpenH264Data(Vec<u8>);

#[cfg(feature = "openh264-video-encoder")]
impl OpenH264Data {
    pub fn entirety(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(feature = "openh264-video-encoder")]
/// Compatibility wrapper for OpenH264 flush
pub struct OpenH264Flush {
    completed: bool,
}

#[cfg(feature = "openh264-video-encoder")]
impl OpenH264Flush {
    pub fn new() -> Self {
        Self { completed: false }
    }

    pub fn next(&mut self) -> Option<Result<(OpenH264Data, u32), RecorderError>> {
        if self.completed {
            None
        } else {
            self.completed = true;
            // Return empty data as OpenH264 doesn't have explicit flush
            Some(Ok((OpenH264Data(Vec::new()), 0)))
        }
    }
}

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

pub struct VideoEncoder {
    width: u32,
    height: u32,
    frame_index: u64,
    fps: FPS,

    annexb: bool,

    #[cfg(feature = "x264-video-encoder")]
    encoder: Encoder,

    #[cfg(feature = "openh264-video-encoder")]
    encoder: Encoder,
}

#[cfg(feature = "x264-video-encoder")]
impl VideoEncoder {
    pub fn new(width: u32, height: u32, fps: FPS, annexb: bool) -> Result<Self, RecorderError> {
        assert!(width > 0 && height > 0);

        let encoder = Setup::preset(
            Preset::Superfast, // Use faster preset to avoid potential encoder issues
            Tune::None,        // Use no specific tuning for screen recording
            true,              // fast_decode: Standard decoding
            true,              // zero_latency: Minimal internal buffering
        )
        .fps(fps.to_u32(), 1)
        .max_keyframe_interval(fps.to_u32() as i32) // Simpler keyframe interval
        .scenecut_threshold(0) // Disable scene detection to guarantee keyframes at max interval
        .annexb(annexb) // Use Annex B format if true (start codes), or AVCC format if false (length prefixes, MP4 compatible)
        .baseline() // Use Baseline profile for maximum compatibility
        .build(Colorspace::I420, width as i32, height as i32)
        .map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("Failed to create x264 encoder: {:?}", e))
        })?;

        Ok(Self {
            encoder,
            width,
            height,
            annexb,
            frame_index: 0,
            fps,
        })
    }

    pub fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame, RecorderError> {
        let (img_width, img_height) = img.dimensions();
        if img_width != self.width || img_height != self.height {
            return Err(RecorderError::ImageProcessingFailed(format!(
                "frame is already resize. current size: {}x{}. expect size: {}x{}",
                img_width, img_height, self.width, self.height
            )));
        }

        // Convert RGB to I420 for x264 encoding using yuv library
        let i420_data = rgb_to_i420_yuv(img.as_raw(), self.width, self.height)?;

        // Create x264 image from I420 buffer using manual plane setup
        let frame_size = (self.width * self.height) as usize;
        let y_plane = &i420_data[0..frame_size];
        let u_plane = &i420_data[frame_size..frame_size + frame_size / 4];
        let v_plane = &i420_data[frame_size + frame_size / 4..];

        let planes = [
            x264::Plane {
                stride: self.width as i32,
                data: y_plane,
            },
            x264::Plane {
                stride: self.width as i32 / 2,
                data: u_plane,
            },
            x264::Plane {
                stride: self.width as i32 / 2,
                data: v_plane,
            },
        ];

        let image = Image::new(
            x264::Colorspace::I420,
            self.width as i32,
            self.height as i32,
            &planes,
        );

        // Calculate timestamp in x264 timebase units (frame_index * timebase / fps)
        // x264 uses a timebase of 1/90000 by default, so we need to convert frame number to this timescale
        let timestamp =
            (self.frame_index * mp4m::VIDEO_TIMESCALE as u64) / self.fps.to_u32() as u64;
        let (data, _) = self.encoder.encode(timestamp as i64, image).map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("x264 encoding failed: {:?}", e))
        })?;

        let encoded_data = data.entirety().to_vec();
        let encoded_frame = EncodedFrame::Frame((self.frame_index, encoded_data));
        self.frame_index += 1;

        Ok(encoded_frame)
    }

    pub fn encoder(&mut self) -> &mut Encoder {
        &mut self.encoder
    }

    pub fn headers(&mut self) -> Result<Data<'_>, RecorderError> {
        self.encoder.headers().map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("Failed to get encoder headers: {:?}", e))
        })
    }

    pub fn flush(self) -> Result<x264::Flush, RecorderError> {
        Ok(self.encoder.flush())
    }

    pub fn annexb(&self) -> bool {
        self.annexb
    }
}

#[cfg(feature = "openh264-video-encoder")]
impl VideoEncoder {
    pub fn new(width: u32, height: u32, fps: FPS, annexb: bool) -> Result<Self, RecorderError> {
        assert!(width > 0 && height > 0);

        // Create encoder with default configuration
        // OpenH264 will automatically configure based on the first frame's dimensions
        let encoder = Encoder::new().map_err(|e| {
            RecorderError::VideoEncodingFailed(format!(
                "Failed to create OpenH264 encoder: {:?}",
                e
            ))
        })?;

        Ok(Self {
            encoder,
            width,
            height,
            annexb,
            frame_index: 0,
            fps,
        })
    }

    pub fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame, RecorderError> {
        let (img_width, img_height) = img.dimensions();
        if img_width != self.width || img_height != self.height {
            return Err(RecorderError::ImageProcessingFailed(format!(
                "frame is already resize. current size: {}x{}. expect size: {}x{}",
                img_width, img_height, self.width, self.height
            )));
        }

        // Convert RGB to YUV using OpenH264's RGB source
        let rgb_data = img.as_raw();
        let rgb_source = RgbSliceU8::new(rgb_data, (self.width as usize, self.height as usize));
        let yuv_buffer = YUVBuffer::from_rgb8_source(rgb_source);

        // Encode the frame (OpenH264 will handle timestamps automatically)
        let bitstream = self.encoder.encode(&yuv_buffer).map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("OpenH264 encoding failed: {:?}", e))
        })?;

        let encoded_data = bitstream.to_vec();
        let encoded_frame = EncodedFrame::Frame((self.frame_index, encoded_data));
        self.frame_index += 1;

        Ok(encoded_frame)
    }

    pub fn encoder(&mut self) -> &mut Encoder {
        &mut self.encoder
    }

    pub fn headers(&mut self) -> Result<OpenH264Data, RecorderError> {
        // OpenH264 doesn't have separate headers like x264
        // The headers are included in the first encoded frame
        // Return empty data for compatibility
        Ok(OpenH264Data(Vec::new()))
    }

    pub fn flush(self) -> Result<OpenH264Flush, RecorderError> {
        // OpenH264 doesn't have explicit flush like x264
        // Return compatibility wrapper
        Ok(OpenH264Flush::new())
    }

    pub fn annexb(&self) -> bool {
        self.annexb
    }
}

/// Convert RGB image to I420 (YUV 4:2:0) format using yuv library
fn rgb_to_i420_yuv(rgb_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, RecorderError> {
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
