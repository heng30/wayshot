use crate::{physics::PendulumState, scene::SceneGeometry};
use glam::{Quat, Vec3};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightDirection {
    Up,
    Down,
    Left,
    Right,
}

impl FromStr for LightDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "up" => Ok(LightDirection::Up),
            "down" => Ok(LightDirection::Down),
            "left" => Ok(LightDirection::Left),
            "right" => Ok(LightDirection::Right),
            _ => Err(format!(
                "invalid direction: {s}, expected one of: up, down, left, right"
            )),
        }
    }
}

impl std::fmt::Display for LightDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LightDirection::Up => "up",
                LightDirection::Down => "down",
                LightDirection::Left => "left",
                LightDirection::Right => "right",
            }
        )
    }
}

impl LightDirection {
    pub fn to_vec3(self) -> Vec3 {
        match self {
            LightDirection::Up => Vec3::new(0.0, 1.0, 0.0),
            LightDirection::Down => Vec3::new(0.0, -1.0, 0.0),
            LightDirection::Left => Vec3::new(-1.0, 0.0, 0.0),
            LightDirection::Right => Vec3::new(1.0, 0.0, 0.0),
        }
    }
}

impl Default for LightDirection {
    fn default() -> Self {
        LightDirection::Down
    }
}

impl LightDirection {
    pub fn is_horizontal(self) -> bool {
        matches!(self, LightDirection::Left | LightDirection::Right)
    }
}

#[derive(Debug, Clone)]
pub struct SpotLightConfig {
    pub color: [f32; 3],
    pub brightness: f32,
    pub angle_deg: f32,
    pub penumbra: f32,
    pub decay: f32,
    pub max_distance: f32,
    pub direction: LightDirection,
    pub pos: (f32, f32),
    pub rope_length: f32,
    pub gravity: f32,
    pub swing: f32,
    pub damping: f32,
}

impl Default for SpotLightConfig {
    fn default() -> Self {
        Self {
            color: [1.0, 0.702, 0.42],
            brightness: 1450.0,
            angle_deg: 34.0,
            penumbra: 0.88,
            decay: 2.0,
            max_distance: 18.0,
            direction: LightDirection::Down,
            pos: (0.5, 0.0),
            rope_length: 0.3,
            gravity: 9.81,
            swing: 0.3,
            damping: 0.9948,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpotLightFrame {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: [f32; 3],
    pub brightness: f32,
    pub angle_rad: f32,
    pub penumbra: f32,
    pub decay: f32,
    pub max_distance: f32,
}

#[derive(Debug, Clone)]
pub struct SpotLightState {
    config: SpotLightConfig,
    current_direction: Vec3,
    lerp_rate_free: f32,
    plane_normal: Vec3,
}

impl SpotLightState {
    pub fn new(config: SpotLightConfig, scene: &SceneGeometry) -> Self {
        let base_dir = config.direction.to_vec3();
        let initial_dir = (base_dir - scene.plane_normal).normalize();
        Self {
            config,
            current_direction: initial_dir,
            lerp_rate_free: 0.14,
            plane_normal: scene.plane_normal,
        }
    }

    pub fn update(&mut self, pendulum: &PendulumState) -> SpotLightFrame {
        let base_dir = self.config.direction.to_vec3();
        let rope_dir = pendulum.rope_direction();

        let dot = base_dir.dot(rope_dir);
        let swing_quat = if dot > 0.9999 {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_arc(base_dir, rope_dir)
        };

        let target_dir = swing_quat * base_dir;
        let aimed_dir = (target_dir - self.plane_normal).normalize();
        self.current_direction = self
            .current_direction
            .lerp(aimed_dir, self.lerp_rate_free)
            .normalize();

        SpotLightFrame {
            position: pendulum.position() + base_dir * 0.35,
            direction: self.current_direction,
            color: self.config.color,
            brightness: self.config.brightness,
            angle_rad: self.config.angle_deg.to_radians(),
            penumbra: self.config.penumbra,
            decay: self.config.decay,
            max_distance: self.config.max_distance,
        }
    }
}
