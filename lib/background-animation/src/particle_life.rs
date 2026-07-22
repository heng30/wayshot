// https://github.com/tom-mohr/particle-life-app.git

use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use rand::{RngExt, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// A single particle with position, velocity, and type
#[derive(Debug, Clone, Copy)]
struct Particle {
    /// Position in unit space [0, 1]
    x: f32,
    y: f32,
    /// Velocity in unit space
    vx: f32,
    vy: f32,
    /// Particle type (0 to type_count-1)
    ty: usize,
}

/// Interaction matrix defining attraction/repulsion between particle types
/// matrix[i][j] = attraction of type i towards type j
/// Positive = attraction, Negative = repulsion, Range [-1, 1]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Matrix {
    size: usize,
    #[serde(default)]
    values: Vec<Vec<f32>>,
}

impl Matrix {
    fn new(size: usize) -> Self {
        let values = vec![vec![0.0; size]; size];
        Self { size, values }
    }

    fn randomize(&mut self, seed: u64) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        for i in 0..self.size {
            for j in 0..self.size {
                self.values[i][j] = rng.random_range(-1.0..1.0);
            }
        }
    }

    fn get(&self, i: usize, j: usize) -> f32 {
        self.values[i][j]
    }
}

/// Particle Life simulation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ParticleLifeConfig {
    /// Number of particles (default: 5000)
    #[derivative(Default(value = "5000"))]
    pub particle_count: usize,

    /// Number of particle types/colors (default: 6)
    #[derivative(Default(value = "6"))]
    pub type_count: usize,

    /// Maximum interaction radius in unit space (default: 0.04)
    /// This is scaled to pixels for the simulation
    #[derivative(Default(value = "0.04"))]
    pub rmax: f32,

    /// Friction coefficient (default: 0.85)
    /// 1.0 = no friction, 0.0 = immediate stop
    #[derivative(Default(value = "0.85"))]
    pub friction: f32,

    /// Force multiplier (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub force: f32,

    /// Time step per frame (default: 0.05)
    #[derivative(Default(value = "0.05"))]
    pub dt: f32,

    /// Wrap positions at boundaries (default: true)
    #[derivative(Default(value = "true"))]
    pub wrap: bool,

    /// Particle size in pixels (default: 4.0)
    #[derivative(Default(value = "4.0"))]
    pub particle_size: f32,

    /// Color palette for particle types (RGBA)
    /// Default: vibrant rainbow palette
    #[derivative(Default(value = "vec![
        (255, 100, 100, 255),  // Red
        (100, 255, 100, 255),  // Green
        (100, 100, 255, 255),  // Blue
        (255, 255, 100, 255),  // Yellow
        (255, 100, 255, 255),  // Magenta
        (100, 255, 255, 255),  // Cyan
    ]"))]
    pub colors: Vec<(u8, u8, u8, u8)>,

    /// Background color (RGB)
    #[derivative(Default(value = "(10, 15, 25)"))]
    pub bg_color: (u8, u8, u8),

    /// Seed for reproducible matrix generation (default: 42)
    #[derivative(Default(value = "42"))]
    pub matrix_seed: u64,

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
    matrix: Matrix,

    /// Spatial container indices for neighbor lookup optimization
    /// Each entry stores the end index (exclusive) for particles in that container
    #[setters(skip)]
    #[serde(skip)]
    containers: Vec<usize>,

    /// Sorted particle indices for spatial lookup
    #[setters(skip)]
    #[serde(skip)]
    particle_indices: Vec<usize>,
}

