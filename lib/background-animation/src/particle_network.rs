// Particle network animation with connecting lines
// Reference: https://github.com/tom-mohu/particle-life-app
// Python reference: output/QWidgetSekai/pyqt_project/Particle/src/particle.py

use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single particle with 3D position and velocity
/// z represents depth (30-60) affecting particle size
#[derive(Debug, Clone)]
struct Particle {
    /// Position [x, y, z] in pixels
    offset: [f32; 3],
    /// Velocity components [vx, vy, vz]
    head: [f32; 3],
    /// Bounds [width, height, max_z] - each particle has random z max (30-60)
    container: [f32; 3],
}

impl Particle {
    /// Create a new particle with random position across entire container
    fn new(container: [f32; 3]) -> Self {
        let mut rng = rand::rng();

        // Random z max for each particle: 30-60
        let z_max = 30.0 + rng.random_range(0..31) as f32;

        // Random position across entire container (uniform distribution)
        let offset = [
            rng.random::<f32>() * container[0],
            rng.random::<f32>() * container[1],
            rng.random::<f32>() * z_max,
        ];

        // Velocity: all directions randomly positive or negative for better spread
        let head = [
            Self::random_head(&mut rng),
            Self::random_head(&mut rng),
            Self::random_head(&mut rng),
        ];

        Self {
            offset,
            head,
            container: [container[0], container[1], z_max],
        }
    }

    /// Random velocity value in [-0.5, 0.5] range, randomly positive or negative
    /// Matches Python's _randomHead()
    fn random_head(rng: &mut impl RngExt) -> f32 {
        let value = rng.random::<f32>() / 2.0;
        if rng.random_bool(0.5) {
            -value
        } else {
            value
        }
    }

    /// Update position with bouncing physics
    /// Matches Python's nextStep()
    fn next_step(&mut self) {
        for i in 0..3 {
            let value = self.offset[i] + self.head[i];
            let low = if i == 2 { 25.0 } else { 0.0 };
            let high = self.container[i];

            if value < low {
                self.offset[i] = low;
                self.head[i] = -self.head[i];
            } else if value > high {
                self.offset[i] = high;
                self.head[i] = -self.head[i];
            } else {
                self.offset[i] = value;
            }
        }
    }

    /// Reset position when container size changes
    fn reset_offset(&mut self, container: [f32; 3]) {
        let mut rng = rand::rng();
        let z_max = 30.0 + rng.random_range(0..31) as f32;
        self.container = [container[0], container[1], z_max];
        self.offset = [
            rng.random::<f32>() * container[0],
            rng.random::<f32>() * container[1],
            rng.random::<f32>() * z_max,
        ];
    }
}

/// Virtual pointer that simulates mouse movement with random behavior
#[derive(Debug, Clone)]
struct VirtualPointer {
    /// Current position [x, y]
    position: [f32; 2],
    /// Target position to move towards
    target: [f32; 2],
    /// Frame counter for target changes
    target_timer: u32,
}

impl VirtualPointer {
    /// Create a new virtual pointer at random position
    fn new(width: f32, height: f32) -> Self {
        let mut rng = rand::rng();
        Self {
            position: [rng.random::<f32>() * width, rng.random::<f32>() * height],
            target: [rng.random::<f32>() * width, rng.random::<f32>() * height],
            target_timer: 0,
        }
    }

    /// Update pointer position with random movement towards target
    fn update(&mut self, width: f32, height: f32) {
        self.target_timer += 1;

        // Change target randomly every 60-180 frames
        let mut rng = rand::rng();
        if self.target_timer > 60 + rng.random_range(0..121) {
            self.target_timer = 0;
            self.target = [rng.random::<f32>() * width, rng.random::<f32>() * height];
        }

        // Move towards target with simple interpolation
        let dx = self.target[0] - self.position[0];
        let dy = self.target[1] - self.position[1];
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > 5.0 {
            // Move at random speed (1.5-5.5 pixels per frame)
            let speed = 1.5 + rng.random::<f32>() * 4.0;
            self.position[0] += dx / dist * speed;
            self.position[1] += dy / dist * speed;
        }

        // Keep within bounds
        self.position[0] = self.position[0].clamp(0.0, width);
        self.position[1] = self.position[1].clamp(0.0, height);
    }
}

/// Particle network animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ParticleNetworkConfig {
    /// Particle density (1-9), affects particle count (default: 5)
    #[derivative(Default(value = "5"))]
    pub density: u32,

    /// Line connection color (RGBA) (default: semi-transparent white)
    #[derivative(Default(value = "(255, 255, 255, 102)"))]
    pub line_color: (u8, u8, u8, u8),

    /// Particle color (RGBA) (default: semi-transparent white)
    #[derivative(Default(value = "(255, 255, 255, 51)"))]
    pub particle_color: (u8, u8, u8, u8),

    /// Background color (RGB) (default: dark)
    #[derivative(Default(value = "(10, 15, 25)"))]
    pub bg_color: (u8, u8, u8),

    /// Enable simulated pointer movement (default: true for dynamic animation)
    #[derivative(Default(value = "true"))]
    pub pointer_enabled: bool,

    /// Pointer interaction range (default: 120)
    #[derivative(Default(value = "120.0"))]
    pub pointer_range: f32,

    /// Number of virtual pointers (default: 3)
    #[derivative(Default(value = "3"))]
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
    particles: Vec<Particle>,

    #[setters(skip)]
    #[serde(skip)]
    grid: HashMap<(i32, i32), Vec<usize>>,

    #[setters(skip)]
    #[serde(skip)]
    pointers: Vec<VirtualPointer>,
}

