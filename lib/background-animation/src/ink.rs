use crate::{AnimationInit, noise};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InkStyle {
    #[default]
    InkOnPaper, // White background + black ink (traditional)
    PaperOnInk, // Black background + white ink (inverted)
}

impl InkStyle {
    pub fn ink_color(self) -> (u8, u8, u8) {
        match self {
            InkStyle::InkOnPaper => (15, 15, 20),    // Black ink
            InkStyle::PaperOnInk => (240, 240, 235), // White ink
        }
    }

    pub fn bg_color(self) -> (u8, u8, u8) {
        match self {
            InkStyle::InkOnPaper => (252, 248, 245), // Rice paper white
            InkStyle::PaperOnInk => (10, 10, 15),    // Deep black
        }
    }
}

#[derive(Debug, Clone, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
pub struct InkDrop {
    pub x: f32,
    pub y: f32,
    pub density: f32, //  (0.0 - 1.0)
    pub radius: f32,
    pub max_radius: f32,
    pub color: (u8, u8, u8),
    pub age: u32,
}

#[derive(Debug, Clone, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
pub struct InkSource {
    pub x: f32,
    pub y: f32,
    pub lifetime: u32, // Remaining lifetime in frames
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct InkDissipationConfig {
    /// Style: InkOnPaper (white bg + black ink) or PaperOnInk (black bg + white ink)
    #[derivative(Default(value = "InkStyle::default()"))]
    pub style: InkStyle,

    #[derivative(Default(value = "15"))]
    pub source_count: usize,

    /// Drop spawn probability per frame per source (default: 0.05)
    #[derivative(Default(value = "0.05"))]
    pub spawn_rate: f32,

    /// Source lifetime in frames (default: 150)
    #[derivative(Default(value = "150"))]
    pub source_lifetime: u32,

    /// Drop initial radius (default: 5.0)
    #[derivative(Default(value = "5.0"))]
    pub initial_radius: f32,

    /// Maximum diffusion radius (default: 80.0)
    #[derivative(Default(value = "80.0"))]
    pub max_radius: f32,

    /// Diffusion speed - how fast radius grows (default: 2.5)
    #[derivative(Default(value = "2.5"))]
    pub spread_rate: f32,

    /// Diffusion field blend factor (default: 0.02)
    #[derivative(Default(value = "0.02"))]
    pub diffusion_strength: f32,

    /// Fade speed - controls how fast ink disappears (default: 0.02)
    /// Value range: 0.001 (slow) to 0.1 (fast)
    #[derivative(Default(value = "0.02"))]
    pub fade_speed: f32,

    /// Simulation resolution divisor (default: 4)
    #[derivative(Default(value = "4"))]
    pub resolution_divisor: u32,

    /// Maximum number of active drops (default: 300)
    #[derivative(Default(value = "300"))]
    pub max_drops: usize,

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
    sim_width: usize,

    #[setters(skip)]
    #[serde(skip)]
    sim_height: usize,

    #[setters(skip)]
    #[serde(skip)]
    density: Vec<Vec<f32>>,

    #[setters(skip)]
    #[serde(skip)]
    color_r: Vec<Vec<f32>>,

    #[setters(skip)]
    #[serde(skip)]
    color_g: Vec<Vec<f32>>,

    #[setters(skip)]
    #[serde(skip)]
    color_b: Vec<Vec<f32>>,

    #[setters(skip)]
    #[serde(skip)]
    sources: Vec<InkSource>,

    #[setters(skip)]
    #[serde(skip)]
    drops: Vec<InkDrop>,

    #[setters(skip)]
    #[serde(skip)]
    spawn_counter: u64,
}

impl InkDissipationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn initialize_simulation(&mut self) {
        self.sim_width = (self.width / self.resolution_divisor) as usize;
        self.sim_height = (self.height / self.resolution_divisor) as usize;

        let gw = self.sim_width;
        let gh = self.sim_height;

        self.density = vec![vec![0.0; gw]; gh];
        self.color_r = vec![vec![0.0; gw]; gh];
        self.color_g = vec![vec![0.0; gw]; gh];
        self.color_b = vec![vec![0.0; gw]; gh];

        self.sources = Vec::with_capacity(self.source_count);
        self.drops = Vec::with_capacity(self.max_drops);
    }

    fn step(&mut self) {
        // 1. Spawn new ink sources
        self.spawn_sources();

        // 2. Spawn drops from sources
        self.spawn_drops();

        // 3. Diffuse drops into density field
        self.diffuse_drops();

        // 4. Density field diffusion (ink wash spread)
        self.diffuse_density();

        // 5. Global density decay
        self.decay_density();

        // 6. Update and cleanup
        self.update_and_cleanup();
    }

