use crate::{
    Result,
    filters::{
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_for_height,
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
pub struct CircleMaskFilter {
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,
    pub feather: f32,
    #[derivative(Default(value = "1.0"))]
    pub opacity: f32,
    pub flip: bool,
    #[derivative(Default(value = "270"))]
    pub radius: u32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty")]
    pub keyframe_tracks: KeyframeTracks,
}

impl CircleMaskFilter {
    pub const NAME: &'static str = "circle mask";

    pub fn new(center_x: f32, center_y: f32, feather: f32, opacity: f32, radius: u32) -> Self {
        Self {
            center_x,
            center_y,
            feather,
            opacity,
            radius,
            ..Default::default()
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("center_x", "Center X", 0.0, 1.0, 0.5),
            AnimatableProperty::float("center_y", "Center Y", 0.0, 1.0, 0.5),
            AnimatableProperty::float("feather", "Feather", 0.0, 1.0, 0.0),
            AnimatableProperty::float("opacity", "Opacity", 0.0, 1.0, 1.0),
            AnimatableProperty::float("radius", "Radius", 0.0, 5000.0, 270.0),
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
        px: f32,
        py: f32,
        center_x_px: f32,
        center_y_px: f32,
        feather_px: f32,
        opacity: f32,
        flip: bool,
        radius_px: f32,
    ) -> f32 {
        let dx = px - center_x_px;
        let dy = py - center_y_px;
        let dist = (dx * dx + dy * dy).sqrt();

        let signed_dist = dist - radius_px;

        let signed = if flip { -signed_dist } else { signed_dist };

        if feather_px <= 0.0 {
            if signed <= 0.0 { 1.0 } else { 1.0 - opacity }
        } else {
            let feather_half = feather_px / 2.0;
            if signed <= -feather_half {
                1.0
            } else if signed >= feather_half {
                1.0 - opacity
            } else {
                let t = (signed + feather_half) / feather_px;
                let smooth_t = t * t * (3.0 - 2.0 * t);
                1.0 - opacity * smooth_t
            }
        }
    }
}

impl VideoFilter for CircleMaskFilter {
    crate::impl_default_video_filter!(CircleMaskFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let (center_x, center_y) = self.get_center_at_time(time_ms);
        let center_x = center_x.clamp(0.0, 1.0);
        let center_y = center_y.clamp(0.0, 1.0);

        let feather = self
            .get_value_at_time(time_ms, "feather", self.feather)
            .clamp(0.0, 1.0);
        let opacity = self
            .get_value_at_time(time_ms, "opacity", self.opacity)
            .clamp(0.0, 1.0);
        let radius = self
            .get_value_at_time(time_ms, "radius", self.radius as f32)
            .clamp(0.0, 5000.0) as u32;

        let output_height = data.config.output_height;
        let radius_px = scale_pixel_for_height(radius, output_height) as f32;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let width_px = buffer.width() as f32;
                let height_px = buffer.height() as f32;
                let center_x_px = center_x * width_px;
                let center_y_px = center_y * height_px;
                let feather_px = feather * height_px;

                for (_x, _y, pixel) in buffer.enumerate_pixels_mut() {
                    let px = _x as f32 + 0.5;
                    let py = _y as f32 + 0.5;

                    let mask_factor = Self::calculate_mask_factor(
                        px,
                        py,
                        center_x_px,
                        center_y_px,
                        feather_px,
                        opacity,
                        self.flip,
                        radius_px,
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
            ("feather", self.feather),
            ("opacity", self.opacity),
            ("radius", self.radius as f32),
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
