use crate::{
    Result,
    filters::{
        interpolation::get_float2_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{RgbaImage, imageops};
use video_utils::convert::resize_rgba_image_contain;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum CropShape {
    #[default]
    Rectangle = 0,
    Circle = 1,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CropFilter {
    pub left: f32,   // 左边缘偏移(0, 1)
    pub top: f32,    // 上边缘偏移(0, 1）
    pub width: f32,  // 裁剪宽度（0, 1）
    pub height: f32, // 裁剪高度（0, 1）
    pub shape: CropShape,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for CropFilter {
    fn default() -> Self {
        Self {
            left: 0.,
            top: 0.,
            width: 1.0,
            height: 1.0,
            shape: CropShape::default(),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl CropFilter {
    pub const NAME: &'static str = "crop";

    pub fn new(left: f32, top: f32, width: f32, height: f32, shape: CropShape) -> Self {
        Self {
            left,
            top,
            width,
            height,
            shape,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float2("region", "Position", 0.0, 1.0, 0.0, 0.0), // left, top
            AnimatableProperty::float2("size", "Size", 0.0, 1.0, 1.0, 1.0),       // width, height
        ]
    }

    /// Get interpolated values at a specific time
    fn get_values_at_time(&self, time_ms: i64) -> CropValues {
        let (left, top) = self
            .keyframe_tracks
            .get_track("region")
            .map(|track| get_float2_at_time(track, time_ms, self.left, self.top))
            .unwrap_or((self.left, self.top));

        let (width, height) = self
            .keyframe_tracks
            .get_track("size")
            .map(|track| get_float2_at_time(track, time_ms, self.width, self.height))
            .unwrap_or((self.width, self.height));

        CropValues {
            left: left.clamp(0.0, 1.0),
            top: top.clamp(0.0, 1.0),
            width: width.clamp(0.0, 1.0),
            height: height.clamp(0.0, 1.0),
            shape: self.shape,
        }
    }

    fn apply_crop_with_values(
        values: &CropValues,
        image: &mut RgbaImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<()> {
        let img_width = image.width();
        let img_height = image.height();

        // Convert normalized values (0.0-1.0) to pixel values
        let px_left = (values.left * img_width as f32).round() as u32;
        let px_top = (values.top * img_height as f32).round() as u32;
        let px_width = (values.width * img_width as f32).round() as u32;
        let px_height = (values.height * img_height as f32).round() as u32;

        // Clamp values to valid ranges
        let px_left = px_left.min(img_width);
        let px_top = px_top.min(img_height);
        let px_width = px_width.min(img_width.saturating_sub(px_left));
        let px_height = px_height.min(img_height.saturating_sub(px_top));

        if px_width == 0 || px_height == 0 {
            return Ok(());
        }

        match values.shape {
            CropShape::Rectangle => {
                let cropped = imageops::crop(image, px_left, px_top, px_width, px_height);
                let cropped_image = cropped.to_image();
                *image =
                    resize_rgba_image_contain(cropped_image, target_width, target_height, false)?;
            }
            CropShape::Circle => {
                Self::apply_circular_crop_with_values(
                    image,
                    px_left,
                    px_top,
                    px_width,
                    px_height,
                    target_width,
                    target_height,
                )?;
            }
        }

        Ok(())
    }

    fn apply_circular_crop_with_values(
        image: &mut RgbaImage,
        px_left: u32,
        px_top: u32,
        px_width: u32,
        px_height: u32,
        target_width: u32,
        target_height: u32,
    ) -> Result<()> {
        // 1. Crop the rectangular region first
        let cropped = imageops::crop(image, px_left, px_top, px_width, px_height);
        let mut cropped_image = cropped.to_image();

        // 2. Calculate circle parameters (use center of the cropped region)
        let cx = px_width as f32 / 2.0;
        let cy = px_height as f32 / 2.0;
        let radius = px_width.min(px_height) as f32 / 2.0;

        // 3. Apply circular mask (pixels outside circle become transparent)
        for y in 0..px_height {
            for x in 0..px_width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > radius {
                    // Outside circle: set transparent
                    let pixel = cropped_image.get_pixel_mut(x, y);
                    pixel[3] = 0; // alpha = 0
                } else if dist > radius - 1.0 {
                    // Edge: apply anti-aliasing
                    let alpha = (radius - dist).clamp(0.0, 1.0);
                    let pixel = cropped_image.get_pixel_mut(x, y);
                    pixel[3] = (pixel[3] as f32 * alpha) as u8;
                }
            }
        }

        // 4. Resize to target dimensions
        *image = resize_rgba_image_contain(cropped_image, target_width, target_height, false)?;

        Ok(())
    }
}

/// Interpolated crop values at a specific time
struct CropValues {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    shape: CropShape,
}

impl VideoFilter for CropFilter {
    crate::impl_default_video_filter!(CropFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let target_width = data.config.output_width;
        let target_height = data.config.output_height;

        // Calculate current time in milliseconds relative to segment start
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let values = self.get_values_at_time(time_ms);

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer } = frame {
                Self::apply_crop_with_values(&values, buffer, target_width, target_height)?;
            }
        }
        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        let mut updated = false;

        if let Some(track) = tracks.get_track("region")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "region",
                time_ms,
                KeyframeValue::Float2(self.left, self.top),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("size")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "size",
                time_ms,
                KeyframeValue::Float2(self.width, self.height),
            );
            updated = true;
        }

        updated
    }
}

