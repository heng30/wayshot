use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{EasingFunction, VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage, imageops};
use rayon::prelude::*;
use std::time::Duration;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum FlyInDirection {
    FromLeft = 0,
    FromRight = 1,
    FromTop = 2,
    FromBottom = 3,
}

impl Default for FlyInDirection {
    fn default() -> Self {
        Self::FromLeft
    }
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
pub struct FlyInFilter {
    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub duration: Duration,

    #[derivative(Default(value = "FlyInDirection::FromLeft"))]
    pub direction: FlyInDirection,

    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub move_to_position: (f32, f32),

    #[derivative(Default(value = "EasingFunction::Linear"))]
    pub easing: EasingFunction,
}

impl EasingFunction {
    fn apply(&self, t: f32) -> f32 {
        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => t * (2.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

impl FlyInFilter {
    pub const NAME: &'static str = "fly in";

    pub fn new(
        duration: Duration,
        direction: FlyInDirection,
        move_to_position: (f32, f32),
    ) -> Self {
        Self {
            duration,
            direction,
            move_to_position,
            easing: EasingFunction::Linear,
        }
    }

    /// Calculate the entry start position based on the specified direction.
    /// Returns the starting position (image center just outside the canvas).
    fn calculate_entry_position(direction: FlyInDirection, move_to: (f32, f32)) -> (f32, f32) {
        let (target_x, target_y) = move_to;

        match direction {
            FlyInDirection::FromLeft => (-0.5, target_y),
            FlyInDirection::FromRight => (1.5, target_y),
            FlyInDirection::FromTop => (target_x, -0.5),
            FlyInDirection::FromBottom => (target_x, 1.5),
        }
    }

    fn apply_move(
        &self,
        buffer: &mut RgbaImage,
        canvas_width: u32,
        canvas_height: u32,
        frame_time_offset: Duration,
    ) -> Result<()> {
        // Ensure buffer is canvas-sized (decoded frames may be smaller than canvas)
        if buffer.width() != canvas_width || buffer.height() != canvas_height {
            let mut canvas = RgbaImage::new(canvas_width, canvas_height);
            let x = (canvas_width.saturating_sub(buffer.width())) / 2;
            let y = (canvas_height.saturating_sub(buffer.height())) / 2;
            imageops::overlay(&mut canvas, buffer, x as i64, y as i64);
            *buffer = canvas;
        }

        // Calculate ratio - after duration, stay at final position (ratio = 1.0)
        let ratio = if frame_time_offset > self.duration {
            1.0
        } else {
            progress_ratio_from_offset(frame_time_offset, self.duration)
        };
        let eased_ratio = self.easing.apply(ratio);

        // Use canvas dimensions for animation calculations
        let width = canvas_width;
        let height = canvas_height;

        // Calculate fly-in start position (image center just outside canvas edge)
        let start_pos = Self::calculate_entry_position(self.direction, self.move_to_position);
        let end_pos = self.move_to_position;

        // Calculate current image center position
        let current_center_x = start_pos.0 + (end_pos.0 - start_pos.0) * eased_ratio;
        let current_center_y = start_pos.1 + (end_pos.1 - start_pos.1) * eased_ratio;

        // Calculate pixel offset (relative to image at canvas center)
        // When image at canvas center, offset is 0
        // Current offset = (current_center - 0.5) * canvas_size
        let offset_x = ((current_center_x - 0.5) * width as f32).round() as i32;
        let offset_y = ((current_center_y - 0.5) * height as f32).round() as i32;

        let original = buffer.clone();
        let mut result = RgbaImage::new(width, height);

        result.par_enumerate_pixels_mut().for_each(|(x, y, pixel)| {
            let src_x = x as i32 - offset_x;
            let src_y = y as i32 - offset_y;

            *pixel = if src_x >= 0 && src_x < width as i32 && src_y >= 0 && src_y < height as i32 {
                *original.get_pixel(src_x as u32, src_y as u32)
            } else {
                Rgba([0, 0, 0, 0])
            };
        });

        *buffer = result;
        Ok(())
    }
}

impl VideoFilter for FlyInFilter {
    crate::impl_default_video_filter!(FlyInFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false // Operate on image_for_composite stream, consistent with transform
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let canvas_width = data.config.output_width;
        let canvas_height = data.config.output_height;
        let frame_time_offset = data.relative_timeline_offset;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_move(buffer, canvas_width, canvas_height, frame_time_offset)?;
            }
        }

        Ok(())
    }
}
