use crate::{
    Result,
    filters::{
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};

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
#[serde(default)]
#[non_exhaustive]
pub struct LinearMaskFilter {
    /// Line center X (0.0-1.0, default 0.5)
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,
    /// Line center Y (0.0-1.0, default 0.5)
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,
    /// Rotation in degrees (0.0-360.0, default 0.0)
    pub rotation: f32,
    /// Edge softness / feather (0.0-1.0, default 0.0)
    pub feather: f32,
    /// Mask opacity (0.0-1.0, default 1.0)
    #[derivative(Default(value = "1.0"))]
    pub opacity: f32,
    /// Flip mask side (default false — left side masked)
    pub flip: bool,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty")]
    pub keyframe_tracks: KeyframeTracks,
}

impl LinearMaskFilter {
    pub const NAME: &'static str = "linear mask";

    pub fn new(center_x: f32, center_y: f32, rotation: f32, feather: f32, opacity: f32) -> Self {
        Self {
            center_x,
            center_y,
            rotation,
            feather,
            opacity,
            ..Default::default()
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("center_x", "Center X", 0.0, 1.0, 0.5),
            AnimatableProperty::float("center_y", "Center Y", 0.0, 1.0, 0.5),
            AnimatableProperty::float("rotation", "Rotation", 0.0, 360.0, 0.0),
            AnimatableProperty::float("feather", "Feather", 0.0, 1.0, 0.0),
            AnimatableProperty::float("opacity", "Opacity", 0.0, 1.0, 1.0),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| crate::filters::interpolation::get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    fn get_center_at_time(&self, time_ms: i64) -> (f32, f32) {
        let cx = self.get_value_at_time(time_ms, "center_x", self.center_x);
        let cy = self.get_value_at_time(time_ms, "center_y", self.center_y);
        (cx, cy)
    }

    fn calculate_mask_factor(
        nx: f32,
        ny: f32,
        center_x: f32,
        center_y: f32,
        rotation_rad: f32,
        feather: f32,
        opacity: f32,
        flip: bool,
        aspect_ratio: f32,
    ) -> f32 {
        let dx = (nx - center_x) * aspect_ratio;
        let dy = ny - center_y;

        let rx = dx * rotation_rad.cos() + dy * rotation_rad.sin();

        // rx > 0 means right side, rx < 0 means left side.
        // By default (flip=false), left side (rx < 0) is masked.
        // flip reverses which side is masked.
        let signed = if flip { rx } else { -rx };

        // Feather creates a smooth transition across the boundary line.
        // The transition spans the feather width centered on rx=0:
        //   fully masked (factor = 1 - opacity) when signed >= feather/2
        //   fully visible (factor = 1) when signed <= -feather/2
        //   smoothstep between when |signed| < feather/2
        if feather <= 0.0 {
            if signed >= 0.0 {
                1.0 - opacity
            } else {
                1.0
            }
        } else {
            let feather_half = feather / 2.0;
            if signed >= feather_half {
                1.0 - opacity
            } else if signed <= -feather_half {
                1.0
            } else {
                // smoothstep from 1.0 (visible) to 1.0 - opacity (masked)
                let t = (signed + feather_half) / feather; // 0.0 at visible edge, 1.0 at masked edge
                let smooth_t = t * t * (3.0 - 2.0 * t);
                1.0 - opacity * smooth_t
            }
        }
    }
}

impl VideoFilter for LinearMaskFilter {
    crate::impl_default_video_filter!(LinearMaskFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let (center_x, center_y) = self.get_center_at_time(time_ms);
        let center_x = center_x.clamp(0.0, 1.0);
        let center_y = center_y.clamp(0.0, 1.0);

        let rotation = self
            .get_value_at_time(time_ms, "rotation", self.rotation)
            .clamp(0.0, 360.0);
        let feather = self
            .get_value_at_time(time_ms, "feather", self.feather)
            .clamp(0.0, 1.0);
        let opacity = self
            .get_value_at_time(time_ms, "opacity", self.opacity)
            .clamp(0.0, 1.0);

        let rotation_rad = rotation.to_radians();

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let width = buffer.width() as f32;
                let height = buffer.height() as f32;
                let aspect_ratio = width / height;

                for (_x, _y, pixel) in buffer.enumerate_pixels_mut() {
                    let nx = _x as f32 / width;
                    let ny = _y as f32 / height;

                    let mask_factor = Self::calculate_mask_factor(
                        nx,
                        ny,
                        center_x,
                        center_y,
                        rotation_rad,
                        feather,
                        opacity,
                        self.flip,
                        aspect_ratio,
                    );

                    pixel.0[3] = ((pixel.0[3] as f32) * mask_factor).clamp(0.0, 255.0) as u8;
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
            ("center_x", self.center_x),
            ("center_y", self.center_y),
            ("rotation", self.rotation),
            ("feather", self.feather),
            ("opacity", self.opacity),
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