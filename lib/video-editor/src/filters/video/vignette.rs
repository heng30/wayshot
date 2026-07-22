use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VignetteFilter {
    /// Intensity of the vignette effect (0.0 = no effect, 1.0 = maximum darkening to black)
    pub intensity: f32,
    /// Inner radius as a fraction from center (0.0-1.0). Pixels within this distance are fully bright.
    /// The vignette darkening starts at this radius.
    pub inner_radius: f32,
    /// Outer radius as a fraction from center (0.0-1.0). Pixels beyond this distance are fully darkened.
    /// Should be >= inner_radius. The vignette transitions from inner to outer radius.
    pub outer_radius: f32,
    /// Center X position (0.0-1.0, 0.5 = center of image)
    pub center_x: f32,
    /// Center Y position (0.0-1.0, 0.5 = center of image)
    pub center_y: f32,
    /// Aspect ratio adjustment (1.0 = circular vignette, use image aspect ratio for elliptical)
    #[serde(default = "default_aspect")]
    pub aspect: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

fn default_aspect() -> f32 {
    1.0
}

impl Default for VignetteFilter {
    fn default() -> Self {
        Self {
            intensity: 0.8,
            inner_radius: 0.3,
            outer_radius: 0.8,
            center_x: 0.5,
            center_y: 0.5,
            aspect: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl VignetteFilter {
    pub const NAME: &'static str = "vignette";

    pub fn new(intensity: f32, inner_radius: f32, outer_radius: f32) -> Self {
        Self {
            intensity: intensity.clamp(0.0, 1.0),
            inner_radius: inner_radius.clamp(0.0, 1.0),
            outer_radius: outer_radius.clamp(0.0, 1.0).max(inner_radius),
            center_x: 0.5,
            center_y: 0.5,
            aspect: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn with_center(mut self, x: f32, y: f32) -> Self {
        self.center_x = x.clamp(0.0, 1.0);
        self.center_y = y.clamp(0.0, 1.0);
        self
    }

    pub fn with_aspect(mut self, aspect: f32) -> Self {
        self.aspect = aspect.max(0.1);
        self
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("intensity", "Intensity", 0.0, 1.0, 0.8),
            AnimatableProperty::float("inner_radius", "Inner Radius", 0.0, 1.0, 0.3),
            AnimatableProperty::float("outer_radius", "Outer Radius", 0.0, 1.0, 0.8),
            AnimatableProperty::float("center_x", "Center X", 0.0, 1.0, 0.5),
            AnimatableProperty::float("center_y", "Center Y", 0.0, 1.0, 0.5),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    fn calculate_vignette_factor(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        center_x: f32,
        center_y: f32,
        inner_radius: f32,
        outer_radius: f32,
        intensity: f32,
        aspect: f32,
    ) -> f32 {
        let nx = x / width;
        let ny = y / height;

        let dx = nx - center_x;
        let dy = ny - center_y;

        let adjusted_dy = dy * aspect;
        let dist = (dx * dx + adjusted_dy * adjusted_dy).sqrt();

        // Calculate vignette factor:
        // - dist <= inner_radius: fully bright (factor = 1.0)
        // - dist >= outer_radius: fully darkened (factor = 1.0 - intensity)
        // - in between: smooth transition
        if dist <= inner_radius {
            1.0
        } else if dist >= outer_radius {
            1.0 - intensity
        } else {
            // Smooth interpolation between inner and outer radius
            let t = (dist - inner_radius) / (outer_radius - inner_radius);
            // Use smoothstep for nicer falloff
            let smooth_t = t * t * (3.0 - 2.0 * t);
            1.0 - intensity * smooth_t
        }
    }
}

impl VideoFilter for VignetteFilter {
    crate::impl_default_video_filter!(VignetteFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let intensity = self
            .get_value_at_time(time_ms, "intensity", self.intensity)
            .clamp(0.0, 1.0);
        let inner_radius = self
            .get_value_at_time(time_ms, "inner_radius", self.inner_radius)
            .clamp(0.0, 1.0);
        let outer_radius = self
            .get_value_at_time(time_ms, "outer_radius", self.outer_radius)
            .clamp(0.0, 1.0)
            .max(inner_radius);
        let center_x = self
            .get_value_at_time(time_ms, "center_x", self.center_x)
            .clamp(0.0, 1.0);
        let center_y = self
            .get_value_at_time(time_ms, "center_y", self.center_y)
            .clamp(0.0, 1.0);

        let width_aspect = if let Some(frame) = data.frames.first() {
            if let VideoImage::Image { buffer, .. } = frame {
                buffer.width() as f32 / buffer.height() as f32
            } else {
                1.0
            }
        } else {
            1.0
        };

        let aspect = if self.aspect != 1.0 {
            self.aspect
        } else {
            width_aspect
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let width = buffer.width() as f32;
                let height = buffer.height() as f32;

                for (x, y, pixel) in buffer.enumerate_pixels_mut() {
                    let px = x as f32;
                    let py = y as f32;

                    let vignette_factor = Self::calculate_vignette_factor(
                        px,
                        py,
                        width,
                        height,
                        center_x,
                        center_y,
                        inner_radius,
                        outer_radius,
                        intensity,
                        aspect,
                    );

                    pixel.0[0] = ((pixel.0[0] as f32) * vignette_factor).clamp(0.0, 255.0) as u8;
                    pixel.0[1] = ((pixel.0[1] as f32) * vignette_factor).clamp(0.0, 255.0) as u8;
                    pixel.0[2] = ((pixel.0[2] as f32) * vignette_factor).clamp(0.0, 255.0) as u8;
                }
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
            ("inner_radius", self.inner_radius),
            ("outer_radius", self.outer_radius),
            ("center_x", self.center_x),
            ("center_y", self.center_y),
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

