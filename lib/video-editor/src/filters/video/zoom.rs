use crate::{
    Error, Result,
    filters::{
        progress_ratio_from_offset,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use fast_image_resize::{PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image as FastImage};
use image::RgbaImage;
use std::time::Duration;

#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ZoomFilter {
    #[derivative(Default(value = "Duration::ZERO"))]
    pub relative_start: Duration,

    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub zoom_in_duration: Duration,

    #[derivative(Default(value = "Duration::from_secs(2)"))]
    pub hold_duration: Duration,

    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub zoom_out_duration: Duration,

    #[derivative(Default(value = "0.5"))]
    pub level: f32,

    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub center: (f32, f32),
}

impl ZoomFilter {
    pub const NAME: &'static str = "zoom";

    pub fn new(
        zoom_in_duration: Duration,
        hold_duration: Duration,
        zoom_out_duration: Duration,
        level: f32,
        center: (f32, f32),
    ) -> Self {
        Self {
            relative_start: Duration::ZERO,
            zoom_in_duration,
            hold_duration,
            zoom_out_duration,
            level,
            center,
        }
    }

    pub fn total_duration(&self) -> Duration {
        self.zoom_in_duration + self.hold_duration + self.zoom_out_duration
    }

    fn apply_zoom(&self, buffer: &mut RgbaImage, frame_time_offset: Duration) -> Result<()> {
        // Skip effect before start time
        if frame_time_offset < self.relative_start {
            return Ok(());
        }

        // Adjust offset for effect calculation
        let effect_offset = frame_time_offset.saturating_sub(self.relative_start);

        let (src_width, src_height) = (buffer.width(), buffer.height());
        let (dst_width, dst_height) = (buffer.width(), buffer.height());

        let target_scale = self.level;

        let total = self.total_duration();
        let zoom_out_start = self.zoom_in_duration + self.hold_duration;

        let current_scale = if effect_offset < self.zoom_in_duration {
            // Phase 1: Zoom In (from 1.0 to target_scale)
            let ratio = progress_ratio_from_offset(effect_offset, self.zoom_in_duration);
            1.0 - (1.0 - target_scale) * ratio
        } else if effect_offset < zoom_out_start {
            // Phase 2: Hold at target scale
            target_scale
        } else if effect_offset < total {
            // Phase 3: Zoom Out (from target_scale back to 1.0)
            let zoom_out_offset = effect_offset.saturating_sub(zoom_out_start);
            let ratio = if self.zoom_out_duration.is_zero() {
                1.0
            } else {
                (zoom_out_offset.as_secs_f32() / self.zoom_out_duration.as_secs_f32())
                    .clamp(0.0, 1.0)
            };
            target_scale + (1.0 - target_scale) * ratio
        } else {
            // After effect completes, stay at 1.0
            1.0
        };

        // Use single scale for both width and height to maintain aspect ratio
        // Keep crop dimensions in floating point to avoid discrete pixel jumps
        // that cause visible "shifting" during smooth zoom transitions.
        let crop_width_f = src_width as f64 * current_scale as f64;
        let crop_height_f = src_height as f64 * current_scale as f64;

        if crop_width_f < 1.0 || crop_height_f < 1.0 || crop_width_f > src_width as f64
            || crop_height_f > src_height as f64
        {
            return Ok(());
        }

        // Calculate crop center position in source image (floating point)
        let crop_center_x = src_width as f64 * self.center.0 as f64;
        let crop_center_y = src_height as f64 * self.center.1 as f64;

        // Calculate top-left position from center point (floating point)
        // This keeps the center point stable as crop size changes continuously
        let x_offset_f = crop_center_x - crop_width_f / 2.0;
        let y_offset_f = crop_center_y - crop_height_f / 2.0;

        // Boundary check - ensure crop box doesn't exceed source image bounds
        let x_offset_f = x_offset_f.clamp(0.0, src_width as f64 - crop_width_f);
        let y_offset_f = y_offset_f.clamp(0.0, src_height as f64 - crop_height_f);

        let src_data = buffer.clone().into_raw();
        let src_image = FastImage::from_vec_u8(src_width, src_height, src_data, PixelType::U8x4)
            .map_err(|e| Error::InvalidConfig(format!("Failed to create source image: {}", e)))?;

        let mut dst_data = vec![0u8; (dst_width * dst_height * 4) as usize];
        let mut dst_image =
            FastImage::from_slice_u8(dst_width, dst_height, &mut dst_data, PixelType::U8x4)
                .map_err(|e| {
                    Error::InvalidConfig(format!("Failed to create destination image: {}", e))
                })?;

        // Use floating-point crop coordinates for smooth sub-pixel transitions.
        // fast_image_resize's crop supports f64, which avoids the discrete
        // pixel-level jumps that occur when rounding to u32.
        let resize_options = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(
                fast_image_resize::FilterType::Lanczos3,
            ))
            .crop(
                x_offset_f,
                y_offset_f,
                crop_width_f,
                crop_height_f,
            );

        Resizer::new()
            .resize(&src_image, &mut dst_image, &resize_options)
            .map_err(|e| Error::InvalidConfig(format!("Failed to resize image: {}", e)))?;

        if let Some(result) = RgbaImage::from_raw(dst_width, dst_height, dst_data) {
            *buffer = result;
        }

        Ok(())
    }
}

impl VideoFilter for ZoomFilter {
    crate::impl_default_video_filter!(ZoomFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_time_offset = data.relative_timeline_offset;
        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_zoom(buffer, frame_time_offset)?;
            }
        }

        Ok(())
    }
}