    fn spawn_sources(&mut self) {
        // Use current_frame + counter for better randomness
        let seed_base = self.current_frame * 10000 + self.spawn_counter as usize;
        let spawn_prob = crate::pseudo_phase(seed_base);
        if spawn_prob < self.spawn_rate && self.sources.len() < self.source_count {
            self.spawn_counter += 1;
            let seed = seed_base + 1;

            // Use combination of frame and counter for truly random positions
            let x = crate::pseudo_density(seed * 7 + 13) * self.width as f32;
            let y = crate::pseudo_density(seed * 11 + 17) * self.height as f32;

            // Use ink color from style with slight variation
            let base_color = self.style.ink_color();
            let variation = crate::pseudo_density(seed * 3 + 5) * 30.0;
            let color = (
                (base_color.0 as f32 + variation).clamp(0.0, 255.0) as u8,
                (base_color.1 as f32 + variation).clamp(0.0, 255.0) as u8,
                (base_color.2 as f32 + variation).clamp(0.0, 255.0) as u8,
            );

            self.sources.push(InkSource {
                x,
                y,
                lifetime: self.source_lifetime,
                color,
            });
        }
    }

    fn spawn_drops(&mut self) {
        for source in &self.sources {
            if source.lifetime == 0 || self.drops.len() >= self.max_drops {
                continue;
            }

            self.spawn_counter += 1;
            // Combine frame, counter, and source position for randomness
            let seed = self.current_frame * 10000 + self.spawn_counter as usize;

            // Spawn probability - higher at source lifetime start
            let lifetime_ratio = source.lifetime as f32 / self.source_lifetime as f32;
            let spawn_prob = crate::pseudo_density(seed * 5 + 7);
            let spawn_threshold = 0.4 * lifetime_ratio;

            if spawn_prob < spawn_threshold {
                // Random offset around source - larger offset for more spread
                let offset_x = (crate::pseudo_phase(seed * 13 + 11) - 0.5) * 40.0;
                let offset_y = (crate::pseudo_phase(seed * 17 + 13) - 0.5) * 40.0;

                self.drops.push(InkDrop {
                    x: source.x + offset_x,
                    y: source.y + offset_y,
                    density: 1.0,
                    radius: self.initial_radius,
                    max_radius: self.max_radius
                        * (0.5 + crate::pseudo_density(seed * 19 + 17) * 0.5),
                    color: source.color,
                    age: 0,
                });
            }
        }
    }

    fn diffuse_drops(&mut self) {
        let gw = self.sim_width;
        let gh = self.sim_height;
        let res_div = self.resolution_divisor as f32;

        for drop in &mut self.drops {
            // Radius increases over time
            drop.radius += self.spread_rate;
            drop.radius = drop.radius.min(drop.max_radius);

            // Density decreases as drop spreads (area increases)
            let area_ratio = (drop.radius / self.initial_radius).powi(2);
            drop.density = 1.0 / area_ratio.sqrt().max(1.0);

            // Age increases
            drop.age += 1;
        }

        // Deposit drop density to field
        for drop in &self.drops {
            if drop.density < 0.01 {
                continue;
            }

            let color_r_norm = drop.color.0 as f32 / 255.0;
            let color_g_norm = drop.color.1 as f32 / 255.0;
            let color_b_norm = drop.color.2 as f32 / 255.0;

            let sim_x = (drop.x / res_div) as i32;
            let sim_y = (drop.y / res_div) as i32;
            let radius_cells = (drop.radius / res_div).ceil() as i32 + 2;

            for dy in -radius_cells..=radius_cells {
                for dx in -radius_cells..=radius_cells {
                    let cell_x = sim_x + dx;
                    let cell_y = sim_y + dy;

                    if cell_x < 0 || cell_x >= gw as i32 || cell_y < 0 || cell_y >= gh as i32 {
                        continue;
                    }

                    let px = cell_x as f32 * res_div;
                    let py = cell_y as f32 * res_div;

                    let dist = ((px - drop.x).powi(2) + (py - drop.y).powi(2)).sqrt();
                    if dist > drop.radius {
                        continue;
                    }

                    // Gaussian distribution - center has higher density
                    let sigma = drop.radius * 0.5;
                    let gaussian = (-dist.powi(2) / (2.0 * sigma.powi(2))).exp();

                    let deposit_amount = drop.density * gaussian * 0.15;

                    let cx = cell_x as usize;
                    let cy = cell_y as usize;

                    self.density[cy][cx] += deposit_amount;

                    // Blend color weighted by density
                    let total = self.density[cy][cx];
                    if total > 0.0001 {
                        let existing_weight = (total - deposit_amount) / total;
                        let new_weight = deposit_amount / total;

                        self.color_r[cy][cx] =
                            self.color_r[cy][cx] * existing_weight + color_r_norm * new_weight;
                        self.color_g[cy][cx] =
                            self.color_g[cy][cx] * existing_weight + color_g_norm * new_weight;
                        self.color_b[cy][cx] =
                            self.color_b[cy][cx] * existing_weight + color_b_norm * new_weight;
                    }
                }
            }
        }
    }

