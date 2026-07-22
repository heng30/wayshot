// fork: https://github.com/Aegisir/QWidgetSekai/tree/main/pyqt_project/FlowFieldBackground/src

use crate::{AnimationInit, scale_pixel_for_height};
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct FlowParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
}

impl FlowParticle {
    fn new(width: f32, height: f32) -> Self {
        let mut rng = rand::rng();
        Self {
            x: rng.random::<f32>() * width,
            y: rng.random::<f32>() * height,
            vx: 0.0,
            vy: 0.0,
            age: 0.0,
            life: rng.random::<f32>() * 200.0 + 100.0,
        }
    }

    fn reset(&mut self, width: f32, height: f32) {
        let mut rng = rand::rng();
        self.x = rng.random::<f32>() * width;
        self.y = rng.random::<f32>() * height;
        self.vx = 0.0;
        self.vy = 0.0;
        self.age = 0.0;
        self.life = rng.random::<f32>() * 200.0 + 100.0;
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FlowFieldConfig {
    /// Particle color (RGBA) (default: indigo #6366f1)
    #[derivative(Default(value = "(99, 102, 241, 255)"))]
    pub color: (u8, u8, u8, u8),

    /// Background color (RGB) (default: black)
    #[derivative(Default(value = "(0, 0, 0)"))]
    pub bg_color: (u8, u8, u8),

    /// Trail opacity [0.0, 1.0] — lower = longer trails (default: 0.15)
    #[derivative(Default(value = "0.15"))]
    pub trail_opacity: f32,

    /// Number of particles (default: 1000)
    #[derivative(Default(value = "1000"))]
    pub particle_count: u32,

    /// Speed multiplier (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub speed: f32,

    /// Enable pointer interaction / virtual pointer simulation (default: true)
    /// When disabled, particles flow without pointer repulsion
    #[derivative(Default(value = "true"))]
    pub pointer_enabled: bool,

    /// Number of virtual pointers (default: 2)
    #[derivative(Default(value = "2"))]
    pub pointer_count: usize,

    #[setters(skip)]
    #[serde(skip)]
    width: u32,

    #[setters(skip)]
    #[serde(skip)]
    height: u32,

    #[setters(skip)]
    #[serde(skip)]
    fps: u32,

    #[setters(skip)]
    #[serde(skip)]
    total_frames: usize,

    #[setters(skip)]
    #[serde(skip)]
    current_frame: usize,

    #[setters(skip)]
    #[serde(skip)]
    particles: Vec<FlowParticle>,

    #[setters(skip)]
    #[serde(skip)]
    pointers: Vec<VirtualPointer>,

    #[setters(skip)]
    #[serde(skip)]
    canvas: Option<RgbaImage>,
}

/// Virtual pointer that simulates mouse movement
#[derive(Debug, Clone)]
struct VirtualPointer {
    position: [f32; 2],
    target: [f32; 2],
    target_timer: u32,
}

impl VirtualPointer {
    fn new(width: f32, height: f32) -> Self {
        let mut rng = rand::rng();
        Self {
            position: [rng.random::<f32>() * width, rng.random::<f32>() * height],
            target: [rng.random::<f32>() * width, rng.random::<f32>() * height],
            target_timer: 0,
        }
    }

    fn update(&mut self, width: f32, height: f32) {
        self.target_timer += 1;
        let mut rng = rand::rng();
        if self.target_timer > 60 + rng.random_range(0..121) {
            self.target_timer = 0;
            self.target = [rng.random::<f32>() * width, rng.random::<f32>() * height];
        }

        let dx = self.target[0] - self.position[0];
        let dy = self.target[1] - self.position[1];
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > 5.0 {
            let speed = 1.5 + rng.random::<f32>() * 4.0;
            self.position[0] += dx / dist * speed;
            self.position[1] += dy / dist * speed;
        }

        self.position[0] = self.position[0].clamp(0.0, width);
        self.position[1] = self.position[1].clamp(0.0, height);
    }
}

impl FlowFieldConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn init_particles(&mut self) {
        let w = self.width.max(1) as f32;
        let h = self.height.max(1) as f32;
        let count = self.particle_count as usize;
        self.particles = (0..count).map(|_| FlowParticle::new(w, h)).collect();
    }

    fn init_pointers(&mut self) {
        let w = self.width as f32;
        let h = self.height as f32;
        self.pointers = (0..self.pointer_count)
            .map(|_| VirtualPointer::new(w, h))
            .collect();
    }

    fn init_canvas(&mut self) {
        let mut canvas = RgbaImage::new(self.width, self.height);
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }
        self.canvas = Some(canvas);
    }

