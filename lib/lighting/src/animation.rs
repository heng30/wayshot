use std::path::Path;

use glam::Vec3;
use image::RgbImage;

use crate::light::{SpotLightConfig, SpotLightState};
use crate::physics::{PendulumConfig, PendulumState};
use crate::render::apply_spotlight;
use crate::scene::SceneGeometry;

pub struct AnimationConfig {
    pub fps: u32,
    pub duration_secs: f32,
    pub fixed_step: f32,
    pub ambient: f32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            duration_secs: 10.0,
            fixed_step: 1.0 / 120.0,
            ambient: 0.06,
        }
    }
}

pub struct FrameProducer {
    pendulum: PendulumState,
    light_state: SpotLightState,
    scene: SceneGeometry,
    config: AnimationConfig,
    source_image: RgbImage,
    current_time: f32,
    accumulator: f32,
    frame_index: u32,
}

impl FrameProducer {
    pub fn new(
        pendulum_config: PendulumConfig,
        light_config: SpotLightConfig,
        scene: SceneGeometry,
        anim_config: AnimationConfig,
        source_image: RgbImage,
    ) -> Self {
        Self {
            pendulum: PendulumState::new(&pendulum_config),
            light_state: SpotLightState::new(light_config, &scene),
            scene,
            config: anim_config,
            source_image,
            current_time: 0.0,
            accumulator: 0.0,
            frame_index: 0,
        }
    }

    pub fn next_frame(&mut self) -> Option<(u32, RgbImage)> {
        let frame_dt = 1.0 / self.config.fps as f32;
        self.accumulator += frame_dt;

        let max_accum = self.config.fixed_step * 5.0;
        self.accumulator = self.accumulator.min(max_accum);
        while self.accumulator >= self.config.fixed_step {
            self.pendulum.step(self.config.fixed_step);
            self.accumulator -= self.config.fixed_step;
        }

        self.current_time += frame_dt;
        if self.current_time > self.config.duration_secs {
            return None;
        }

        let light_frame = self.light_state.update(&self.pendulum);
        let output = apply_spotlight(
            &self.source_image,
            &light_frame,
            &self.scene,
            self.config.ambient,
        );
        let idx = self.frame_index;
        self.frame_index += 1;
        Some((idx, output))
    }
}

pub fn render_animation(
    source_path: &Path,
    output_dir: &Path,
    light_config: SpotLightConfig,
    scene: SceneGeometry,
    anim_config: AnimationConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open(source_path)?.to_rgb8();
    std::fs::create_dir_all(output_dir)?;

    let source_name = source_path.file_name().unwrap_or_default();
    let source_copy = output_dir.join(source_name);
    if source_copy != source_path {
        std::fs::copy(source_path, &source_copy)?;
    }

    let actual_rope = scene.actual_rope_length(light_config.direction, light_config.rope_length);
    let anchor = scene.compute_anchor(light_config.pos, light_config.direction, light_config.rope_length);
    let gravity = light_config.direction.to_vec3() * light_config.gravity;
    let hang_dir = light_config.direction.to_vec3();
    let swing_dir = scene.plane_normal.cross(hang_dir).normalize();
    let initial_offset = swing_dir * (light_config.swing * actual_rope);
    let pendulum_config = PendulumConfig {
        anchor,
        rope_length: actual_rope,
        hang_direction: hang_dir,
        gravity,
        damping_free: light_config.damping,
        initial_offset,
        initial_velocity: Vec3::ZERO,
        ..Default::default()
    };

    let mut producer = FrameProducer::new(
        pendulum_config,
        light_config,
        scene,
        anim_config,
        img,
    );

    while let Some((idx, frame)) = producer.next_frame() {
        let path = output_dir.join(format!("frame_{:04}.png", idx));
        frame.save(&path)?;
    }
    Ok(())
}
