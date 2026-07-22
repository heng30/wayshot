use crate::{
    Error, Result,
    filters::{
        interpolation::{get_float_at_time, get_float2_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter, VideoFilterConfig},
    },
    tracks::video_frame_cache::VideoImage,
};
use fast_image_resize::{
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image as FastImage,
};
use image::{Rgba, RgbaImage, imageops};
use imageproc::geometric_transformations::{Interpolation, rotate_about_center};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransformFilter {
    // Static values (used as default/fallback when no keyframes)
    // Zoom level (1.0 = original size, 0.5 = 50% size, 2.0 = 200% size)
    pub zoom_level: f32,

    // Center X position as percentage of canvas (unlimited, 0.5 = center)
    pub center_x_percent: f32,

    // Center Y position as percentage of canvas (unlimited, 0.5 = center)
    pub center_y_percent: f32,

    // Rotation in radians (positive = clockwise)
    pub rotation: f32,

    // Keyframe tracks (optional, empty by default)
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for TransformFilter {
    fn default() -> Self {
        Self {
            zoom_level: 1.0,
            center_x_percent: 0.5,
            center_y_percent: 0.5,
            rotation: 0.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl TransformFilter {
    pub const NAME: &'static str = "transform";

    pub fn new(
        zoom_level: f32,
        center_x_percent: f32,
        center_y_percent: f32,
        rotation: f32,
    ) -> Self {
        Self {
            zoom_level: zoom_level.clamp(0.01, 10.0),
            center_x_percent,
            center_y_percent,
            rotation,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Create a filter with keyframe tracks
    pub fn with_keyframes(
        zoom_level: f32,
        center_x_percent: f32,
        center_y_percent: f32,
        rotation: f32,
        keyframe_tracks: KeyframeTracks,
    ) -> Self {
        Self {
            zoom_level: zoom_level.clamp(0.01, 10.0),
            center_x_percent,
            center_y_percent,
            rotation,
            keyframe_tracks,
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("zoom_level", "Zoom", 0.01, 10.0, 1.0),
            AnimatableProperty::float2("center_percent", "Position", -100.0, 100.0, 0.5, 0.5),
            AnimatableProperty::float(
                "rotation",
                "Rotation",
                -std::f32::consts::PI,
                std::f32::consts::PI,
                0.0,
            ),
        ]
    }

    /// Get interpolated values at a specific time
    fn get_values_at_time(&self, time_ms: i64) -> TransformValues {
        let zoom = self
            .keyframe_tracks
            .get_track("zoom_level")
            .map(|track| get_float_at_time(track, time_ms, self.zoom_level))
            .unwrap_or(self.zoom_level);

        let (center_x, center_y) = self
            .keyframe_tracks
            .get_track("center_percent")
            .map(|track| {
                get_float2_at_time(track, time_ms, self.center_x_percent, self.center_y_percent)
            })
            .unwrap_or((self.center_x_percent, self.center_y_percent));

        let rotation = self
            .keyframe_tracks
            .get_track("rotation")
            .map(|track| get_float_at_time(track, time_ms, self.rotation.to_degrees()).to_radians())
            .unwrap_or(self.rotation);

        TransformValues {
            zoom_level: zoom.clamp(0.01, 10.0),
            center_x_percent: center_x,
            center_y_percent: center_y,
            rotation,
        }
    }

    fn apply_transform_with_values(
        values: &TransformValues,
        buffer: &mut RgbaImage,
        config: &VideoFilterConfig,
    ) -> Result<()> {
        let src_width = buffer.width();
        let src_height = buffer.height();
        let output_width = config.output_width;
        let output_height = config.output_height;

        // Skip if no transformation needed
        if values.zoom_level == 1.0
            && values.rotation == 0.0
            && values.center_x_percent == 0.5
            && values.center_y_percent == 0.5
            && src_width == output_width
            && src_height == output_height
        {
            return Ok(());
        }

        // Step 1: Scale using fast_image_resize (SIMD-optimized)
        let scaled_width = (src_width as f32 * values.zoom_level).round() as u32;
        let scaled_height = (src_height as f32 * values.zoom_level).round() as u32;

        // Ensure minimum size of 1 pixel
        let scaled_width = scaled_width.max(1);
        let scaled_height = scaled_height.max(1);

        let mut scaled_image = if values.zoom_level != 1.0 {
            // Convert buffer to fast_image_resize format (avoids clone when possible)
            let src_data = buffer.clone().into_raw();
            let src_image =
                FastImage::from_vec_u8(src_width, src_height, src_data, PixelType::U8x4).map_err(
                    |e| Error::InvalidConfig(format!("Failed to create source image: {}", e)),
                )?;

            let mut dst_data = vec![0u8; (scaled_width * scaled_height * 4) as usize];
            let mut dst_image = FastImage::from_slice_u8(
                scaled_width,
                scaled_height,
                &mut dst_data,
                PixelType::U8x4,
            )
            .map_err(|e| {
                Error::InvalidConfig(format!("Failed to create destination image: {}", e))
            })?;

            let resize_options =
                ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

            Resizer::new()
                .resize(&src_image, &mut dst_image, &resize_options)
                .map_err(|e| Error::InvalidConfig(format!("Failed to resize image: {}", e)))?;

            RgbaImage::from_raw(scaled_width, scaled_height, dst_data)
                .unwrap_or_else(|| RgbaImage::new(scaled_width, scaled_height))
        } else {
            buffer.clone()
        };

        // Step 2: Rotate the image around its center
        if values.rotation != 0.0 {
            let theta = values.rotation;
            let w = scaled_image.width() as f32;
            let h = scaled_image.height() as f32;

            let cos_theta = theta.cos().abs();
            let sin_theta = theta.sin().abs();
            let rotated_w = (w * cos_theta + h * sin_theta).ceil();
            let rotated_h = (w * sin_theta + h * cos_theta).ceil();

            // The expanded canvas must be at least as large as the original image
            // in each dimension, otherwise the un-rotated image gets clipped when
            // overlaid onto the canvas before rotation.
            let expanded_width = rotated_w.max(w) as u32;
            let expanded_height = rotated_h.max(h) as u32;

            // Create transparent canvas with expanded size
            let mut expanded_canvas =
                RgbaImage::from_pixel(expanded_width, expanded_height, Rgba([0, 0, 0, 0]));

            // Place scaled image in center of expanded canvas
            let offset_x = (expanded_width as i64 - scaled_image.width() as i64) / 2;
            let offset_y = (expanded_height as i64 - scaled_image.height() as i64) / 2;
            imageops::overlay(&mut expanded_canvas, &scaled_image, offset_x, offset_y);

            // Now rotate - all content will fit within the expanded canvas
            scaled_image = rotate_about_center::<Rgba<u8>>(
                &expanded_canvas,
                theta,
                Interpolation::Bilinear,
                imageproc::geometric_transformations::Border::Constant(Rgba([0, 0, 0, 0])),
            );
        }

        // Step 3: Create output canvas and position the image
        let mut canvas = RgbaImage::from_pixel(output_width, output_height, Rgba([0, 0, 0, 0]));

        // Calculate the position where the image center should be placed
        let canvas_center_x = output_width as f32 * values.center_x_percent;
        let canvas_center_y = output_height as f32 * values.center_y_percent;

        // Calculate top-left position for the image
        let image_width = scaled_image.width();
        let image_height = scaled_image.height();
        let x = (canvas_center_x - image_width as f32 / 2.0).round() as i64;
        let y = (canvas_center_y - image_height as f32 / 2.0).round() as i64;

        // Overlay the transformed image onto the canvas
        imageops::overlay(&mut canvas, &scaled_image, x, y);

        *buffer = canvas;

        Ok(())
    }
}

/// Interpolated transform values at a specific time
struct TransformValues {
    zoom_level: f32,
    center_x_percent: f32,
    center_y_percent: f32,
    rotation: f32,
}

impl VideoFilter for TransformFilter {
    crate::impl_default_video_filter!(TransformFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let config = data.config.clone();

        // Calculate current time in milliseconds relative to segment start
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let values = self.get_values_at_time(time_ms);

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer } = frame {
                _ = Self::apply_transform_with_values(&values, buffer, &config);
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

        if let Some(track) = tracks.get_track("zoom_level")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "zoom_level",
                time_ms,
                KeyframeValue::Float(self.zoom_level),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("center_percent")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "center_percent",
                time_ms,
                KeyframeValue::Float2(self.center_x_percent, self.center_y_percent),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("rotation")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "rotation",
                time_ms,
                KeyframeValue::Float(self.rotation.to_degrees()),
            );
            updated = true;
        }

        updated
    }
}
