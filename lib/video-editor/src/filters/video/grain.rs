use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
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
pub struct GrainFilter {
    /// Grain intensity (0.0-1.0). Controls the amount of grain added to the image.
    #[derivative(Default(value = "0.3"))]
    pub intensity: f32,
    /// Grain particle size (1.0-10.0). Larger values create bigger, more visible grain particles.
    #[derivative(Default(value = "2.0"))]
    pub grain_size: f32,
    /// Whether to use colored grain (true) or monochrome grain (false).
    /// Colored grain adds separate noise to each RGB channel, monochrome adds the same noise.
    #[derivative(Default(value = "false"))]
    pub colored: bool,
    /// Grain roughness/clumping (0.0-1.0). Higher values create more contrasty, clumped grain.
    #[derivative(Default(value = "0.5"))]
    pub roughness: f32,
    /// Random seed for animation consistency. Same seed produces same grain pattern for a given frame.
    #[derivative(Default(value = "0"))]
    pub seed: u32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl GrainFilter {
    pub const NAME: &'static str = "grain";

    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: intensity.clamp(0.0, 1.0),
            grain_size: 2.0,
            colored: false,
            roughness: 0.5,
            seed: 0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("intensity", "Intensity", 0.0, 1.0, 0.3),
            AnimatableProperty::float("grain_size", "Grain Size", 1.0, 10.0, 2.0),
            AnimatableProperty::float("roughness", "Roughness", 0.0, 1.0, 0.5),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Hash-based pseudo-random number generator for deterministic grain.
    /// Returns a value in range [-1.0, 1.0].
    #[inline]
    fn hash_noise(x: i32, y: i32, seed: u32, channel: u32) -> f32 {
        // Simple hash function combining position, seed, and channel
        // Convert i32 to u32 for hash operations
        let x_u = x as u32;
        let y_u = y as u32;
        let mut h = seed;
        h ^= x_u.wrapping_mul(374761393);
        h ^= y_u.wrapping_mul(668265263);
        h ^= channel.wrapping_mul(1274126177);
        h = (h ^ (h >> 13)).wrapping_mul(1274126177);

        // Convert to float in range [-1, 1]
        let normalized = (h & 0x7FFFFFFF) as f32 / 0x7FFFFFFF as f32;
        normalized * 2.0 - 1.0
    }

    /// Smooth noise interpolation using bilinear filtering.
    /// Creates smoother grain patterns based on grain_size.
    #[inline]
    fn smooth_noise(
        x: f32,
        y: f32,
        grain_size: f32,
        seed: u32,
        channel: u32,
    ) -> f32 {
        // Scale coordinates by grain_size
        let sx = x / grain_size;
        let sy = y / grain_size;

        // Get integer coordinates for the 4 corners
        let x0 = sx.floor() as i32;
        let y0 = sy.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;

        // Get interpolation weights
        let fx = sx - x0 as f32;
        let fy = sy - y0 as f32;

        // Get noise values at 4 corners
        let n00 = Self::hash_noise(x0, y0, seed, channel);
        let n10 = Self::hash_noise(x1, y0, seed, channel);
        let n01 = Self::hash_noise(x0, y1, seed, channel);
        let n11 = Self::hash_noise(x1, y1, seed, channel);

        // Bilinear interpolation
        let nx0 = n00 * (1.0 - fx) + n10 * fx;
        let nx1 = n01 * (1.0 - fx) + n11 * fx;

        nx0 * (1.0 - fy) + nx1 * fy
    }

    /// Apply roughness modulation to noise value.
    /// Higher roughness creates more contrasty grain.
    #[inline]
    fn apply_roughness(noise: f32, roughness: f32) -> f32 {
        // roughness = 0.0: smooth, gentle noise
        // roughness = 1.0: harsh, high contrast noise
        if roughness < 0.5 {
            // Smooth transition: reduce contrast
            let smooth_factor = 1.0 - roughness * 2.0;
            noise * (0.5 + smooth_factor * 0.5)
        } else {
            // Harsh transition: increase contrast and clamp
            let harsh_factor = (roughness - 0.5) * 2.0;
            // Apply a curve that increases contrast
            let sign = if noise >= 0.0 { 1.0 } else { -1.0 };
            let abs_noise = noise.abs();
            sign * (abs_noise + harsh_factor * abs_noise * abs_noise)
        }
    }

    /// Apply grain to an image buffer (parallelized by row).
    fn apply_grain(
        buffer: &mut image::RgbaImage,
        grain_size: f32,
        time_seed: u32,
        intensity: f32,
        roughness: f32,
        colored: bool,
    ) {
        let width = buffer.width();
        let height = buffer.height();

        // Maximum grain amount (in 0-255 range)
        let max_grain = intensity * 50.0; // Scale intensity to visible grain amount

        // Process rows in parallel
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let px = x as f32;
                        let py = y as f32;

                        // Get original pixel
                        let original = buffer.get_pixel(x, y);

                        if colored {
                            // Colored grain: separate noise for each channel
                            let noise_r = Self::smooth_noise(px, py, grain_size, time_seed, 0);
                            let noise_g = Self::smooth_noise(px, py, grain_size, time_seed, 1);
                            let noise_b = Self::smooth_noise(px, py, grain_size, time_seed, 2);

                            let grain_r = Self::apply_roughness(noise_r, roughness) * max_grain;
                            let grain_g = Self::apply_roughness(noise_g, roughness) * max_grain;
                            let grain_b = Self::apply_roughness(noise_b, roughness) * max_grain;

                            let r = ((original.0[0] as f32 + grain_r).clamp(0.0, 255.0)) as u8;
                            let g = ((original.0[1] as f32 + grain_g).clamp(0.0, 255.0)) as u8;
                            let b = ((original.0[2] as f32 + grain_b).clamp(0.0, 255.0)) as u8;

                            [r, g, b, original.0[3]]
                        } else {
                            // Monochrome grain: same noise for all channels
                            let noise = Self::smooth_noise(px, py, grain_size, time_seed, 0);
                            let grain = Self::apply_roughness(noise, roughness) * max_grain;

                            let r = ((original.0[0] as f32 + grain).clamp(0.0, 255.0)) as u8;
                            let g = ((original.0[1] as f32 + grain).clamp(0.0, 255.0)) as u8;
                            let b = ((original.0[2] as f32 + grain).clamp(0.0, 255.0)) as u8;

                            [r, g, b, original.0[3]]
                        }
                    })
                    .collect()
            })
            .collect();

        // Flatten and rebuild image
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        *buffer = image::RgbaImage::from_raw(width, height, pixels)
            .expect("Buffer size matches image dimensions");
    }
}

impl VideoFilter for GrainFilter {
    crate::impl_default_video_filter!(GrainFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let intensity = self
            .get_value_at_time(time_ms, "intensity", self.intensity)
            .clamp(0.0, 1.0);
        let grain_size = self
            .get_value_at_time(time_ms, "grain_size", self.grain_size)
            .clamp(1.0, 10.0);
        let roughness = self
            .get_value_at_time(time_ms, "roughness", self.roughness)
            .clamp(0.0, 1.0);

        // Create time-based seed for animation consistency
        // The seed incorporates the frame time so that grain animates over time
        // but remains consistent for the same frame
        let time_seed = self.seed.wrapping_add(time_ms as u32);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_grain(
                    buffer,
                    grain_size,
                    time_seed,
                    intensity,
                    roughness,
                    self.colored,
                );
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
            ("intensity", self.intensity),
            ("grain_size", self.grain_size),
            ("roughness", self.roughness),
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