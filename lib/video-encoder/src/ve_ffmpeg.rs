use super::{
    EncodedFrame, EncoderError, ResizedImageBuffer, Result, VideoEncoder, VideoEncoderConfig,
};
use ffmpeg_next::{Dictionary, Rational, codec, encoder, format, frame, packet};
use std::time::Duration;

pub struct FfmpegVideoEncoder {
    width: u32,
    height: u32,
    frame_index: u64,
    encoder: encoder::Video,
}

impl FfmpegVideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self> {
        assert!(config.width > 0 && config.height > 0);

        ffmpeg_next::init().map_err(|e| {
            EncoderError::VideoEncodingFailed(format!("Failed to initialize ffmpeg: {}", e))
        })?;

        let codec = encoder::find_by_name("libx264")
            .or_else(|| encoder::find(codec::Id::H264))
            .ok_or_else(|| {
                EncoderError::VideoEncodingFailed("H.264 encoder not found".to_string())
            })?;

        let mut encoder = codec::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(|e| {
                EncoderError::VideoEncodingFailed(format!("Failed to create encoder context: {e}"))
            })?;

        encoder.set_width(config.width);
        encoder.set_height(config.height);
        encoder.set_format(format::Pixel::YUV420P);
        encoder.set_frame_rate(Some(Rational::new(config.fps as i32, 1)));
        encoder.set_time_base((1, config.fps as i32));

        let fps = if config.annexb {
            config.fps * 5
        } else {
            config.fps
        };

        let mut opts = Dictionary::new();
        opts.set("preset", if config.annexb { "faster" } else { "superfast" });
        opts.set("profile", "baseline");
        opts.set("crf", "23");
        opts.set("g", &fps.to_string()); // max_keyframe_interval
        opts.set("tune", "zerolatency");
        opts.set("forced-idr", "1"); // Force keyframes more regularly

        let x264_params = format!(
            "annexb={}:bframes=0:cabac=0:scenecut=0:keyint={fps}:keyint_min={fps}:rc_lookahead=0",
            if config.annexb { 1 } else { 0 },
        );
        opts.set("x264-params", x264_params.as_str());

        let encoder = encoder.open_with(opts).map_err(|e| {
            EncoderError::VideoEncodingFailed(format!("Failed to open encoder: {e}"))
        })?;

        Ok(Self {
            width: config.width,
            height: config.height,
            encoder,
            frame_index: 0,
        })
    }

    fn create_yuv_frame_from_i420(&self, i420_data: &[u8]) -> Result<frame::Video> {
        let mut output_frame = frame::Video::empty();
        output_frame.set_format(format::Pixel::YUV420P);
        output_frame.set_width(self.width);
        output_frame.set_height(self.height);

        unsafe {
            output_frame.alloc(format::Pixel::YUV420P, self.width, self.height);
        }

        // Copy I420 data to YUV420P frame planes
        let frame_size = (self.width * self.height) as usize;

        // Y plane
        let y_plane = output_frame.data_mut(0);
        y_plane[..frame_size].copy_from_slice(&i420_data[0..frame_size]);

        // U plane
        let u_plane = output_frame.data_mut(1);
        let u_size = frame_size / 4;
        u_plane[..u_size].copy_from_slice(&i420_data[frame_size..frame_size + u_size]);

        // V plane
        let v_plane = output_frame.data_mut(2);
        v_plane[..u_size].copy_from_slice(&i420_data[frame_size + u_size..]);

        Ok(output_frame)
    }
}