impl ParticleLifeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the force on a particle from a neighbor
    /// Based on the reference implementation's accelerator
    fn accelerate(a: f32, rel_x: f32, rel_y: f32) -> (f32, f32) {
        let beta = 0.3; // Repulsion threshold
        let dist = (rel_x * rel_x + rel_y * rel_y).sqrt();

        if dist < 0.0001 {
            return (0.0, 0.0);
        }

        // Normalize relative position
        let nx = rel_x / dist;
        let ny = rel_y / dist;

        // Force calculation:
        // When close (dist < beta): repulsion (negative force)
        // When far (dist >= beta): attraction/repulsion based on matrix value a
        let force = if dist < beta {
            dist / beta - 1.0 // Repulsion near center
        } else {
            // Linear falloff with matrix influence
            a * (1.0 - ((1.0 + beta - 2.0 * dist).abs()) / (1.0 - beta))
        };

        (nx * force, ny * force)
    }

    /// Build spatial containers for efficient neighbor lookup
    fn build_containers(&mut self) {
        // Container size should be at least rmax for efficient lookup
        let container_size = self.rmax;
        let nx = (1.0 / container_size).ceil() as usize;
        let ny = nx; // Square grid

        let num_containers = nx * ny;

        // Reset containers
        self.containers.clear();
        self.containers.resize(num_containers, 0);

        // Count particles per container
        for p in &self.particles {
            let cx = (p.x / container_size).min(nx as f32 - 1.0) as usize;
            let cy = (p.y / container_size).min(ny as f32 - 1.0) as usize;
            let ci = cx + cy * nx;
            self.containers[ci] += 1;
        }

        // Convert counts to cumulative end indices
        let mut offset = 0;
        for i in 0..num_containers {
            let count = self.containers[i];
            self.containers[i] = offset;
            offset += count;
        }

        // Sort particle indices into containers
        self.particle_indices.clear();
        self.particle_indices.resize(self.particles.len(), 0);

        // We need to track where to insert each particle
        let mut insert_pos = vec![0usize; num_containers];
        for (i, p) in self.particles.iter().enumerate() {
            let cx = (p.x / container_size).min(nx as f32 - 1.0) as usize;
            let cy = (p.y / container_size).min(ny as f32 - 1.0) as usize;
            let ci = cx + cy * nx;

            // Get the start position for this container
            let start = if ci == 0 { 0 } else { self.containers[ci - 1] };
            let pos = start + insert_pos[ci];
            self.particle_indices[pos] = i;
            insert_pos[ci] += 1;
        }

        // Update containers to hold end indices (exclusive)
        // containers[i] now represents the end index for container i
        let mut cumulative = 0;
        for i in 0..num_containers {
            cumulative += insert_pos[i];
            self.containers[i] = cumulative;
        }
    }

    /// Update velocities based on particle interactions
    fn update_velocities(&mut self) {
        let container_size = self.rmax;
        let nx = (1.0 / container_size).ceil() as usize;
        let ny = nx;
        let rmax = self.rmax;
        let rmax_sq = rmax * rmax;
        let force = self.force;
        let dt = self.dt;
        let wrap = self.wrap;
        let friction_factor = self.friction.powf(60.0 * dt);

        // Store new velocities separately to avoid race conditions
        let mut new_velocities: Vec<(f32, f32)> =
            self.particles.iter().map(|p| (p.vx, p.vy)).collect();

        new_velocities
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, (vx, vy))| {
                let p = &self.particles[i];

                // Apply friction first
                *vx *= friction_factor;
                *vy *= friction_factor;

                // Get container coordinates for this particle
                let cx0 = (p.x / container_size).min(nx as f32 - 1.0) as usize;
                let cy0 = (p.y / container_size).min(ny as f32 - 1.0) as usize;

                // Check 9 neighboring containers (including own)
                for dcy in -1i32..=1 {
                    for dcx in -1i32..=1 {
                        let cx = if wrap {
                            Self::wrap_container((cx0 as i32 + dcx) as usize, nx)
                        } else {
                            let c = cx0 as i32 + dcx;
                            if c < 0 || c >= nx as i32 {
                                continue;
                            }
                            c as usize
                        };

                        let cy = if wrap {
                            Self::wrap_container((cy0 as i32 + dcy) as usize, ny)
                        } else {
                            let c = cy0 as i32 + dcy;
                            if c < 0 || c >= ny as i32 {
                                continue;
                            }
                            c as usize
                        };

                        let ci = cx + cy * nx;

                        // Get particles in this container
                        let start = if ci == 0 { 0 } else { self.containers[ci - 1] };
                        let end = self.containers[ci];

                        for j in start..end {
                            let q_idx = self.particle_indices[j];
                            if i == q_idx {
                                continue;
                            }

                            let q = &self.particles[q_idx];

                            // Calculate relative position (connection)
                            let mut rel_x = q.x - p.x;
                            let mut rel_y = q.y - p.y;

                            // Wrap connection if enabled
                            if wrap {
                                if rel_x < -0.5 {
                                    rel_x += 1.0;
                                } else if rel_x >= 0.5 {
                                    rel_x -= 1.0;
                                }
                                if rel_y < -0.5 {
                                    rel_y += 1.0;
                                } else if rel_y >= 0.5 {
                                    rel_y -= 1.0;
                                }
                            }

                            let dist_sq = rel_x * rel_x + rel_y * rel_y;

                            // Only interact if within rmax
                            if dist_sq > 0.0 && dist_sq <= rmax_sq {
                                // Normalize by rmax for accelerate function
                                let norm_x = rel_x / rmax;
                                let norm_y = rel_y / rmax;

                                // Get matrix value for this type pair
                                let a = self.matrix.get(p.ty, q.ty);

                                // Calculate force
                                let (fx, fy) = Self::accelerate(a, norm_x, norm_y);

                                // Apply force to velocity
                                *vx += fx * rmax * force * dt;
                                *vy += fy * rmax * force * dt;
                            }
                        }
                    }
                }
            });

        // Update particle velocities
        for (i, (vx, vy)) in new_velocities.iter().enumerate() {
            self.particles[i].vx = *vx;
            self.particles[i].vy = *vy;
        }
    }

    /// Update positions based on velocities
    fn update_positions(&mut self) {
        let dt = self.dt;
        let wrap = self.wrap;

        self.particles.par_iter_mut().for_each(|p| {
            // Update position: pos += vel * dt
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            // Ensure position is in valid range
            if wrap {
                // Wrap to [0, 1)
                while p.x < 0.0 {
                    p.x += 1.0;
                }
                while p.x >= 1.0 {
                    p.x -= 1.0;
                }
                while p.y < 0.0 {
                    p.y += 1.0;
                }
                while p.y >= 1.0 {
                    p.y -= 1.0;
                }
            } else {
                // Clamp to [0, 1]
                p.x = p.x.clamp(0.0, 1.0);
                p.y = p.y.clamp(0.0, 1.0);
            }
        });
    }

    /// Wrap container index for toroidal space
    fn wrap_container(c: usize, n: usize) -> usize {
        // Handle signed offsets that might go negative
        let c_signed = c as i64;
        let n_signed = n as i64;
        if c_signed < 0 {
            (c_signed + n_signed) as usize
        } else if c_signed >= n_signed {
            (c_signed - n_signed) as usize
        } else {
            c
        }
    }

    /// Generate a frame from current particle positions
    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        let width_f = self.width as f32;
        let height_f = self.height as f32;
        let radius = (self.particle_size / 2.0) as i32;

        // Draw each particle
        for p in &self.particles {
            let px = (p.x * width_f) as i32;
            let py = (p.y * height_f) as i32;

            // Get color for this particle type
            let color_idx = p.ty % self.colors.len();
            let color = self.colors[color_idx];

            // Draw filled circle
            if radius > 0 {
                Self::draw_circle(&mut img, px, py, radius, color);
            }
        }

        img
    }

    /// Draw a filled circle at the given position
    fn draw_circle(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: (u8, u8, u8, u8)) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px = cx + dx;
                let py = cy + dy;

                if px < 0 || px >= img.width() as i32 || py < 0 || py >= img.height() as i32 {
                    continue;
                }

                // Check if inside circle with soft edge
                let dist_sq = dx * dx + dy * dy;
                let radius_sq = radius * radius;

                if dist_sq <= radius_sq {
                    // Soft edge: fade at boundary
                    let edge_dist = (dist_sq as f32).sqrt() / radius as f32;
                    let alpha = if edge_dist > 0.8 {
                        1.0 - (edge_dist - 0.8) / 0.2
                    } else {
                        1.0
                    };

                    let existing = img.get_pixel(px as u32, py as u32);
                    let blend_alpha = alpha * (color.3 as f32 / 255.0);
                    let bg_alpha = 1.0 - blend_alpha;

                    let new_r = ((existing.0[0] as f32 * bg_alpha + color.0 as f32 * blend_alpha)
                        .min(255.0)) as u8;
                    let new_g = ((existing.0[1] as f32 * bg_alpha + color.1 as f32 * blend_alpha)
                        .min(255.0)) as u8;
                    let new_b = ((existing.0[2] as f32 * bg_alpha + color.2 as f32 * blend_alpha)
                        .min(255.0)) as u8;

                    img.put_pixel(px as u32, py as u32, Rgba([new_r, new_g, new_b, 255]));
                }
            }
        }
    }
}

