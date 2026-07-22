use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Force source type for fluid simulation
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum ForceSource {
    #[default]
    Random, // Random force application
    Circular,    // Circular motion source
    Vortices,    // Multiple vortex centers
    MouseDriven, // External control (placeholder)
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FluidConfig {
    /// Simulation resolution divisor (default: 8)
    #[derivative(Default(value = "8"))]
    pub resolution_divisor: u32,

    /// Viscosity (fluid thickness) (default: 0.0001)
    #[derivative(Default(value = "0.01"))]
    pub viscosity: f32,

    /// Diffusion rate for density (default: 0.0001)
    #[derivative(Default(value = "0.01"))]
    pub diffusion: f32,

    /// Force source type (default: Random)
    #[derivative(Default)]
    pub force_source: ForceSource,

    /// Number of force sources (default: 3)
    #[derivative(Default(value = "3"))]
    pub num_sources: usize,

    /// Color palette for density visualization (default: smoke/ink colors)
    #[derivative(Default(value = "vec![
        (10, 20, 40),    // Low density - dark blue
        (30, 50, 80),    // Medium-low
        (60, 90, 120),   // Medium
        (100, 130, 160), // Medium-high
        (150, 170, 190), // High
        (200, 210, 220), // Very high
        (250, 250, 255), // Maximum - bright
    ]"))]
    pub colors: Vec<(u8, u8, u8)>,

    /// Background color (default: dark)
    #[derivative(Default(value = "(5, 10, 20)"))]
    pub bg_color: (u8, u8, u8),

    /// Steps per frame (default: 4)
    #[derivative(Default(value = "4"))]
    pub steps_per_frame: usize,

    /// Color injection mode (default: true - adds colors over time)
    #[derivative(Default(value = "true"))]
    pub color_injection: bool,

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

    // Velocity field (vx, vy)
    #[setters(skip)]
    #[serde(skip)]
    vx: Vec<Vec<f32>>,

    #[setters(skip)]
    #[serde(skip)]
    vy: Vec<Vec<f32>>,

    // Density field (for visualization)
    #[setters(skip)]
    #[serde(skip)]
    density: Vec<Vec<f32>>,

    // Color field (RGB per cell)
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
    sources: Vec<(usize, usize, f32, f32, (f32, f32, f32))>, // (x, y, force_x, force_y, color)
}

impl FluidConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn initialize_simulation(&mut self) {
        self.sim_width = (self.width / self.resolution_divisor) as usize;
        self.sim_height = (self.height / self.resolution_divisor) as usize;

        let gw = self.sim_width;
        let gh = self.sim_height;

        // Initialize all fields to zero
        self.vx = vec![vec![0.0; gw]; gh];
        self.vy = vec![vec![0.0; gw]; gh];
        self.density = vec![vec![0.0; gw]; gh];
        self.color_r = vec![vec![0.0; gw]; gh];
        self.color_g = vec![vec![0.0; gw]; gh];
        self.color_b = vec![vec![0.0; gw]; gh];

