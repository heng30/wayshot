use super::{EncodedFrame, rgb_to_i420_yuv};
use crate::{RecorderError, VideoEncoder, VideoEncoderConfig, recorder::ResizedImageBuffer};
use image::{ImageBuffer, Rgb};
use openh264::{
    OpenH264API,
    encoder::{Complexity, Encoder, EncoderConfig, FrameRate, Profile, RateControlMode, UsageType},
    formats::{RgbSliceU8, YUVBuffer},
};
use std::time::Instant;

pub struct OpenH264VideoEncoder {
    width: u32,
    height: u32,
    annexb: bool,
    frame_index: u64,
    encoder: Encoder,
    headers_cache: Option<Vec<u8>>,
    first_frame_encoded: bool,
}

impl OpenH264VideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self, RecorderError> {
        assert!(config.width > 0 && config.height > 0);

        let encoder_config = EncoderConfig::new()
            .skip_frames(false)
            .profile(Profile::Baseline)
            .complexity(Complexity::High)
            .background_detection(false)
            .adaptive_quantization(false)
            .rate_control_mode(RateControlMode::Bufferbased)
            .usage_type(UsageType::ScreenContentRealTime)
            .max_frame_rate(FrameRate::from_hz(config.fps.to_u32() as f32));

        let encoder = Encoder::with_api_config(OpenH264API::from_source(), encoder_config)
            .map_err(|e| {
                RecorderError::VideoEncodingFailed(format!(
                    "Failed to create OpenH264 encoder: {e:?}"
                ))
            })?;

