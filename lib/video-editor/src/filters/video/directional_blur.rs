use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::RgbaImage;
use rayon::prelude::*;

/// Directional blur filter that blurs along a specified angle.
/// Creates motion blur effect in any direction.
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
pub struct DirectionalBlurFilter {
    /// Blur direction angle in degrees (0.0-360.0). 0 = horizontal right, 90 = vertical down.
    #[derivative(Default(value = "0.0"))]
    pub angle: f32,
    /// Blur length/distance (0.0-100.0). Longer values create more pronounced blur.
    #[derivative(Default(value = "10.0"))]
    pub length: f32,
    /// Blur spread/softness (0.0-1.0). Higher values create softer, more distributed blur.
    #[derivative(Default(value = "0.5"))]
    pub spread: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl DirectionalBlurFilter {
    pub const NAME: &'static str = "directional blur";

    pub fn new(angle: f32, length: f32) -> Self {
        Self {
            angle: angle % 360.0,
            length: length.clamp(0.0, 100.0),
            spread: 0.5,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("angle", "Angle", 0.0, 360.0, 0.0),
            AnimatableProperty::float("length", "Length", 0.0, 100.0, 10.0),
            AnimatableProperty::float("spread", "Spread", 0.0, 1.0, 0.5),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Apply directional blur along the specified angle (parallelized)
    fn apply_directional_blur(image: &mut RgbaImage, angle: f32, length: f32, spread: f32) {
        if length <= 0.0 {
            return;
        }

        let (width, height) = image.dimensions();

        // Convert angle to direction vector
        let angle_rad = angle * std::f32::consts::PI / 180.0;
        let dir_x = angle_rad.cos();
        let dir_y = angle_rad.sin();

        // Calculate sample count based on length
        let sample_count = (length.ceil() as usize).max(2);
        let half_length = length / 2.0;

        // Pre-compute all pixel values in parallel, row by row
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut r = 0.0;
                        let mut g = 0.0;
                        let mut b = 0.0;
                        let mut a = 0.0;
                        let mut weight_sum = 0.0;

                        // Sample along the blur direction
                        for i in 0..sample_count {
                            // Offset from center (-half_length to +half_length)
                            let offset = (i as f32 / (sample_count - 1) as f32 - 0.5) * length;

                            // Calculate sample position
                            let sample_x = x as f32 + dir_x * offset;
                            let sample_y = y as f32 + dir_y * offset;

                            // Clamp to image bounds
                            let px = sample_x.clamp(0.0, width as f32 - 1.0) as u32;
                            let py = sample_y.clamp(0.0, height as f32 - 1.0) as u32;

                            // Calculate weight based on spread
                            // spread = 0: uniform weights
                            // spread = 1: gaussian-like distribution
                            let normalized_offset = offset / half_length;
                            let weight = if spread > 0.0 {
                                // Gaussian-like weight
                                (-normalized_offset * normalized_offset / (2.0 * spread * spread))
                                    .exp()
                            } else {
                                1.0
                            };

                            let pixel = image.get_pixel(px, py);
                            r += pixel.0[0] as f32 * weight;
                            g += pixel.0[1] as f32 * weight;
                            b += pixel.0[2] as f32 * weight;
                            a += pixel.0[3] as f32 * weight;
                            weight_sum += weight;
                        }

                        // Normalize and return result
                        if weight_sum > 0.0 {
                            [
                                (r / weight_sum).clamp(0.0, 255.0) as u8,
                                (g / weight_sum).clamp(0.0, 255.0) as u8,
                                (b / weight_sum).clamp(0.0, 255.0) as u8,
                                (a / weight_sum).clamp(0.0, 255.0) as u8,
                            ]
                        } else {
                            image.get_pixel(x, y).0
                        }
                    })
                    .collect()
            })
            .collect();

        // Flatten and build result image
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        *image = RgbaImage::from_raw(width, height, pixels)
            .expect("Buffer size matches image dimensions");
    }
}

impl VideoFilter for DirectionalBlurFilter {
    crate::impl_default_video_filter!(DirectionalBlurFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let angle = self.get_value_at_time(time_ms, "angle", self.angle) % 360.0;
        let length = self
            .get_value_at_time(time_ms, "length", self.length)
            .clamp(0.0, 100.0);
        let spread = self
            .get_value_at_time(time_ms, "spread", self.spread)
            .clamp(0.0, 1.0);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_directional_blur(buffer, angle, length, spread);
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

        for (property, value) in [
            ("angle", self.angle),
            ("length", self.length),
            ("spread", self.spread),
        ] {
            if let Some(track) = tracks.get_track(property)
                && track.keyframes.iter().any(|k| k.time_ms == time_ms)
            {
                tracks.update_keyframe_value(property, time_ms, KeyframeValue::Float(value));
                updated = true;
            }
        }

        updated
    }
}