    fn diffuse_density(&mut self) {
        let gw = self.sim_width;
        let gh = self.sim_height;
        let diff = self.diffusion_strength;

        // Use Gauss-Seidel iteration for stable diffusion
        let iterations = 4;

        for _ in 0..iterations {
            let mut new_density = vec![vec![0.0; gw]; gh];
            let mut new_r = vec![vec![0.0; gw]; gh];
            let mut new_g = vec![vec![0.0; gw]; gh];
            let mut new_b = vec![vec![0.0; gw]; gh];

            new_density
                .par_iter_mut()
                .zip(new_r.par_iter_mut())
                .zip(new_g.par_iter_mut())
                .zip(new_b.par_iter_mut())
                .enumerate()
                .for_each(|(y, (((new_d_row, new_r_row), new_g_row), new_b_row))| {
                    for x in 0..gw {
                        let left = self.density[y][(x + gw - 1) % gw];
                        let right = self.density[y][(x + 1) % gw];
                        let up = self.density[(y + gh - 1) % gh][x];
                        let down = self.density[(y + 1) % gh][x];
                        let center = self.density[y][x];

                        let neighbors_avg = (left + right + up + down) / 4.0;
                        new_d_row[x] = center * (1.0 - diff) + neighbors_avg * diff;

                        // Also diffuse colors
                        let r_neighbors_avg = (self.color_r[y][(x + gw - 1) % gw]
                            + self.color_r[y][(x + 1) % gw]
                            + self.color_r[(y + gh - 1) % gh][x]
                            + self.color_r[(y + 1) % gh][x])
                            / 4.0;
                        new_r_row[x] = self.color_r[y][x] * (1.0 - diff) + r_neighbors_avg * diff;

                        let g_neighbors_avg = (self.color_g[y][(x + gw - 1) % gw]
                            + self.color_g[y][(x + 1) % gw]
                            + self.color_g[(y + gh - 1) % gh][x]
                            + self.color_g[(y + 1) % gh][x])
                            / 4.0;
                        new_g_row[x] = self.color_g[y][x] * (1.0 - diff) + g_neighbors_avg * diff;

                        let b_neighbors_avg = (self.color_b[y][(x + gw - 1) % gw]
                            + self.color_b[y][(x + 1) % gw]
                            + self.color_b[(y + gh - 1) % gh][x]
                            + self.color_b[(y + 1) % gh][x])
                            / 4.0;
                        new_b_row[x] = self.color_b[y][x] * (1.0 - diff) + b_neighbors_avg * diff;
                    }
                });

            self.density = new_density;
            self.color_r = new_r;
            self.color_g = new_g;
            self.color_b = new_b;
        }
    }

    fn decay_density(&mut self) {
        let decay = self.fade_speed;

        self.density.par_iter_mut().for_each(|row| {
            for cell in row.iter_mut() {
                *cell *= 1.0 - decay;
            }
        });

        self.color_r.par_iter_mut().for_each(|row| {
            for cell in row.iter_mut() {
                *cell *= 1.0 - decay * 0.5;
            }
        });

        self.color_g.par_iter_mut().for_each(|row| {
            for cell in row.iter_mut() {
                *cell *= 1.0 - decay * 0.5;
            }
        });

        self.color_b.par_iter_mut().for_each(|row| {
            for cell in row.iter_mut() {
                *cell *= 1.0 - decay * 0.5;
            }
        });
    }

