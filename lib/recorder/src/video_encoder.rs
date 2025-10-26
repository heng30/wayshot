use crate::{FPS, recorder::ResizedImageBuffer, recorder_error::RecorderError};
use x264::{Colorspace, Data, Encoder, Image, Preset, Setup, Tune};

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
    encoder: Encoder,
    fps: FPS,
}

impl VideoEncoder {
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

    pub fn new(width: u32, height: u32, fps: FPS) -> Result<Self, RecorderError> {
        assert!(width > 0 && height > 0);

        // Create x264 encoder with optimized settings for screen recording
        // Using ultrafast preset with stillimage tune for minimal memory usage
        let encoder = Setup::preset(
            Preset::Ultrafast, // Minimal memory usage and fastest encoding
            Tune::StillImage,  // Optimized for screen content (static images)
            true,              // fast_decode: Enable fast decoding
            true,              // zero_latency: Minimal internal buffering
        )
        .fps(fps.to_u32(), 1)
        .baseline() // Ensure maximum compatibility
        .build(Colorspace::RGB, width as i32, height as i32)
        .map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("Failed to create x264 encoder: {:?}", e))
        })?;

        Ok(Self {
            encoder,
            width,
            height,
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

        // Create x264 image from RGB buffer
        let image = Image::rgb(self.width as i32, self.height as i32, img.as_raw());

        // Calculate timestamp in milliseconds (frame_index * 1000 / fps)
        let timestamp_ms = (self.frame_index * 1000) / self.fps.to_u32() as u64;

        let (data, _) = self
            .encoder
            .encode(timestamp_ms as i64, image)
            .map_err(|e| {
                RecorderError::VideoEncodingFailed(format!("x264 encoding failed: {:?}", e))
            })?;

        let encoded_data = data.entirety().to_vec();
        let encoded_frame = EncodedFrame::Frame((self.frame_index, encoded_data));
        self.frame_index += 1;
        Ok(encoded_frame)
    }
}