        // Initialize force sources
        self.sources = Vec::with_capacity(self.num_sources);
        for i in 0..self.num_sources {
            let x = (gw as f32 * crate::pseudo_phase(i)) as usize;
            let y = (gh as f32 * crate::pseudo_phase(i + 100)) as usize;

            // Random color for this source
            let r = 0.3 + 0.7 * crate::pseudo_phase(i + 200);
            let g = 0.3 + 0.7 * crate::pseudo_phase(i + 300);
            let b = 0.3 + 0.7 * crate::pseudo_phase(i + 400);

            self.sources.push((x, y, 0.0, 0.0, (r, g, b)));
        }
    }

    fn step(&mut self) {
        let time = self.current_frame as f32 / self.fps as f32;

        // Apply forces from sources
        self.apply_forces(time);

        // Diffusion step
        self.diffuse_velocity();
        self.diffuse_density();

        // Advection step (move fluid along velocity field)
        self.advect_velocity();
        self.advect_density();

        // Project velocity field (make it divergence-free)
        self.project();
    }

    fn apply_forces(&mut self, time: f32) {
        let gw = self.sim_width;
        let gh = self.sim_height;

        for (i, source) in self.sources.iter_mut().enumerate() {
            let (x, y, _, _, color) = *source;

            // Calculate force based on source type
            let (fx, fy) = match self.force_source {
                ForceSource::Random => {
                    let angle = (time * 2.0 + crate::pseudo_phase(i) * std::f32::consts::TAU)
                        % std::f32::consts::TAU;
                    let strength = 0.5 + 0.5 * (time * 5.0 + i as f32).sin();
                    (angle.cos() * strength, angle.sin() * strength)
                }
                ForceSource::Circular => {
                    let cx = gw as f32 / 2.0;
                    let cy = gh as f32 / 2.0;
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cy;
                    let angle = dy.atan2(dx) + std::f32::consts::FRAC_PI_2;
                    (angle.cos() * 0.3, angle.sin() * 0.3)
                }
                ForceSource::Vortices => {
                    let phase = time * 3.0 + crate::pseudo_phase(i) * std::f32::consts::TAU;
                    let strength = 0.4;
                    let spiral = phase.sin() * 0.3;
                    (
                        (phase + std::f32::consts::FRAC_PI_4).cos() * strength + spiral,
                        (phase + std::f32::consts::FRAC_PI_4).sin() * strength + spiral,
                    )
                }
                ForceSource::MouseDriven => {
                    // Placeholder - would need external input
                    (0.0, 0.0)
                }
            };

            *source = (x, y, fx, fy, color);

            // Apply force and add density at source location
            let radius = 3;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let px = ((x as i32 + dx).max(0) as usize).min(gw - 1);
                    let py = ((y as i32 + dy).max(0) as usize).min(gh - 1);

                    let dist_factor = 1.0 - (dx * dx + dy * dy) as f32 / (radius * radius) as f32;
                    if dist_factor > 0.0 {
                        self.vx[py][px] += fx * dist_factor * 0.1;
                        self.vy[py][px] += fy * dist_factor * 0.1;
                        self.density[py][px] += dist_factor * 0.5;

                        if self.color_injection {
                            self.color_r[py][px] += color.0 * dist_factor * 0.5;
                            self.color_g[py][px] += color.1 * dist_factor * 0.5;
                            self.color_b[py][px] += color.2 * dist_factor * 0.5;
                        }
                    }
                }
            }

            // Move source slowly
            let new_x = ((x as f32 + fx * 0.5 + gw as f32) % gw as f32) as usize;
            let new_y = ((y as f32 + fy * 0.5 + gh as f32) % gh as f32) as usize;
            *source = (new_x, new_y, fx, fy, color);
        }
    }

    fn diffuse_velocity(&mut self) {
        let gw = self.sim_width;
        let gh = self.sim_height;
        let visc = self.viscosity;

        // Simple diffusion using Jacobi iteration
        let iterations = 4;

        for _ in 0..iterations {
            let mut new_vx = vec![vec![0.0; gw]; gh];
            let mut new_vy = vec![vec![0.0; gw]; gh];

            new_vx
                .par_iter_mut()
                .zip(new_vy.par_iter_mut())
                .enumerate()
                .for_each(|(y, (new_vx_row, new_vy_row))| {
                    for x in 0..gw {
                        let left = self.vx[y][(x + gw - 1) % gw];
                        let right = self.vx[y][(x + 1) % gw];
                        let up = self.vx[(y + gh - 1) % gh][x];
                        let down = self.vx[(y + 1) % gh][x];
                        let center = self.vx[y][x];

                        new_vx_row[x] = (center + visc * (left + right + up + down - 4.0 * center))
                            / (1.0 + 4.0 * visc);

                        let left_vy = self.vy[y][(x + gw - 1) % gw];
                        let right_vy = self.vy[y][(x + 1) % gw];
                        let up_vy = self.vy[(y + gh - 1) % gh][x];
                        let down_vy = self.vy[(y + 1) % gh][x];
                        let center_vy = self.vy[y][x];

                        new_vy_row[x] = (center_vy
                            + visc * (left_vy + right_vy + up_vy + down_vy - 4.0 * center_vy))
                            / (1.0 + 4.0 * visc);
                    }
                });

            self.vx = new_vx;
            self.vy = new_vy;
        }
    }

    fn diffuse_density(&mut self) {
        let gw = self.sim_width;
        let gh = self.sim_height;
        let diff = self.diffusion;

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
                        // Density diffusion
                        let left = self.density[y][(x + gw - 1) % gw];
                        let right = self.density[y][(x + 1) % gw];
                        let up = self.density[(y + gh - 1) % gh][x];
                        let down = self.density[(y + 1) % gh][x];
                        let center = self.density[y][x];

                        new_d_row[x] = (center + diff * (left + right + up + down - 4.0 * center))
                            / (1.0 + 4.0 * diff);

                        // Color diffusion
                        if self.color_injection {
                            new_r_row[x] = (self.color_r[y][x]
                                + diff
                                    * (self.color_r[y][(x + gw - 1) % gw]
                                        + self.color_r[y][(x + 1) % gw]
                                        + self.color_r[(y + gh - 1) % gh][x]
                                        + self.color_r[(y + 1) % gh][x]
                                        - 4.0 * self.color_r[y][x]))
                                / (1.0 + 4.0 * diff);

                            new_g_row[x] = (self.color_g[y][x]
                                + diff
                                    * (self.color_g[y][(x + gw - 1) % gw]
                                        + self.color_g[y][(x + 1) % gw]
                                        + self.color_g[(y + gh - 1) % gh][x]
                                        + self.color_g[(y + 1) % gh][x]
                                        - 4.0 * self.color_g[y][x]))
                                / (1.0 + 4.0 * diff);

                            new_b_row[x] = (self.color_b[y][x]
                                + diff
                                    * (self.color_b[y][(x + gw - 1) % gw]
                                        + self.color_b[y][(x + 1) % gw]
                                        + self.color_b[(y + gh - 1) % gh][x]
                                        + self.color_b[(y + 1) % gh][x]
                                        - 4.0 * self.color_b[y][x]))
                                / (1.0 + 4.0 * diff);
                        }
                    }
                });

            self.density = new_density;
            if self.color_injection {
                self.color_r = new_r;
                self.color_g = new_g;
                self.color_b = new_b;
            }
        }
    }

    fn advect_velocity(&mut self) {
        let gw = self.sim_width;
        let gh = self.sim_height;

        let mut new_vx = vec![vec![0.0; gw]; gh];
        let mut new_vy = vec![vec![0.0; gw]; gh];

        new_vx
            .par_iter_mut()
            .zip(new_vy.par_iter_mut())
            .enumerate()
            .for_each(|(y, (new_vx_row, new_vy_row))| {
                for x in 0..gw {
                    // Trace back through velocity field
                    let px = x as f32 - self.vx[y][x];
                    let py = y as f32 - self.vy[y][x];

                    // Clamp to grid bounds
                    let px = px.clamp(0.0, (gw - 1) as f32);
                    let py = py.clamp(0.0, (gh - 1) as f32);

                    // Interpolate (bilinear)
                    let i0 = px as usize;
                    let j0 = py as usize;
                    let i1 = (i0 + 1).min(gw - 1);
                    let j1 = (j0 + 1).min(gh - 1);

                    let s1 = px - i0 as f32;
                    let s0 = 1.0 - s1;
                    let t1 = py - j0 as f32;
                    let t0 = 1.0 - t1;

                    new_vx_row[x] = s0 * (t0 * self.vx[j0][i0] + t1 * self.vx[j1][i0])
                        + s1 * (t0 * self.vx[j0][i1] + t1 * self.vx[j1][i1]);

                    new_vy_row[x] = s0 * (t0 * self.vy[j0][i0] + t1 * self.vy[j1][i0])
                        + s1 * (t0 * self.vy[j0][i1] + t1 * self.vy[j1][i1]);
                }
            });

        self.vx = new_vx;
        self.vy = new_vy;
    }

    fn advect_density(&mut self) {
        let gw = self.sim_width;
        let gh = self.sim_height;

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
                    // Trace back through velocity field
                    let px = x as f32 - self.vx[y][x];
                    let py = y as f32 - self.vy[y][x];

                    let px = px.clamp(0.0, (gw - 1) as f32);
                    let py = py.clamp(0.0, (gh - 1) as f32);

                    let i0 = px as usize;
                    let j0 = py as usize;
                    let i1 = (i0 + 1).min(gw - 1);
                    let j1 = (j0 + 1).min(gh - 1);

                    let s1 = px - i0 as f32;
                    let s0 = 1.0 - s1;
                    let t1 = py - j0 as f32;
                    let t0 = 1.0 - t1;

                    // Density advection
                    new_d_row[x] = s0 * (t0 * self.density[j0][i0] + t1 * self.density[j1][i0])
                        + s1 * (t0 * self.density[j0][i1] + t1 * self.density[j1][i1]);

                    // Color advection
                    if self.color_injection {
                        new_r_row[x] = s0 * (t0 * self.color_r[j0][i0] + t1 * self.color_r[j1][i0])
                            + s1 * (t0 * self.color_r[j0][i1] + t1 * self.color_r[j1][i1]);

                        new_g_row[x] = s0 * (t0 * self.color_g[j0][i0] + t1 * self.color_g[j1][i0])
                            + s1 * (t0 * self.color_g[j0][i1] + t1 * self.color_g[j1][i1]);

                        new_b_row[x] = s0 * (t0 * self.color_b[j0][i0] + t1 * self.color_b[j1][i0])
                            + s1 * (t0 * self.color_b[j0][i1] + t1 * self.color_b[j1][i1]);
                    }
                }
            });

        self.density = new_density;
        if self.color_injection {
            self.color_r = new_r;
            self.color_g = new_g;
            self.color_b = new_b;
        }
    }

    fn project(&mut self) {
        // Make velocity field divergence-free (incompressible)
        let gw = self.sim_width;
        let gh = self.sim_height;

        // Calculate divergence
        let mut div = vec![vec![0.0; gw]; gh];
        let mut p = vec![vec![0.0; gw]; gh];

        div.par_iter_mut().enumerate().for_each(|(y, div_row)| {
            for x in 0..gw {
                div_row[x] = -0.5
                    * (self.vx[y][(x + 1) % gw] - self.vx[y][(x + gw - 1) % gw]
                        + self.vy[(y + 1) % gh][x]
                        - self.vy[(y + gh - 1) % gh][x]);
            }
        });

        // Solve pressure field using Jacobi iteration
        let iterations = 4;
        for _ in 0..iterations {
            let mut new_p = vec![vec![0.0; gw]; gh];

            new_p.par_iter_mut().enumerate().for_each(|(y, p_row)| {
                for x in 0..gw {
                    p_row[x] = (div[y][x]
                        + p[(y + gh - 1) % gh][x]
                        + p[(y + 1) % gh][x]
                        + p[y][(x + gw - 1) % gw]
                        + p[y][(x + 1) % gw])
                        / 4.0;
                }
            });

            p = new_p;
        }

        // Subtract pressure gradient from velocity
        self.vx.par_iter_mut().enumerate().for_each(|(y, vx_row)| {
            for x in 0..gw {
                vx_row[x] -= 0.5 * (p[y][(x + 1) % gw] - p[y][(x + gw - 1) % gw]);
            }
        });

        self.vy.par_iter_mut().enumerate().for_each(|(y, vy_row)| {
            for x in 0..gw {
                vy_row[x] -= 0.5 * (p[(y + 1) % gh][x] - p[(y + gh - 1) % gh][x]);
            }
        });
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        let res_div = self.resolution_divisor;

        // Map simulation to output image
        for (sim_y, row) in self.density.iter().enumerate() {
            for (sim_x, d) in row.iter().enumerate() {
                let density_val = *d;

                // Get color: either from color field or from palette
                let color = if self.color_injection {
                    let r = self.color_r[sim_y][sim_x];
                    let g = self.color_g[sim_y][sim_x];
                    let b = self.color_b[sim_y][sim_x];

                    let intensity = density_val.clamp(0.0, 1.0);
                    (
                        ((r * intensity * 255.0).min(255.0)
                            + self.bg_color.0 as f32 * (1.0 - intensity))
                            as u8,
                        ((g * intensity * 255.0).min(255.0)
                            + self.bg_color.1 as f32 * (1.0 - intensity))
                            as u8,
                        ((b * intensity * 255.0).min(255.0)
                            + self.bg_color.2 as f32 * (1.0 - intensity))
                            as u8,
                    )
                } else {
                    Self::density_to_color(density_val, &self.colors, self.bg_color)
                };

                // Draw scaled cell
                for py in 0..res_div {
                    for px in 0..res_div {
                        let out_x = sim_x as u32 * res_div + px;
                        let out_y = sim_y as u32 * res_div + py;

                        if out_x < self.width && out_y < self.height {
                            img.put_pixel(out_x, out_y, Rgba([color.0, color.1, color.2, 255]));
                        }
                    }
                }
            }
        }

        img
    }

    fn density_to_color(density: f32, colors: &[(u8, u8, u8)], bg: (u8, u8, u8)) -> (u8, u8, u8) {
        let intensity = density.clamp(0.0, 1.0);

        if colors.is_empty() {
            let v = (intensity * 255.0).min(255.0) as u8;
            return (
                (bg.0 as f32 * (1.0 - intensity) + v as f32 * intensity) as u8,
                (bg.1 as f32 * (1.0 - intensity) + v as f32 * intensity) as u8,
                (bg.2 as f32 * (1.0 - intensity) + v as f32 * intensity) as u8,
            );
        }

        let idx = (intensity * (colors.len() - 1) as f32).round() as usize;
        let idx = idx.min(colors.len() - 1);

        let fg = colors[idx];
        (
            (bg.0 as f32 * (1.0 - intensity) + fg.0 as f32 * intensity) as u8,
            (bg.1 as f32 * (1.0 - intensity) + fg.1 as f32 * intensity) as u8,
            (bg.2 as f32 * (1.0 - intensity) + fg.2 as f32 * intensity) as u8,
        )
    }
}

impl Iterator for FluidConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        for _ in 0..self.steps_per_frame {
            self.step();
        }

        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for FluidConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        self.initialize_simulation();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.vx.clear();
        self.vy.clear();
        self.density.clear();
        self.sources.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(FluidConfig);

