use crate::light::LightDirection;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct SceneGeometry {
    pub plane_center: Vec3,
    pub plane_normal: Vec3,
    pub plane_up: Vec3,
    pub plane_right: Vec3,
    pub plane_width: f32,
    pub plane_height: f32,
}

impl SceneGeometry {
    pub fn default_vertical() -> Self {
        Self {
            plane_center: Vec3::new(0.0, 2.0, 0.0),
            plane_normal: Vec3::new(0.0, 0.0, 1.0),
            plane_up: Vec3::new(0.0, 1.0, 0.0),
            plane_right: Vec3::new(1.0, 0.0, 0.0),
            plane_width: 12.0,
            plane_height: 8.0,
        }
    }

    pub fn default_horizontal() -> Self {
        Self {
            plane_center: Vec3::new(0.0, 0.0, 0.0),
            plane_normal: Vec3::new(0.0, 1.0, 0.0),
            plane_up: Vec3::new(0.0, 0.0, -1.0),
            plane_right: Vec3::new(1.0, 0.0, 0.0),
            plane_width: 12.0,
            plane_height: 8.0,
        }
    }

    pub fn pixel_to_world(&self, px: u32, py: u32, width: u32, height: u32) -> Vec3 {
        let u = (px as f32 + 0.5) / width as f32 - 0.5;
        let v = (py as f32 + 0.5) / height as f32 - 0.5;
        self.plane_center + self.plane_right * (u * self.plane_width)
            - self.plane_up * (v * self.plane_height)
    }

    pub fn normalized_to_world(&self, u: f32, v: f32) -> Vec3 {
        let cu = u - 0.5;
        let cv = v - 0.5;
        self.plane_center + self.plane_right * (cu * self.plane_width)
            - self.plane_up * (cv * self.plane_height)
    }

    pub fn compute_anchor(
        &self,
        pos: (f32, f32),
        direction: LightDirection,
        rope_length: f32,
    ) -> Vec3 {
        let lamp_surface = self.normalized_to_world(pos.0, pos.1);
        let ref_dim = if direction.is_horizontal() {
            self.plane_width
        } else {
            self.plane_height
        };
        let actual_rope = rope_length * ref_dim;
        let lamp_pos = lamp_surface + self.plane_normal * (actual_rope * 0.5);
        lamp_pos - direction.to_vec3() * actual_rope
    }

    pub fn actual_rope_length(&self, direction: LightDirection, rope_length: f32) -> f32 {
        let ref_dim = if direction.is_horizontal() {
            self.plane_width
        } else {
            self.plane_height
        };
        rope_length * ref_dim
    }
}