impl Iterator for ParticleLifeConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        // Physics update: build containers -> update velocities -> update positions
        self.build_containers();
        self.update_velocities();
        self.update_positions();

        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for ParticleLifeConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Ensure colors are set
        if self.colors.is_empty() {
            self.colors = vec![
                (255, 100, 100, 255),
                (100, 255, 100, 255),
                (100, 100, 255, 255),
                (255, 255, 100, 255),
                (255, 100, 255, 255),
                (100, 255, 255, 255),
            ];
        }

        // Initialize matrix
        self.matrix = Matrix::new(self.type_count);
        self.matrix.randomize(self.matrix_seed);

        // Initialize particles with proper random positions
        self.particles = Vec::with_capacity(self.particle_count);
        let mut rng = rand::rngs::StdRng::seed_from_u64(self.matrix_seed);

        for _ in 0..self.particle_count {
            let x = rng.random_range(0.0..1.0);
            let y = rng.random_range(0.0..1.0);
            let ty = rng.random_range(0..self.type_count);

            self.particles.push(Particle {
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                ty,
            });
        }

        // Initialize spatial container structures
        let container_size = self.rmax;
        let nx = (1.0 / container_size).ceil() as usize;
        let num_containers = nx * nx;
        self.containers = Vec::with_capacity(num_containers);
        self.particle_indices = Vec::with_capacity(self.particle_count);
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.particles.clear();
        self.containers.clear();
        self.particle_indices.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(ParticleLifeConfig);