    fn draw_frame(&mut self, frame_scale: f32) {
        let canvas = match &mut self.canvas {
            Some(c) => c,
            None => return,
        };

        let w = self.width.max(1) as f32;
        let h = self.height.max(1) as f32;

        // Trail: overlay background with low alpha
        let trail_alpha = (self.trail_opacity * 255.0).round() as u8;
        let trail_color = Rgba([
            self.bg_color.0,
            self.bg_color.1,
            self.bg_color.2,
            trail_alpha,
        ]);

        for pixel in canvas.pixels_mut() {
            let bg_a = trail_color.0[3] as f32 / 255.0;
            let dst_a = 1.0 - bg_a;
            pixel.0[0] =
                (pixel.0[0] as f32 * dst_a + trail_color.0[0] as f32 * bg_a).min(255.0) as u8;
            pixel.0[1] =
                (pixel.0[1] as f32 * dst_a + trail_color.0[1] as f32 * bg_a).min(255.0) as u8;
            pixel.0[2] =
                (pixel.0[2] as f32 * dst_a + trail_color.0[2] as f32 * bg_a).min(255.0) as u8;
            pixel.0[3] = 255;
        }

        // Update virtual pointers
        if self.pointer_enabled {
            for pointer in &mut self.pointers {
                pointer.update(w, h);
            }
        }

        let force_scale = frame_scale * self.speed;
        let particle_size = scale_pixel_for_height(2.0, self.height).max(1.0) as i32;
        let pointer_range = scale_pixel_for_height(150.0, self.height);
        let pointer_range2 = pointer_range * pointer_range;

        for particle in &mut self.particles {
            // Flow field angle: (cos(x * 0.005) + sin(y * 0.005)) * PI
            let angle = (particle.x.mul_add(0.005, 0.0).cos()
                + particle.y.mul_add(0.005, 0.0).sin())
                * std::f32::consts::PI;

            particle.vx += angle.cos() * 0.2 * force_scale;
            particle.vy += angle.sin() * 0.2 * force_scale;

            // Pointer repulsion
            if self.pointer_enabled {
                for pointer in &self.pointers {
                    let dx = pointer.position[0] - particle.x;
                    let dy = pointer.position[1] - particle.y;
                    let dist2 = dx * dx + dy * dy;
                    if dist2 < pointer_range2 {
                        let dist = dist2.sqrt();
                        let force = (pointer_range - dist) / pointer_range;
                        particle.vx -= dx * force * 0.05 * frame_scale;
                        particle.vy -= dy * force * 0.05 * frame_scale;
                    }
                }
            }

            particle.x += particle.vx * frame_scale;
            particle.y += particle.vy * frame_scale;

            let friction = 0.95_f32.powf(frame_scale);
            particle.vx *= friction;
            particle.vy *= friction;
            particle.age += frame_scale;

            // Reset or wrap
            if particle.age > particle.life {
                particle.reset(w, h);
            } else if particle.x < 0.0 {
                particle.x = w;
            } else if particle.x > w {
                particle.x = 0.0;
            } else if particle.y < 0.0 {
                particle.y = h;
            } else if particle.y > h {
                particle.y = 0.0;
            }

            // Alpha: (1 - |age/life - 0.5| * 2) * 255
            let alpha = ((1.0 - (particle.age / particle.life - 0.5).abs() * 2.0) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;

            // Draw particle as small square
            let px = particle.x as i32;
            let py = particle.y as i32;
            let color = self.color;
            let base_a = color.3 as f32 / 255.0;
            let a_f = base_a * (alpha as f32 / 255.0);

            for dy in 0..particle_size {
                for dx in 0..particle_size {
                    let ix = px + dx;
                    let iy = py + dy;
                    if ix >= 0 && ix < self.width as i32 && iy >= 0 && iy < self.height as i32 {
                        let existing = canvas.get_pixel(ix as u32, iy as u32);
                        let bg_a = 1.0 - a_f;
                        let new_r =
                            (existing.0[0] as f32 * bg_a + color.0 as f32 * a_f).min(255.0) as u8;
                        let new_g =
                            (existing.0[1] as f32 * bg_a + color.1 as f32 * a_f).min(255.0) as u8;
                        let new_b =
                            (existing.0[2] as f32 * bg_a + color.2 as f32 * a_f).min(255.0) as u8;
                        canvas.put_pixel(ix as u32, iy as u32, Rgba([new_r, new_g, new_b, 255]));
                    }
                }
            }
        }
    }
}

impl Iterator for FlowFieldConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        // frame_scale ≈ 1.0 at 60fps; clamp to [0.25, 3.0]
        let frame_scale = if self.fps > 0 {
            (60.0 / self.fps as f32).clamp(0.25, 3.0)
        } else {
            1.0
        };

        self.draw_frame(frame_scale);

        let frame = self.canvas.clone();
        self.current_frame += 1;
        Some(frame.unwrap_or_else(|| RgbaImage::new(self.width, self.height)))
    }
}

impl AnimationInit for FlowFieldConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.init_particles();
        if self.pointer_enabled {
            self.init_pointers();
        }
        self.init_canvas();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.particles.clear();
        self.pointers.clear();
        self.canvas = None;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(FlowFieldConfig);
