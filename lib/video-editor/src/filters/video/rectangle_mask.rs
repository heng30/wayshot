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
pub struct RectangleMaskFilter {
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,
    pub rotation: f32,
    pub feather: f32,
    #[derivative(Default(value = "1.0"))]
    pub opacity: f32,
    pub flip: bool,
    #[derivative(Default(value = "0.5"))]
    pub width: f32,
    #[derivative(Default(value = "0.5"))]
    pub height: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty")]
    pub keyframe_tracks: KeyframeTracks,
}

impl RectangleMaskFilter {
    pub const NAME: &'static str = "rectangle mask";

    pub fn new(
        center_x: f32,
        center_y: f32,
        rotation: f32,
        feather: f32,
        opacity: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            center_x,
            center_y,
            rotation,
            feather,
            opacity,
            width,
            height,
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
            AnimatableProperty::float("width", "Width", 0.0, 1.0, 0.5),
            AnimatableProperty::float("height", "Height", 0.0, 1.0, 0.5),
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
        px_x: f32,
        px_y: f32,
        width_px: f32,
        height_px: f32,
        center_x: f32,
        center_y: f32,
        rotation_rad: f32,
        feather: f32,
        opacity: f32,
        flip: bool,
        half_w: f32,
        half_h: f32,
    ) -> f32 {
        let cx_px = center_x * width_px;
        let cy_px = center_y * height_px;
        let hw_px = half_w * width_px;
        let hh_px = half_h * height_px;

        let dx = px_x - cx_px;
        let dy = px_y - cy_px;

        let rx = dx * rotation_rad.cos() + dy * rotation_rad.sin();
        let ry = -dx * rotation_rad.sin() + dy * rotation_rad.cos();

        // Signed distance to rectangle boundary in pixel space (negative = inside)
        let sx = rx.abs() - hw_px;
        let sy = ry.abs() - hh_px;
        // Normalize feather to pixel space for consistent blur width
        let outside_dist = sx.max(sy) / width_px.max(height_px);

        // By default (flip=false): inside rectangle = visible (1.0), outside = masked
        // flip reverses: inside = masked, outside = visible
        let signed = if flip { -outside_dist } else { outside_dist };

        if feather <= 0.0 {
            if signed <= 0.0 {
                1.0
            } else {
                1.0 - opacity
            }
        } else {
            if signed <= -feather / 2.0 {
                1.0
            } else if signed >= feather / 2.0 {
                1.0 - opacity
            } else {
                let t = (signed + feather / 2.0) / feather;
                let smooth_t = t * t * (3.0 - 2.0 * t);
                1.0 - opacity * smooth_t
            }
        }
    }
}

impl VideoFilter for RectangleMaskFilter {
    crate::impl_default_video_filter!(RectangleMaskFilter);

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
        let width = self
            .get_value_at_time(time_ms, "width", self.width)
            .clamp(0.0, 1.0);
        let height = self
            .get_value_at_time(time_ms, "height", self.height)
            .clamp(0.0, 1.0);

        let rotation_rad = rotation.to_radians();
        let half_w = width / 2.0;
        let half_h = height / 2.0;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let width_px = buffer.width() as f32;
                let height_px = buffer.height() as f32;

                for (_x, _y, pixel) in buffer.enumerate_pixels_mut() {
                    let mask_factor = Self::calculate_mask_factor(
                        _x as f32,
                        _y as f32,
                        width_px,
                        height_px,
                        center_x,
                        center_y,
                        rotation_rad,
                        feather,
                        opacity,
                        self.flip,
                        half_w,
                        half_h,
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
            ("width", self.width),
            ("height", self.height),
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