impl VideoEncoder for FfmpegVideoEncoder {
    fn encode_frame(&mut self, img: ResizedImageBuffer) -> Result<EncodedFrame> {
        let (img_width, img_height) = img.dimensions();
        if img_width != self.width || img_height != self.height {
            return Err(EncoderError::ImageProcessingFailed(format!(
                "frame is already resize. current size: {}x{}. expect size: {}x{}",
                img_width, img_height, self.width, self.height
            )));
        }

        let i420_data = super::rgb_to_i420_yuv(img.as_raw(), self.width, self.height)?;
        let mut output_frame = self.create_yuv_frame_from_i420(&i420_data)?;
        output_frame.set_pts(Some(self.frame_index as i64));

        self.encoder.send_frame(&output_frame).map_err(|e| {
            EncoderError::VideoEncodingFailed(format!("FFmpeg encoding failed: {e}"))
        })?;

        let mut packet = packet::Packet::empty();
        match self.encoder.receive_packet(&mut packet) {
            Ok(_) => {
                if let Some(data) = packet.data() {
                    self.frame_index += 1;
                    Ok(EncodedFrame::Frame((self.frame_index, data.to_vec())))
                } else {
                    return Err(EncoderError::VideoEncodingFailed(
                        "FFmpeg encoder encode data is empty".to_string(),
                    ));
                }
            }
            Err(ffmpeg_next::Error::Other { errno }) if errno == 11 => {
                return Err(EncoderError::VideoEncodingFailed(
                    "FFmpeg encoder encode empty frame".to_string(),
                ));
            }
            Err(ffmpeg_next::Error::Eof) => {
                return Err(EncoderError::VideoEncodingFailed(
                    "FFmpeg encoder Eof".to_string(),
                ));
            }
            Err(e) => {
                return Err(EncoderError::VideoEncodingFailed(format!(
                    "FFmpeg receive packet failed: {e}"
                )));
            }
        }
    }

    fn headers(&mut self) -> Result<Vec<u8>> {
        log::debug!("Encoding test frame to extract headers from FFmpeg");

        // Create a test frame (black frame)
        let test_frame_data = vec![0u8; (self.width * self.height * 3) as usize];
        let test_img = image::RgbImage::from_raw(self.width, self.height, test_frame_data)
            .ok_or_else(|| {
                EncoderError::ImageProcessingFailed(
                    "Failed to create test frame for header extraction".to_string(),
                )
            })?;

        let i420_data = super::rgb_to_i420_yuv(test_img.as_raw(), self.width, self.height)?;
        let mut output_frame = self.create_yuv_frame_from_i420(&i420_data)?;
        output_frame.set_pts(Some(0));

        // Send test frame to encoder
        self.encoder.send_frame(&output_frame).map_err(|e| {
            EncoderError::VideoEncodingFailed(format!("FFmpeg test frame encoding failed: {e}"))
        })?;

        // Try to receive packet (should contain SPS/PPS headers)
        let mut packet = packet::Packet::empty();
        match self.encoder.receive_packet(&mut packet) {
            Ok(_) => {
                if let Some(data) = packet.data() {
                    log::debug!(
                        "Successfully extracted headers from FFmpeg test frame: {} bytes",
                        data.len()
                    );
                    return Ok(data.to_vec());
                }
            }
            Err(ffmpeg_next::Error::Other { errno }) if errno == 11 => {
                log::warn!("FFmpeg encoder needs more frames to generate headers");
            }
            Err(e) => {
                return Err(EncoderError::VideoEncodingFailed(format!(
                    "Failed to receive headers packet: {e}",
                )));
            }
        }

        log::warn!("Could not extract headers from FFmpeg test frame, using empty headers");
        Ok(vec![])
    }

    fn flush(mut self: Box<Self>, mut cb: Box<dyn FnMut(Vec<u8>) + 'static>) -> Result<()> {
        let mut empty_count = 0;
        let max_empty_attempts = 3;

        loop {
            let mut packet = packet::Packet::empty();
            match self.encoder.receive_packet(&mut packet) {
                Ok(_) => {
                    empty_count += 1;

                    if let Some(data) = packet.data() {
                        cb(data.to_vec());
                        empty_count = 0;
                    } else {
                        if empty_count >= max_empty_attempts {
                            log::debug!("FFmpeg encoder flush completed (empty data limit)");
                            break;
                        }
                    }
                }
                Err(ffmpeg_next::Error::Eof) => {
                    log::debug!("FFmpeg encoder flush completed (EOF)");
                    break;
                }
                Err(ffmpeg_next::Error::Other { errno }) if errno == 11 => {
                    empty_count += 1;
                    if empty_count >= max_empty_attempts {
                        log::debug!("FFmpeg encoder flush completed (EAGAIN limit)");
                        break;
                    }
                    continue;
                }
                Err(e) => {
                    return Err(EncoderError::VideoEncodingFailed(format!(
                        "Failed to flush encoder: {e}"
                    )));
                }
            }
            std::thread::sleep(Duration::from_millis(3));
        }

        Ok(())
    }
}
