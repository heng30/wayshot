use super::{MP4PlayerError, Result};
use image::{ImageBuffer, Rgb};
use openh264::{
    decoder::{DecodedYUV, Decoder},
    formats::YUVSource,
};
use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix, yuv420_to_rgb};

pub struct VideoDecoder {
    decoder: Decoder,
    width: u32,
    height: u32,
}

impl VideoDecoder {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let decoder = Decoder::new().map_err(|e| {
            MP4PlayerError::FrameError(format!("Failed to create OpenH264 decoder: {:?}", e))
        })?;

        Ok(Self {
            decoder,
            width,
            height,
        })
    }

    pub fn decode_frame(&mut self, encoded_data: &[u8]) -> Result<Option<DecodedFrame>> {
        if encoded_data.is_empty() {
            return Ok(None);
        }

        let nal_units = self.parse_nal_units(encoded_data);
        for nal_data in nal_units {
            match self.decoder.decode(&nal_data) {
                Ok(Some(yuv_frame)) => {
                    let rgb_data = Self::yuv420_to_rgb(&yuv_frame, self.width, self.height)?;
                    return Ok(Some(DecodedFrame {
                        rgb_data,
                        width: self.width,
                        height: self.height,
                    }));
                }
                Ok(None) => continue,
                Err(_) => continue,
            }
        }

        Ok(None)
    }

    fn parse_nal_units(&self, data: &[u8]) -> Vec<Vec<u8>> {
        // Try to detect if this is AVCC format (starts with length prefix)
        if data.len() >= 4 && data[0] == 0 && data[1] == 0 {
            // This could be AVCC format - check if we find valid length prefixes
            let mut found_length_prefix = false;
            let mut pos = 0;

            while pos + 4 <= data.len() {
                let length =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                if length == 0 || length > data.len() - pos - 4 {
                    break; // Invalid length, probably not AVCC format
                }
                if length > data.len() - pos - 4 {
                    break; // Length exceeds remaining data
                }

                found_length_prefix = true;
                pos += 4 + length;
            }

            if found_length_prefix {
                return self.parse_nal_units_avcc(data);
            }
        }

        vec![]
    }

    fn parse_nal_units_avcc(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut nal_units = Vec::new();
        let mut i = 0;

        while i + 4 <= data.len() {
            // Read length prefix (big-endian 32-bit)
            let length =
                u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;

            if length == 0 || i + 4 + length > data.len() {
                log::debug!("Invalid NAL length: {} at position {}", length, i);
                break;
            }

            i += 4;
            let nal_data = data[i..i + length].to_vec();
            i += length;

            if !nal_data.is_empty() {
                // Convert AVCC to Annex B format by adding start code prefix
                let mut annexb_nal = Vec::with_capacity(4 + nal_data.len());
                annexb_nal.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // 4-byte start code
                annexb_nal.extend_from_slice(&nal_data);

                nal_units.push(annexb_nal);
            }
        }

        nal_units
    }

    fn yuv420_to_rgb(yuv_frame: &DecodedYUV, width: u32, height: u32) -> Result<Vec<u8>> {
        let y_plane = yuv_frame.y();
        let u_plane = yuv_frame.u();
        let v_plane = yuv_frame.v();
        let y_plane_len = y_plane.len();
        let u_plane_len = u_plane.len();
        let v_plane_len = v_plane.len();

        let height_usize = height as usize;
        let yuv_planar_image = YuvPlanarImage {
            y_plane,
            y_stride: (y_plane_len / height_usize) as u32, // Calculate actual stride from data length
            u_plane,
            u_stride: (u_plane_len / (height_usize / 2)) as u32, // U plane stride for 420 format
            v_plane,
            v_stride: (v_plane_len / (height_usize / 2)) as u32, // V plane stride for 420 format
            width,
            height,
        };

        let mut rgb_data = vec![0u8; (width * height * 3) as usize];
        yuv420_to_rgb(
            &yuv_planar_image,
            &mut rgb_data,
            width * 3,                // RGB stride (3 bytes per pixel)
            YuvRange::Limited,        // TV range (16-235) - matches encoder
            YuvStandardMatrix::Bt601, // BT.601 standard - matches encoder
        )
        .map_err(|e| {
            MP4PlayerError::FrameError(format!("YUV to RGB conversion failed: {:?}", e))
        })?;

        Ok(rgb_data)
    }
}

#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub rgb_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl DecodedFrame {
    pub fn to_image_buffer(&self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        let expected_size = (self.width * self.height * 3) as usize;
        if self.rgb_data.len() != expected_size {
            return Err(MP4PlayerError::FrameError(format!(
                "RGB data size mismatch: expected {}, got {}",
                expected_size,
                self.rgb_data.len()
            )));
        }

        ImageBuffer::from_raw(self.width, self.height, self.rgb_data.clone()).ok_or(
            MP4PlayerError::FrameError("Failed to create image buffer from RGB data".to_string()),
        )
    }
}