impl ParticleNetworkConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate particle count based on density
    /// Matches Python's _particleCount()
    fn particle_count(&self) -> usize {
        let density = self.density.min(9).max(1);
        (self.width.min(self.height) as f32 / (10.0 - density as f32)) as usize
    }

    /// Get container bounds [width, height]
    /// Matches Python's _container()
    fn container(&self) -> [f32; 3] {
        [self.width.max(1) as f32, self.height.max(1) as f32, 60.0]
    }

    /// Initialize or update particles based on current size
    /// Matches Python's _updateParticles()
    fn update_particles(&mut self) {
        let count = self.particle_count();
        let container = self.container();

        let diff = count as i32 - self.particles.len() as i32;

        if diff > 0 {
            // Add new particles with random initialization
            for _ in 0..diff {
                self.particles.push(Particle::new(container));
            }
        } else if diff < 0 {
            // Remove excess particles
            self.particles.truncate(count);
        }

        // Update existing particles' containers
        for p in &mut self.particles {
            p.container = [container[0], container[1], p.container[2]];
            if p.offset[0] > container[0] || p.offset[1] > container[1] {
                p.reset_offset(container);
            }
        }
    }

    /// Initialize virtual pointers
    fn init_pointers(&mut self) {
        let width = self.width as f32;
        let height = self.height as f32;

        self.pointers.clear();
        for _ in 0..self.pointer_count {
            self.pointers.push(VirtualPointer::new(width, height));
        }
    }

    /// Update pointer positions and attract nearby particles
    /// Matches Python's _drawPointer attraction logic
    fn update_pointers(&mut self) {
        let width = self.width as f32;
        let height = self.height as f32;
        let pointer_range_sq = self.pointer_range * self.pointer_range; // 14400 for 120px

        for pointer in &mut self.pointers {
            pointer.update(width, height);

            // Attract nearby particles when distance is between 80px (6400) and 120px (14400)
            // Matches Python: if dist2 > 6400: m = abs(max(dx, dy)) or 1; offset += (dx/m, dy/m)
            let attract_min_sq = 6400.0; // 80 * 80
            for p in &mut self.particles {
                let dx = pointer.position[0] - p.offset[0];
                let dy = pointer.position[1] - p.offset[1];
                let dist_sq = dx * dx + dy * dy;

                // Attract when dist2 > 6400 and dist2 < 14400
                if dist_sq > attract_min_sq && dist_sq < pointer_range_sq {
                    let m = dx.abs().max(dy.abs()).max(1.0);
                    p.offset[0] += dx / m;
                    p.offset[1] += dy / m;
                }
            }
        }
    }

    /// Draw line between two particles if close enough
    /// Matches Python's _drawLine
    fn draw_line(
        img: &mut RgbaImage,
        p1: &[f32; 3],
        p2: &[f32; 3],
        dist_sq: f32,
        color: (u8, u8, u8, u8),
    ) {
        const MAX_DIST_SQ: f32 = 14400.0; // 120 * 120

        if dist_sq >= MAX_DIST_SQ {
            return;
        }

        let dist = dist_sq.sqrt();
        // Python: painter.setPen(QPen(color, (120 - dist) / 80))
        let line_width = (120.0 - dist) / 80.0;

        // Calculate alpha based on distance
        let alpha_factor = (1.0 - dist / 120.0) * (color.3 as f32 / 255.0);

        let x0 = p1[0] as i32;
        let y0 = p1[1] as i32;
        let x1 = p2[0] as i32;
        let y1 = p2[1] as i32;

        // Draw line using Bresenham-like approach
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };

        let mut x = x0;
        let mut y = y0;

        let width_f = img.width() as i32;
        let height_f = img.height() as i32;

        if dx > dy {
            let mut err = dx / 2;
            while x != x1 {
                if x >= 0 && x < width_f && y >= 0 && y < height_f {
                    Self::blend_pixel(img, x as u32, y as u32, color, alpha_factor * line_width);
                }
                err -= dy;
                if err < 0 {
                    y += sy;
                    err += dx;
                }
                x += sx;
            }
        } else {
            let mut err = dy / 2;
            while y != y1 {
                if x >= 0 && x < width_f && y >= 0 && y < height_f {
                    Self::blend_pixel(img, x as u32, y as u32, color, alpha_factor * line_width);
                }
                err -= dx;
                if err < 0 {
                    x += sx;
                    err += dy;
                }
                y += sy;
            }
        }

        // Draw endpoint
        if x1 >= 0 && x1 < width_f && y1 >= 0 && y1 < height_f {
            Self::blend_pixel(img, x1 as u32, y1 as u32, color, alpha_factor * line_width);
        }
    }

    /// Blend a pixel with alpha
    fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: (u8, u8, u8, u8), alpha: f32) {
        let alpha = alpha.min(1.0);
        if alpha <= 0.0 {
            return;
        }

        let existing = img.get_pixel(x, y);
        let bg_alpha = 1.0 - alpha;

        let new_r = ((existing.0[0] as f32 * bg_alpha + color.0 as f32 * alpha).min(255.0)) as u8;
        let new_g = ((existing.0[1] as f32 * bg_alpha + color.1 as f32 * alpha).min(255.0)) as u8;
        let new_b = ((existing.0[2] as f32 * bg_alpha + color.2 as f32 * alpha).min(255.0)) as u8;

        img.put_pixel(x, y, Rgba([new_r, new_g, new_b, 255]));
    }

    /// Draw particle as a circle with size based on z-depth
    /// Matches Python: painter.drawEllipse(QPointF(offset[0], offset[1]), offset[2] / 30, offset[2] / 30)
    fn draw_particle(img: &mut RgbaImage, offset: &[f32; 3], color: (u8, u8, u8, u8)) {
        // Particle radius based on z-depth: z / 30
        let radius = (offset[2] / 30.0) as i32;
        if radius <= 0 {
            return;
        }

        let cx = offset[0] as i32;
        let cy = offset[1] as i32;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px = cx + dx;
                let py = cy + dy;

                if px < 0 || px >= img.width() as i32 || py < 0 || py >= img.height() as i32 {
                    continue;
                }

                let dist_sq = dx * dx + dy * dy;
                let radius_sq = radius * radius;

                if dist_sq <= radius_sq {
                    // Soft edge
                    let edge_dist = (dist_sq as f32).sqrt() / radius as f32;
                    let alpha = if edge_dist > 0.7 {
                        1.0 - (edge_dist - 0.7) / 0.3
                    } else {
                        1.0
                    };

                    Self::blend_pixel(
                        img,
                        px as u32,
                        py as u32,
                        color,
                        alpha * (color.3 as f32 / 255.0),
                    );
                }
            }
        }
    }

    /// Generate a frame from current particle positions
    fn generate_frame(&mut self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Build spatial grid for efficient neighbor lookup (120px cells)
        self.grid.clear();
        const CELL_SIZE: f32 = 120.0;

        for (i, p) in self.particles.iter().enumerate() {
            let cell_x = (p.offset[0] / CELL_SIZE) as i32;
            let cell_y = (p.offset[1] / CELL_SIZE) as i32;
            self.grid
                .entry((cell_x, cell_y))
                .or_insert_with(Vec::new)
                .push(i);
        }

        // Draw pointer connections first (so particle lines are on top)
        if self.pointer_enabled {
            let pointer_range_sq = self.pointer_range * self.pointer_range;
            for pointer in &self.pointers {
                // Draw lines from pointer to nearby particles
                for p in &self.particles {
                    let dx = p.offset[0] - pointer.position[0];
                    let dy = p.offset[1] - pointer.position[1];
                    let dist_sq = dx * dx + dy * dy;

                    if dist_sq < pointer_range_sq {
                        // Use pointer position as a pseudo-3D point (z = 40 for medium size)
                        let pointer_offset = [pointer.position[0], pointer.position[1], 40.0];
                        Self::draw_line(&mut img, &p.offset, &pointer_offset, dist_sq, self.line_color);
                    }
                }
            }
        }

        // Draw lines between particles
        for (i, p) in self.particles.iter().enumerate() {
            let cell_x = (p.offset[0] / CELL_SIZE) as i32;
            let cell_y = (p.offset[1] / CELL_SIZE) as i32;

            // Check neighboring cells (including own)
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(neighbors) = self.grid.get(&(cell_x + dx, cell_y + dy)) {
                        for j in neighbors {
                            if *j <= i {
                                continue; // Avoid duplicate connections
                            }
                            let q = &self.particles[*j];
                            let dx = p.offset[0] - q.offset[0];
                            let dy = p.offset[1] - q.offset[1];
                            let dist_sq = dx * dx + dy * dy;
                            Self::draw_line(&mut img, &p.offset, &q.offset, dist_sq, self.line_color);
                        }
                    }
                }
            }

            // Draw particle
            Self::draw_particle(&mut img, &p.offset, self.particle_color);
        }

        img
    }
}

impl Iterator for ParticleNetworkConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        // Update particle positions
        for p in &mut self.particles {
            p.next_step();
        }

        // Update virtual pointers and attract nearby particles
        if self.pointer_enabled {
            self.update_pointers();
        }

        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for ParticleNetworkConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Initialize particles with random positions
        self.update_particles();

        // Initialize virtual pointers
        if self.pointer_enabled {
            self.init_pointers();
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.particles.clear();
        self.grid.clear();
        self.pointers.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(ParticleNetworkConfig);