    fn update_and_cleanup(&mut self) {
        // Update sources
        for source in &mut self.sources {
            if source.lifetime > 0 {
                source.lifetime -= 1;
            }
        }

        // Remove dead sources
        self.sources.retain(|s| s.lifetime > 0);

        // Remove drops that have fully dissipated
        self.drops
            .retain(|d| d.radius < d.max_radius * 1.5 && d.density > 0.01);
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background with style-defined background color
        let bg = self.style.bg_color();
        for pixel in img.pixels_mut() {
            *pixel = Rgba([bg.0, bg.1, bg.2, 255]);
        }

        let res_div = self.resolution_divisor;
        let fiber_scale = 0.003; // Paper fiber texture scale

        // Render density field with paper texture
        for (sim_y, row) in self.density.iter().enumerate() {
            for (sim_x, d) in row.iter().enumerate() {
                if *d < 0.001 {
                    continue;
                }

                let r = self.color_r[sim_y][sim_x].clamp(0.0, 1.0);
                let g = self.color_g[sim_y][sim_x].clamp(0.0, 1.0);
                let b = self.color_b[sim_y][sim_x].clamp(0.0, 1.0);

                for py in 0..res_div {
                    for px in 0..res_div {
                        let out_x = sim_x as u32 * res_div + px;
                        let out_y = sim_y as u32 * res_div + py;

                        if out_x >= self.width || out_y >= self.height {
                            continue;
                        }

                        // Paper fiber texture noise
                        let fiber_noise = noise::noise3d(
                            out_x as f32 * fiber_scale,
                            out_y as f32 * fiber_scale,
                            0.0,
                        );

                        // Grain noise for paper texture
                        let grain = noise::noise3d(
                            out_x as f32 * 0.1,
                            out_y as f32 * 0.1,
                            self.current_frame as f32 * 0.01,
                        );

                        // Combine effects
                        let texture_factor = 1.0 + fiber_noise * 0.1 + grain * 0.05;
                        let final_intensity = *d * texture_factor;

                        // Soft edges at cell boundaries
                        let edge_dist_x = if px < res_div / 2 {
                            px as f32
                        } else {
                            (res_div - px) as f32
                        };
                        let edge_dist_y = if py < res_div / 2 {
                            py as f32
                        } else {
                            (res_div - py) as f32
                        };
                        let edge_factor = (edge_dist_x / (res_div as f32 / 2.0))
                            .min(edge_dist_y / (res_div as f32 / 2.0));
                        let soft_edge = 0.7 + edge_factor * 0.3;

                        let blend = (final_intensity * soft_edge).clamp(0.0, 1.0);

                        let existing = img.get_pixel(out_x, out_y);

                        // Ink color with texture variation
                        let ink_r = (r * 255.0 + fiber_noise * 10.0).clamp(0.0, 255.0) as u8;
                        let ink_g = (g * 255.0 + fiber_noise * 10.0).clamp(0.0, 255.0) as u8;
                        let ink_b = (b * 255.0 + fiber_noise * 10.0).clamp(0.0, 255.0) as u8;

                        let new_r = (existing.0[0] as f32 * (1.0 - blend) + ink_r as f32 * blend)
                            .min(255.0) as u8;
                        let new_g = (existing.0[1] as f32 * (1.0 - blend) + ink_g as f32 * blend)
                            .min(255.0) as u8;
                        let new_b = (existing.0[2] as f32 * (1.0 - blend) + ink_b as f32 * blend)
                            .min(255.0) as u8;

                        img.put_pixel(out_x, out_y, Rgba([new_r, new_g, new_b, 255]));
                    }
                }
            }
        }

        img
    }
}

impl Iterator for InkDissipationConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        self.step();
        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for InkDissipationConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.spawn_counter = 0;

        self.initialize_simulation();

        // Spawn initial sources
        for _i in 0..(self.source_count / 3).max(1) {
            self.spawn_counter += 1;
            let seed = self.spawn_counter;

            let x = crate::pseudo_density(seed as usize) * width as f32;
            let y = crate::pseudo_density((seed + 1) as usize) * height as f32;

            // Use style-based ink color with variation
            let base_color = self.style.ink_color();
            let variation = crate::pseudo_density((seed + 2) as usize) * 30.0;
            let color = (
                (base_color.0 as f32 + variation).clamp(0.0, 255.0) as u8,
                (base_color.1 as f32 + variation).clamp(0.0, 255.0) as u8,
                (base_color.2 as f32 + variation).clamp(0.0, 255.0) as u8,
            );

            self.sources.push(InkSource {
                x,
                y,
                lifetime: self.source_lifetime,
                color,
            });
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.density.clear();
        self.color_r.clear();
        self.color_g.clear();
        self.color_b.clear();
        self.sources.clear();
        self.drops.clear();
        self.spawn_counter = 0;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(InkDissipationConfig);
