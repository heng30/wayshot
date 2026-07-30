use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use glam::Vec3;
use lighting::{
    LightDirection, SceneGeometry, SpotLightConfig, SpotLightState, apply_spotlight_rgba,
    physics::{PendulumConfig, PendulumState},
};

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
pub enum LightingDirection {
    #[default]
    Down = 0,
    Up = 1,
    Left = 2,
    Right = 3,
}

impl From<LightingDirection> for LightDirection {
    fn from(value: LightingDirection) -> Self {
        match value {
            LightingDirection::Down => LightDirection::Down,
            LightingDirection::Up => LightDirection::Up,
            LightingDirection::Left => LightDirection::Left,
            LightingDirection::Right => LightDirection::Right,
        }
    }
}

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
pub enum LightingScene {
    #[default]
    Vertical = 0,
    Horizontal = 1,
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
#[serde(default)]
#[non_exhaustive]
pub struct LightingFilter {
    /// Light color as [R, G, B] in 0.0-1.0 range.
    #[derivative(Default(value = "[1.0, 0.702, 0.42]"))]
    pub color: [f32; 3],

    /// Light brightness (default 1450.0).
    #[derivative(Default(value = "1450.0"))]
    pub brightness: f32,

    /// Cone angle in degrees.
    #[derivative(Default(value = "34.0"))]
    pub angle_deg: f32,

    /// Penumbra ratio (0.0-1.0, higher = softer edge).
    #[derivative(Default(value = "0.88"))]
    pub penumbra: f32,

    /// Distance decay exponent.
    #[derivative(Default(value = "2.0"))]
    pub decay: f32,

    /// Maximum light distance in world units.
    #[derivative(Default(value = "18.0"))]
    pub max_distance: f32,

    /// Light direction.
    #[derivative(Default(value = "LightingDirection::Down"))]
    pub direction: LightingDirection,

    /// Light position as normalized (x, y) in -1.0 to 2.0 range.
    /// [0, 1] = within the image, outside that = beyond image edges.
    /// (0.0, 0.0) = top-left, (0.5, 0.0) = top-center, (1.0, 1.0) = bottom-right.
    #[derivative(Default(value = "(0.5, 0.0)"))]
    pub pos: (f32, f32),

    /// Rope length for pendulum swing (0.0-1.0, fraction of scene dimension).
    #[derivative(Default(value = "0.3"))]
    pub rope_length: f32,

    /// Gravity strength for pendulum.
    #[derivative(Default(value = "9.81"))]
    pub gravity: f32,

    /// Initial swing amplitude (0.0-1.0).
    #[derivative(Default(value = "0.3"))]
    pub swing: f32,

    /// Pendulum damping factor (0.0-1.0, higher = less damping).
    #[derivative(Default(value = "0.9948"))]
    pub damping: f32,

    /// Ambient light level (0.0-1.0).
    #[derivative(Default(value = "0.06"))]
    pub ambient: f32,

    /// Scene type: Vertical (wall) or Horizontal (floor).
    #[derivative(Default(value = "LightingScene::Vertical"))]
    pub scene: LightingScene,

