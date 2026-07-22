use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;

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
pub struct FisheyeFilter {
    /// Center X position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    /// Center Y position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    /// Distortion strength (-1.0 to 2.0)
    /// Positive values create convex/bulge effect (fisheye)
    /// Negative values create concave/pinch effect
    #[derivative(Default(value = "0.5"))]
    pub strength: f32,

    /// Influence radius in pixels (based on 1080p)
    /// Pixels beyond this radius are not affected
    #[derivative(Default(value = "200"))]
    pub radius: u32,

    /// Keyframe tracks for animatable properties
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

pub struct FisheyeValues {
    pub center_x: f32,
    pub center_y: f32,
    pub strength: f32,
    pub radius: u32,
}

impl FisheyeFilter {
    pub const NAME: &'static str = "fisheye";

    pub fn new(center_x: f32, center_y: f32, strength: f32, radius: u32) -> Self {
        Self {
            center_x,
            center_y,
            strength,
            radius,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("center_x", "Center X", 0.0, 1.0, 0.5),
            AnimatableProperty::float("center_y", "Center Y", 0.0, 1.0, 0.5),
            AnimatableProperty::float("strength", "Strength", -1.0, 2.0, 0.5),
            AnimatableProperty::float("radius", "Radius", 0.0, 5000.0, 200.0),
        ]
    }

    pub fn get_values_at_time(&self, time_ms: i64) -> FisheyeValues {
        let center_x = self
            .keyframe_tracks
            .get_track("center_x")
            .map(|track| get_float_at_time(track, time_ms, self.center_x))
            .unwrap_or(self.center_x);

        let center_y = self
            .keyframe_tracks
            .get_track("center_y")
            .map(|track| get_float_at_time(track, time_ms, self.center_y))
            .unwrap_or(self.center_y);

        let strength = self
            .keyframe_tracks
            .get_track("strength")
            .map(|track| get_float_at_time(track, time_ms, self.strength))
            .unwrap_or(self.strength);

        let radius = self
            .keyframe_tracks
            .get_track("radius")
            .map(|track| get_float_at_time(track, time_ms, self.radius as f32) as u32)
            .unwrap_or(self.radius);

        FisheyeValues {
            center_x,
            center_y,
            strength,
            radius,
        }
    }

    /// Bilinear interpolation sampling
    fn bilinear_sample(source: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
        let width = source.width();
        let height = source.height();

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);

        let dx = x - x0 as f64;
        let dy = y - y0 as f64;

        let p00 = source.get_pixel(x0, y0);
        let p01 = source.get_pixel(x1, y0);
        let p10 = source.get_pixel(x0, y1);
        let p11 = source.get_pixel(x1, y1);

        let interpolate = |v00: u8, v01: u8, v10: u8, v11: u8, dx: f64, dy: f64| -> u8 {
            let v0 = v00 as f64 * (1.0 - dx) + v01 as f64 * dx;
            let v1 = v10 as f64 * (1.0 - dx) + v11 as f64 * dx;
            ((v0 * (1.0 - dy) + v1 * dy).clamp(0.0, 255.0)) as u8
        };

        Rgba([
            interpolate(p00[0], p01[0], p10[0], p11[0], dx, dy),
            interpolate(p00[1], p01[1], p10[1], p11[1], dx, dy),
            interpolate(p00[2], p01[2], p10[2], p11[2], dx, dy),
            interpolate(p00[3], p01[3], p10[3], p11[3], dx, dy),
        ])
    }

    fn apply_fisheye(values: &FisheyeValues, buffer: &mut RgbaImage) -> Result<()> {
        let width = buffer.width();
        let height = buffer.height();

        if values.radius == 0 || values.strength == 0.0 {
            return Ok(());
        }

        let cx = (values.center_x * width as f32).round() as f64;
        let cy = (values.center_y * height as f32).round() as f64;
        let radius = values.radius as f64;
        let strength = values.strength as f64;

        // Create a copy of the original image for sampling
        let source = buffer.clone();

        // Process all pixels within the radius
        let x_start = ((cx - radius - 1.0).max(0.0) as u32).min(width);
        let x_end = ((cx + radius + 1.0).min(width as f64) as u32).min(width);
        let y_start = ((cy - radius - 1.0).max(0.0) as u32).min(height);
        let y_end = ((cy + radius + 1.0).min(height as f64) as u32).min(height);

        // Process rows in parallel
        let rows: Vec<Vec<[u8; 4]>> = (y_start..y_end)
            .into_par_iter()
            .map(|y| {
                (x_start..x_end)
                    .map(|x| {
                        // Calculate distance from center (using pixel center)
                        let px = x as f64 + 0.5;
                        let py = y as f64 + 0.5;
                        let dx = px - cx;
                        let dy = py - cy;
                        let dist = (dx * dx + dy * dy).sqrt();

                        // Only process pixels within the radius
                        if dist <= radius && dist > 0.0 {
                            // Normalize distance (0 to 1 within radius)
                            let normalized_dist = dist / radius;

                            // Apply fisheye/spherize transformation
                            let factor = 1.0 + strength * (1.0 - normalized_dist * normalized_dist);
                            let new_dist = dist * factor;

                            // Calculate source pixel position
                            let angle = dy.atan2(dx);
                            let src_x = cx + new_dist * angle.cos();
                            let src_y = cy + new_dist * angle.sin();

                            // Sample from source image with bilinear interpolation
                            if src_x >= 0.0
                                && src_x < width as f64 - 1.0
                                && src_y >= 0.0
                                && src_y < height as f64 - 1.0
                            {
                                Self::bilinear_sample(&source, src_x, src_y).0
                            } else {
                                // Handle boundary - sample with clamping
                                let clamped_x = src_x.clamp(0.0, (width - 1) as f64);
                                let clamped_y = src_y.clamp(0.0, (height - 1) as f64);
                                Self::bilinear_sample(&source, clamped_x, clamped_y).0
                            }
                        } else {
                            // Outside radius - keep original pixel
                            source.get_pixel(x, y).0
                        }
                    })
                    .collect()
            })
            .collect();

        // Apply results back to buffer
        for (y_idx, row) in rows.iter().enumerate() {
            let y = y_start + y_idx as u32;
            for (x_idx, pixel_data) in row.iter().enumerate() {
                let x = x_start + x_idx as u32;
                buffer.put_pixel(x, y, Rgba(*pixel_data));
            }
        }

        Ok(())
    }
}

impl VideoFilter for FisheyeFilter {
    crate::impl_default_video_filter!(FisheyeFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;
        let values = self.get_values_at_time(time_ms);

        let output_height = data.config.output_height;

        // Scale pixel values from base 1080p to target resolution
        let scaled_values = FisheyeValues {
            center_x: values.center_x,
            center_y: values.center_y,
            strength: values.strength,
            radius: scale_pixel_for_height(values.radius, output_height),
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_fisheye(&scaled_values, buffer)?;
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

        if let Some(track) = tracks.get_track("center_x")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value("center_x", time_ms, KeyframeValue::Float(self.center_x));
            updated = true;
        }

        if let Some(track) = tracks.get_track("center_y")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value("center_y", time_ms, KeyframeValue::Float(self.center_y));
            updated = true;
        }

        if let Some(track) = tracks.get_track("strength")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value("strength", time_ms, KeyframeValue::Float(self.strength));
            updated = true;
        }

        if let Some(track) = tracks.get_track("radius")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "radius",
                time_ms,
                KeyframeValue::Float(self.radius as f32),
            );
            updated = true;
        }

        updated
    }
}

