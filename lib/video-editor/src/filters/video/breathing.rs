use crate::{
    Error, Result,
    filters::traits::{VideoData, VideoFilter},
    tracks::video_frame_cache::VideoImage,
};
use fast_image_resize::{PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image as FastImage};
use image::RgbaImage;
use std::time::Duration;

/// Breathing curve type for controlling the animation feel
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum BreathingCurve {
    /// Linear interpolation - constant speed (may feel mechanical)
    Linear,
    /// Ease-in-out using cosine - smooth acceleration/deceleration (default, natural feel)
    #[default]
    EaseInOut,
}

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
pub struct BreathingFilter {
    /// Duration of one breathing cycle (how fast it breathes)
    #[derivative(Default(value = "Duration::from_secs(2)"))]
    pub breathing_duration: Duration,

    /// Minimum scale during breathing (0.1-1.0, 0.95)
    #[derivative(Default(value = "0.95"))]
    pub min_scale: f32,

    /// Center X position for scaling (0.0-1.0, default 0.5)
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    /// Center Y position for scaling (0.0-1.0, default 0.5)
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    /// Breathing curve type (linear or ease-in-out)
    #[derivative(Default)]
    pub curve: BreathingCurve,
}

impl BreathingFilter {
    pub const NAME: &'static str = "breathing";

    pub fn new(breathing_duration: Duration, min_scale: f32) -> Self {
        Self {
            breathing_duration,
            min_scale: min_scale.clamp(0.1, 1.0),
            center_x: 0.5,
            center_y: 0.5,
            curve: BreathingCurve::default(),
        }
    }

    pub fn with_center(mut self, x: f32, y: f32) -> Self {
        self.center_x = x.clamp(0.0, 1.0);
        self.center_y = y.clamp(0.0, 1.0);
        self
    }

    /// Calculate the current scale based on time offset
    /// The scale oscillates between min_scale and 1.0, starting from original size
    fn get_scale_at_time(&self, frame_time_offset: Duration) -> f32 {
        let cycle_progress = if self.breathing_duration.is_zero() {
            0.0
        } else {
            (frame_time_offset.as_secs_f32() % self.breathing_duration.as_secs_f32())
                / self.breathing_duration.as_secs_f32()
        };

        match self.curve {
            BreathingCurve::Linear => {
                // Linear: from original size -> min_scale -> original size
                // Uses triangle wave
                let triangle = if cycle_progress < 0.5 {
                    1.0 - 2.0 * cycle_progress // 1.0 -> 0.0
                } else {
                    2.0 * cycle_progress - 1.0 // 0.0 -> 1.0
                };
                self.min_scale + (1.0 - self.min_scale) * triangle
            }
            BreathingCurve::EaseInOut => {
                // Cosine ease-in-out: starts from original size with smooth acceleration/deceleration
                // cos(0)=1, cos(π)=-1, cos(2π)=1
                let cos_value = (cycle_progress * std::f32::consts::TAU).cos();
                let mid_point = (1.0 + self.min_scale) / 2.0;
                let amplitude = (1.0 - self.min_scale) / 2.0;
                mid_point + amplitude * cos_value
            }
        }
    }

    fn apply_breathing(&self, buffer: &mut RgbaImage, frame_time_offset: Duration) -> Result<()> {
        let current_scale = self.get_scale_at_time(frame_time_offset);

        let (src_width, src_height) = (buffer.width(), buffer.height());
        let (dst_width, dst_height) = (buffer.width(), buffer.height());

        // Use f64 for crop size calculations to avoid integer jump artifacts
        let crop_width = src_width as f64 * current_scale as f64;
        let crop_height = src_height as f64 * current_scale as f64;

        if crop_width < 1.0 || crop_height < 1.0 || crop_width > src_width as f64 || crop_height > src_height as f64
        {
            return Ok(());
        }

        // Calculate crop center position in source image using f64
        let crop_center_x = src_width as f64 * self.center_x as f64;
        let crop_center_y = src_height as f64 * self.center_y as f64;

        // Calculate top-left position from center point, using f64
        let x_offset = (crop_center_x - crop_width / 2.0).clamp(0.0, src_width as f64 - crop_width);
        let y_offset = (crop_center_y - crop_height / 2.0).clamp(0.0, src_height as f64 - crop_height);

        let src_data = buffer.clone().into_raw();
        let src_image = FastImage::from_vec_u8(src_width, src_height, src_data, PixelType::U8x4)
            .map_err(|e| Error::InvalidConfig(format!("Failed to create source image: {}", e)))?;

        let mut dst_data = vec![0u8; (dst_width * dst_height * 4) as usize];
        let mut dst_image =
            FastImage::from_slice_u8(dst_width, dst_height, &mut dst_data, PixelType::U8x4)
                .map_err(|e| {
                    Error::InvalidConfig(format!("Failed to create destination image: {}", e))
                })?;

        // Use f64 crop parameters directly to avoid integer jump artifacts
        let resize_options = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(
                fast_image_resize::FilterType::Lanczos3,
            ))
            .crop(x_offset, y_offset, crop_width, crop_height);

        Resizer::new()
            .resize(&src_image, &mut dst_image, &resize_options)
            .map_err(|e| Error::InvalidConfig(format!("Failed to resize image: {}", e)))?;

        if let Some(result) = RgbaImage::from_raw(dst_width, dst_height, dst_data) {
            *buffer = result;
        }

        Ok(())
    }
}

impl VideoFilter for BreathingFilter {
    crate::impl_default_video_filter!(BreathingFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_time_offset = data.relative_timeline_offset;
        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_breathing(buffer, frame_time_offset)?;
            }
        }

        Ok(())
    }
}
