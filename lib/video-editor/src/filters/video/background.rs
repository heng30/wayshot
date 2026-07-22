use crate::{
    Result,
    filters::traits::{VideoData, VideoFilter},
    tracks::video_frame_cache::VideoImage,
};
use rayon::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackgroundFilter {
    pub color: [u8; 4],
}

impl Default for BackgroundFilter {
    fn default() -> Self {
        Self {
            color: [0, 0, 0, 255],
        }
    }
}

impl BackgroundFilter {
    pub const NAME: &'static str = "background";

    pub fn new(color: [u8; 4]) -> Self {
        Self { color }
    }

    fn apply_background(&self, buffer: &mut image::RgbaImage) {
        let bg_r = self.color[0] as f32 / 255.0;
        let bg_g = self.color[1] as f32 / 255.0;
        let bg_b = self.color[2] as f32 / 255.0;
        let bg_a = self.color[3] as f32 / 255.0;

        buffer.par_pixels_mut().for_each(|pixel| {
            let fg_r = pixel.0[0] as f32 / 255.0;
            let fg_g = pixel.0[1] as f32 / 255.0;
            let fg_b = pixel.0[2] as f32 / 255.0;
            let fg_a = pixel.0[3] as f32 / 255.0;

            // Alpha blending: result = foreground * fg_alpha + background * (1 - fg_alpha)
            // But we also need to handle background alpha for transparency
            let out_alpha = fg_a + bg_a * (1.0 - fg_a);

            if out_alpha > 0.0 {
                let out_r = (fg_r * fg_a + bg_r * bg_a * (1.0 - fg_a)) / out_alpha;
                let out_g = (fg_g * fg_a + bg_g * bg_a * (1.0 - fg_a)) / out_alpha;
                let out_b = (fg_b * fg_a + bg_b * bg_a * (1.0 - fg_a)) / out_alpha;

                pixel.0[0] = (out_r * 255.0).clamp(0.0, 255.0) as u8;
                pixel.0[1] = (out_g * 255.0).clamp(0.0, 255.0) as u8;
                pixel.0[2] = (out_b * 255.0).clamp(0.0, 255.0) as u8;
                pixel.0[3] = (out_alpha * 255.0).clamp(0.0, 255.0) as u8;
            } else {
                // Both are fully transparent, set to 0
                pixel.0[0] = 0;
                pixel.0[1] = 0;
                pixel.0[2] = 0;
                pixel.0[3] = 0;
            }
        });
    }
}

impl VideoFilter for BackgroundFilter {
    crate::impl_default_video_filter!(BackgroundFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_background(buffer);
            }
        }
        Ok(())
    }
}