    /// Physics fixed step time in seconds.
    #[derivative(Default(value = "1.0 / 120.0"))]
    pub fixed_step: f32,

    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl LightingFilter {
    pub const NAME: &'static str = "lighting";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("brightness", "Brightness", 0.0, 5000.0, 1450.0),
            AnimatableProperty::float("angle_deg", "Cone Angle", 1.0, 180.0, 34.0),
            AnimatableProperty::float("penumbra", "Penumbra", 0.0, 1.0, 0.88),
            AnimatableProperty::float("decay", "Decay", 0.0, 10.0, 2.0),
            AnimatableProperty::float("max_distance", "Max Distance", 1.0, 100.0, 18.0),
            AnimatableProperty::float2("pos", "Position", -1.0, 2.0, 0.5, 0.0),
            AnimatableProperty::float("ambient", "Ambient", 0.0, 1.0, 0.06),
            AnimatableProperty::float("swing", "Swing", 0.0, 1.0, 0.3),
            AnimatableProperty::float("rope_length", "Rope Length", 0.0, 1.0, 0.3),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    fn get_pos_at_time(&self, time_ms: i64) -> (f32, f32) {
        self.keyframe_tracks
            .get_track("pos")
            .map(|track| {
                crate::filters::interpolation::get_float2_at_time(
                    track, time_ms, self.pos.0, self.pos.1,
                )
            })
            .unwrap_or(self.pos)
    }

    fn build_scene(&self) -> SceneGeometry {
        match self.scene {
            LightingScene::Vertical => SceneGeometry::default_vertical(),
            LightingScene::Horizontal => SceneGeometry::default_horizontal(),
        }
    }

    fn simulate_to_time(
        &self,
        target_time_secs: f32,
        brightness: f32,
        angle_deg: f32,
        penumbra: f32,
        decay: f32,
        max_distance: f32,
        pos: (f32, f32),
        swing: f32,
        rope_length: f32,
    ) -> lighting::SpotLightFrame {
        let direction: LightDirection = self.direction.into();
        let scene = self.build_scene();

        let actual_rope = scene.actual_rope_length(direction, rope_length);
        let anchor = scene.compute_anchor(pos, direction, rope_length);
        let gravity_vec = direction.to_vec3() * self.gravity;
        let hang_dir = direction.to_vec3();
        let swing_dir = scene.plane_normal.cross(hang_dir).normalize();
        let initial_offset = swing_dir * (swing * actual_rope);

        let pendulum_config = PendulumConfig {
            anchor,
            rope_length: actual_rope,
            hang_direction: hang_dir,
            gravity: gravity_vec,
            damping_free: self.damping,
            initial_offset,
            initial_velocity: Vec3::ZERO,
            ..Default::default()
        };

        let light_config = SpotLightConfig {
            color: self.color,
            brightness,
            angle_deg,
            penumbra,
            decay,
            max_distance,
            direction,
            pos,
            rope_length,
            gravity: self.gravity,
            swing,
            damping: self.damping,
        };

        let mut pendulum = PendulumState::new(&pendulum_config);
        let mut light_state = SpotLightState::new(light_config, &scene);

        let fixed_step = self.fixed_step;
        let mut accumulated = 0.0f32;
        while accumulated < target_time_secs {
            pendulum.step(fixed_step);
            accumulated += fixed_step;
        }

        light_state.update(&pendulum)
    }
}

impl VideoFilter for LightingFilter {
    crate::impl_default_video_filter!(LightingFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;
        let time_secs = data.relative_timeline_offset.as_secs_f32();

        let brightness = self.get_value_at_time(time_ms, "brightness", self.brightness);
        let angle_deg = self.get_value_at_time(time_ms, "angle_deg", self.angle_deg);
        let penumbra = self
            .get_value_at_time(time_ms, "penumbra", self.penumbra)
            .clamp(0.0, 1.0);
        let decay = self
            .get_value_at_time(time_ms, "decay", self.decay)
            .max(0.0);
        let max_distance = self
            .get_value_at_time(time_ms, "max_distance", self.max_distance)
            .max(1.0);
        let pos = self.get_pos_at_time(time_ms);
        let pos = (pos.0.clamp(-1.0, 2.0), pos.1.clamp(-1.0, 2.0));
        let ambient = self
            .get_value_at_time(time_ms, "ambient", self.ambient)
            .clamp(0.0, 1.0);
        let swing = self
            .get_value_at_time(time_ms, "swing", self.swing)
            .clamp(0.0, 1.0);
        let rope_length = self
            .get_value_at_time(time_ms, "rope_length", self.rope_length)
            .clamp(0.0, 1.0);

        let light_frame = self.simulate_to_time(
            time_secs,
            brightness,
            angle_deg,
            penumbra,
            decay,
            max_distance,
            pos,
            swing,
            rope_length,
        );

        let scene = self.build_scene();

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                *buffer = apply_spotlight_rgba(buffer, &light_frame, &scene, ambient);
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
            ("brightness", self.brightness),
            ("angle_deg", self.angle_deg),
            ("penumbra", self.penumbra),
            ("decay", self.decay),
            ("max_distance", self.max_distance),
            ("ambient", self.ambient),
            ("swing", self.swing),
            ("rope_length", self.rope_length),
        ] {
            if let Some(track) = tracks.get_track(property)
                && track.keyframes.iter().any(|k| k.time_ms == time_ms)
            {
                tracks.update_keyframe_value(property, time_ms, KeyframeValue::Float(value));
                updated = true;
            }
        }

        if let Some(track) = tracks.get_track("pos")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "pos",
                time_ms,
                KeyframeValue::Float2(self.pos.0, self.pos.1),
            );
            updated = true;
        }

        updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_name() {
        assert_eq!(LightingFilter::NAME, "lighting");
    }

    #[test]
    fn test_default_values() {
        let filter = LightingFilter::default();
        assert_eq!(filter.direction, LightingDirection::Down);
        assert_eq!(filter.scene, LightingScene::Vertical);
        assert!((filter.brightness - 1450.0).abs() < f32::EPSILON);
        assert!((filter.angle_deg - 34.0).abs() < f32::EPSILON);
        assert!((filter.penumbra - 0.88).abs() < f32::EPSILON);
        assert!((filter.ambient - 0.06).abs() < f32::EPSILON);
        assert!((filter.pos.0 - 0.5).abs() < f32::EPSILON);
        assert!((filter.pos.1 - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_setters() {
        let filter = LightingFilter::default()
            .with_brightness(2000.0)
            .with_angle_deg(45.0)
            .with_ambient(0.1)
            .with_direction(LightingDirection::Up)
            .with_scene(LightingScene::Horizontal);

        assert!((filter.brightness - 2000.0).abs() < f32::EPSILON);
        assert!((filter.angle_deg - 45.0).abs() < f32::EPSILON);
        assert!((filter.ambient - 0.1).abs() < f32::EPSILON);
        assert_eq!(filter.direction, LightingDirection::Up);
        assert_eq!(filter.scene, LightingScene::Horizontal);
    }

    #[test]
    fn test_supports_keyframes() {
        let filter = LightingFilter::default();
        assert!(filter.supports_keyframes());
        assert_eq!(filter.get_animatable_properties().len(), 9);
    }
}
