use glam::Vec3;

#[derive(Debug, Clone)]
pub struct PendulumConfig {
    pub anchor: Vec3,
    pub rope_length: f32,
    pub gravity: Vec3,
    pub damping_free: f32,
    pub damping_pulled: f32,
    pub initial_offset: Vec3,
    pub initial_velocity: Vec3,
    pub hang_direction: Vec3,
}

impl Default for PendulumConfig {
    fn default() -> Self {
        let anchor = Vec3::new(0.0, 4.72, 1.18);
        Self {
            anchor,
            rope_length: 1.22,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            damping_free: 0.9948,
            damping_pulled: 0.985,
            initial_offset: Vec3::new(0.16, 0.0, 0.08),
            initial_velocity: Vec3::new(-0.018, 0.0, 0.012),
            hang_direction: Vec3::new(0.0, -1.0, 0.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendulumState {
    position: Vec3,
    previous: Vec3,
    anchor: Vec3,
    rope_length: f32,
    gravity: Vec3,
    damping_free: f32,
    #[allow(dead_code)]
    damping_pulled: f32,
    hang_direction: Vec3,
}

impl PendulumState {
    pub fn new(config: &PendulumConfig) -> Self {
        let equilibrium = config.anchor + config.hang_direction * config.rope_length;
        let position = equilibrium + config.initial_offset;
        let previous = position - config.initial_velocity;
        Self {
            position,
            previous,
            anchor: config.anchor,
            rope_length: config.rope_length,
            gravity: config.gravity,
            damping_free: config.damping_free,
            damping_pulled: config.damping_pulled,
            hang_direction: config.hang_direction,
        }
    }

    pub fn step(&mut self, dt: f32) {
        let velocity = (self.position - self.previous) * self.damping_free;
        self.previous = self.position;
        self.position += velocity + self.gravity * (dt * dt);

        let mut offset = self.position - self.anchor;
        if offset.length_squared() < 1e-8 {
            offset = self.hang_direction;
        }
        offset = offset.normalize() * self.rope_length;
        self.position = self.anchor + offset;
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn anchor(&self) -> Vec3 {
        self.anchor
    }

    pub fn rope_direction(&self) -> Vec3 {
        let dir = self.position - self.anchor;
        if dir.length_squared() < 1e-8 {
            Vec3::new(0.0, -1.0, 0.0)
        } else {
            dir.normalize()
        }
    }
}
