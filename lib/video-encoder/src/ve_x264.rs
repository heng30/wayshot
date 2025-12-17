use crate::{
    EncodedFrame, EncoderError, ResizedImageBuffer, Result, VIDEO_TIMESCALE, VideoEncoder,
    VideoEncoderConfig, rgb_to_i420_yuv,
};
use x264::{Colorspace, Encoder, Image, Preset, Setup, Tune};

pub struct X264VideoEncoder {
    width: u32,
    height: u32,
    frame_index: u64,
    fps: u32,
    encoder: Encoder,
}

impl X264VideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self> {
        let VideoEncoderConfig {
            width,
            height,
            fps,
            annexb,
            ..
        } = config;

        assert!(width > 0 && height > 0);
        let is_real_time = annexb;

        let encoder = Setup::preset(
            if is_real_time {
                Preset::Faster
            } else {
                Preset::Superfast
            },
            Tune::None,
            true,
            true,
        )
        .max_keyframe_interval(if is_real_time {
            fps as i32 * 5
        } else {
            fps as i32
        })
        .fps(fps, 1)
        .scenecut_threshold(0)
        .annexb(annexb)
        .baseline()
        .build(Colorspace::I420, width as i32, height as i32)
        .map_err(|e| {
            EncoderError::VideoEncodingFailed(format!("Failed to create x264 encoder: {e:?}"))
        })?;

        Ok(Self {
            encoder,
            width,
            height,
            frame_index: 0,
            fps,
        })
    }
}

impl VideoEncoder for X264VideoEncoder {
    fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame> {
        let (img_width, img_height) = img.dimensions();
        if img_width != self.width || img_height != self.height {
            return Err(EncoderError::ImageProcessingFailed(format!(
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
        let timestamp = (self.frame_index * VIDEO_TIMESCALE as u64) / self.fps as u64;
        let (data, _) = self.encoder.encode(timestamp as i64, image).map_err(|e| {
            EncoderError::VideoEncodingFailed(format!("x264 encoding failed: {:?}", e))
        })?;

        let encoded_data = data.entirety().to_vec();
        let encoded_frame = EncodedFrame::Frame((self.frame_index, encoded_data));
        self.frame_index += 1;

        Ok(encoded_frame)
    }

    fn headers(&mut self) -> Result<Vec<u8>> {
        Ok(self
            .encoder
            .headers()
            .map_err(|e| {
                EncoderError::VideoEncodingFailed(format!("Failed to get encoder headers: {e:?}"))
            })?
            .entirety()
            .to_vec())
    }

    fn flush(self: Box<Self>, mut cb: Box<dyn FnMut(Vec<u8>) + 'static>) -> Result<()> {
        let mut items = self.encoder.flush();
        while let Some(result) = items.next() {
            match result {
                Ok((data, _)) => cb(data.entirety().to_vec()),
                Err(e) => {
                    return Err(EncoderError::VideoEncodingFailed(format!(
                        "Failed to flush encoder frame: {e:?}"
                    )));
                }
            }
        }

        Ok(())
    }
}