        Ok(Self {
            width: config.width,
            height: config.height,
            annexb: config.annexb,
            encoder,
            frame_index: 0,
            headers_cache: None,
            first_frame_encoded: false,
        })
    }

    fn convert_annex_b_to_length_prefixed(&self, annex_b_data: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(annex_b_data.len());
        let mut i = 0;

        // Parse Annex B NAL units and convert to length-prefixed format
        while i < annex_b_data.len() {
            // Look for NAL start codes (00 00 00 01 or 00 00 01)
            let start_code_len = if i + 4 <= annex_b_data.len()
                && annex_b_data[i] == 0
                && annex_b_data[i + 1] == 0
                && annex_b_data[i + 2] == 0
                && annex_b_data[i + 3] == 1
            {
                4
            } else if i + 3 <= annex_b_data.len()
                && annex_b_data[i] == 0
                && annex_b_data[i + 1] == 0
                && annex_b_data[i + 2] == 1
            {
                3
            } else {
                i += 1;
                continue;
            };

            // Skip the start code
            let nal_start = i + start_code_len;
            if nal_start >= annex_b_data.len() {
                break;
            }

            // Find the end of this NAL unit (next start code or end of data)
            let mut nal_end = nal_start;
            while nal_end + 3 <= annex_b_data.len() {
                if (annex_b_data[nal_end] == 0
                    && annex_b_data[nal_end + 1] == 0
                    && annex_b_data[nal_end + 2] == 0
                    && annex_b_data[nal_end + 3] == 1)
                    || (annex_b_data[nal_end] == 0
                        && annex_b_data[nal_end + 1] == 0
                        && annex_b_data[nal_end + 2] == 1)
                {
                    break;
                }
                nal_end += 1;
            }

            // If we reached the end without finding another start code, go to the actual end
            if nal_end + 3 > annex_b_data.len() {
                nal_end = annex_b_data.len();
            }

            let nal_data = &annex_b_data[nal_start..nal_end];
            if !nal_data.is_empty() {
                // Add length prefix (4 bytes, big-endian)
                result.extend_from_slice(&(nal_data.len() as u32).to_be_bytes());
                result.extend_from_slice(nal_data);
            }

            i = nal_end;
        }

        result
    }

    fn extract_length_prefixed_sps_pps_from_bitstream(&self, bitstream: &[u8]) -> Option<Vec<u8>> {
        let mut sps_data = None;
        let mut pps_data = None;
        let mut result = Vec::new();
        let mut i = 0;

        // Parse length-prefixed NAL units (this should be the converted format)
        while i + 4 <= bitstream.len() {
            // Read NAL unit length (big-endian)
            let nal_length = ((bitstream[i] as u32) << 24)
                | ((bitstream[i + 1] as u32) << 16)
                | ((bitstream[i + 2] as u32) << 8)
                | (bitstream[i + 3] as u32);

            if i + 4 + nal_length as usize > bitstream.len() {
                log::warn!("Invalid NAL length {} at position {}", nal_length, i);
                break;
            }

            let nal_start = i + 4;
            let nal_end = nal_start + nal_length as usize;
            let nal_data = &bitstream[nal_start..nal_end];

            if !nal_data.is_empty() {
                let nal_type = nal_data[0] & 0x1F;
                match nal_type {
                    7 => {
                        sps_data = Some(nal_data);
                        log::debug!("Found SPS: {} bytes", nal_data.len());
                    }
                    8 => {
                        pps_data = Some(nal_data);
                        log::debug!("Found PPS: {} bytes", nal_data.len());
                    }
                    _ => {}
                }
            }

            i = nal_end;
        }

        // If we found both SPS and PPS, create length-prefixed header data
        if let (Some(sps), Some(pps)) = (sps_data, pps_data) {
            // Add length prefix (4 bytes, big-endian) for SPS
            result.extend_from_slice(&(sps.len() as u32).to_be_bytes());
            result.extend_from_slice(sps);
            // Add length prefix (4 bytes, big-endian) for PPS
            result.extend_from_slice(&(pps.len() as u32).to_be_bytes());
            result.extend_from_slice(pps);

            log::debug!(
                "Extracted SPS ({} bytes) and PPS ({} bytes) from OpenH264 bitstream",
                sps.len(),
                pps.len()
            );
            return Some(result);
        }

        log::warn!("Could not find both SPS and PPS in bitstream");
        None
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

        let yuv_raw = rgb_to_i420_yuv(&img.as_raw(), self.width, self.height)?;
        let yuv_buffer = YUVBuffer::from_vec(yuv_raw, self.width as usize, self.height as usize);

        // FIXME: low efficiency(~50ms)
        let now = Instant::now();
        let bitstream = self.encoder.encode(&yuv_buffer).map_err(|e| {
            RecorderError::VideoEncodingFailed(format!("OpenH264 encoding failed: {:?}", e))
        })?;
        log::debug!("openh264 encode yuv frame spent: {:.2?}", now.elapsed());

        let bitstream_data = bitstream.to_vec();
        let final_data = if self.annexb {
            bitstream_data
        } else {
            self.convert_annex_b_to_length_prefixed(&bitstream_data)
        };

        // If this is the first frame and we haven't cached headers yet, try to extract SPS/PPS
        if !self.annexb && !self.first_frame_encoded && self.headers_cache.is_none() {
            if let Some(headers) = self.extract_length_prefixed_sps_pps_from_bitstream(&final_data)
            {
                self.headers_cache = Some(headers);
                log::info!(
                    "Successfully extracted SPS/PPS headers from first OpenH264 frame (annexb: {})",
                    self.annexb
                );
            } else {
                log::warn!(
                    "Could not extract SPS/PPS from first frame (annexb: {})",
                    self.annexb
                );
            }
            self.first_frame_encoded = true;
        }

        let encoded_frame = EncodedFrame::Frame((self.frame_index, final_data));
        self.frame_index += 1;
        Ok(encoded_frame)
    }

    fn headers(&mut self) -> Result<Vec<u8>, RecorderError> {
        // TODO: maybe parse the annexb headers from the test_img
        if self.annexb {
            return Ok(vec![]);
        }

        if let Some(ref headers) = self.headers_cache {
            return Ok(headers.clone());
        }

        log::debug!("Encoding test frame to extract SPS/PPS headers from OpenH264");

        let test_frame_data = vec![0u8; (self.width * self.height * 3) as usize];
        let test_img =
            ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(self.width, self.height, test_frame_data)
                .ok_or_else(|| {
                    RecorderError::ImageProcessingFailed(
                        "Failed to create test frame for SPS/PPS extraction".to_string(),
                    )
                })?;

        let rgb_source = RgbSliceU8::new(
            test_img.as_raw(),
            (self.width as usize, self.height as usize),
        );
        let yuv_buffer = YUVBuffer::from_rgb8_source(rgb_source);

        match self.encoder.encode(&yuv_buffer) {
            Ok(bitstream) => {
                let bitstream_data = bitstream.to_vec();
                let converted_data = self.convert_annex_b_to_length_prefixed(&bitstream_data);
                log::debug!(
                    "Test frame: Converted to length-prefixed for headers extraction (annexb: {})",
                    self.annexb
                );

                if let Some(headers) =
                    self.extract_length_prefixed_sps_pps_from_bitstream(&converted_data)
                {
                    self.headers_cache = Some(headers.clone());
                    log::info!(
                        "Successfully extracted SPS/PPS headers from OpenH264 test frame (annexb: {})",
                        self.annexb
                    );
                    return Ok(headers);
                } else {
                    log::warn!(
                        "Could not extract SPS/PPS from OpenH264 test frame, using empty headers (annexb: {})",
                        self.annexb
                    );
                }
            }
            Err(e) => log::warn!("Failed to encode test frame for SPS/PPS extraction: {e:?}"),
        }

        Ok(vec![])
    }

    fn flush(self: Box<Self>, _cb: Box<dyn FnMut(Vec<u8>) + 'static>) -> Result<(), RecorderError> {
        Ok(())
    }
}
