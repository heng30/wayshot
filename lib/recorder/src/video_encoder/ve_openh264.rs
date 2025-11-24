use super::EncodedFrame;
use crate::{RecorderError, VideoEncoder, VideoEncoderConfig, recorder::ResizedImageBuffer};
use openh264::{
    OpenH264API,
    encoder::{Complexity, Encoder, EncoderConfig, FrameRate, Profile, UsageType},
    formats::{RgbSliceU8, YUVBuffer},
};

pub struct OpenH264VideoEncoder {
    width: u32,
    height: u32,
    frame_index: u64,
    encoder: Encoder,
}

impl OpenH264VideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self, RecorderError> {
        assert!(config.width > 0 && config.height > 0);

        let encoder_config = EncoderConfig::new()
            .max_frame_rate(FrameRate::from_hz(config.fps.to_u32() as f32))
            .skip_frames(false)
            .usage_type(UsageType::ScreenContentRealTime)
            .complexity(Complexity::High)
            .profile(Profile::Baseline);

        let encoder = Encoder::with_api_config(OpenH264API::from_source(), encoder_config)
            .map_err(|e| {
                RecorderError::VideoEncodingFailed(format!(
                    "Failed to create OpenH264 encoder: {e:?}"
                ))
            })?;

        Ok(Self {
            width: config.width,
            height: config.height,
            encoder,
            frame_index: 0,
        })
    }
}

impl VideoEncoder for OpenH264VideoEncoder {
    fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame, RecorderError> {
        let (img_width, img_height) = img.dimensions();
        if img_width != self.width || img_height != self.height {
            return Err(RecorderError::ImageProcessingFailed(format!(
                "frame is already resize. current size: {}x{}. expect size: {}x{}",
                img_width, img_height, self.width, self.height
            )));
        }

        // Convert RGB to YUV using OpenH264's RGB source
        let rgb_source = RgbSliceU8::new(img.as_raw(), (self.width as usize, self.height as usize));
        let yuv_buffer = YUVBuffer::from_rgb8_source(rgb_source);

        let bitstream = self.encoder.encode(&yuv_buffer).map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("OpenH264 encoding failed: {:?}", e))
        })?;

        let encoded_frame = EncodedFrame::Frame((self.frame_index, bitstream.to_vec()));
        self.frame_index += 1;

        Ok(encoded_frame)
    }

    fn headers(&mut self) -> Result<Vec<u8>, RecorderError> {
        Ok(vec![])
    }

    fn flush(self: Box<Self>, _cb: Box<dyn FnMut(Vec<u8>) + 'static>) -> Result<(), RecorderError> {
        Ok(())
    }
}
