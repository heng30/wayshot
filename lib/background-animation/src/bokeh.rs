use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
pub struct BokehSpot {
    /// Position as percentage of width/height
    x_pct: f32,
    y_pct: f32,
    /// Size in pixels (current animated size)
    size: f32,
    /// Base size for animation (original size before breathing effect)
    base_size: f32,
    /// Movement direction
    dx: f32,
    dy: f32,
    /// Color
    color: (u8, u8, u8, u8),
    /// Phase for animation
    phase: f32,
    /// Opacity
    opacity: f32,
    /// Depth layer (affects blur and opacity)
    depth: f32,
    /// Shape type (0: circle, 1: hexagon, 2: soft circle)
    shape: u32,
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct BokehConfig {
    /// Number of bokeh spots (default: 30)
    #[derivative(Default(value = "30"))]
    pub spot_count: usize,

    /// Minimum spot size in pixels (default: 20)
    #[derivative(Default(value = "20.0"))]
    pub min_size: f32,

    /// Maximum spot size in pixels (default: 100)
    #[derivative(Default(value = "100.0"))]
    pub max_size: f32,

    /// Blur radius for spots (default: 0, computed from size)
    #[derivative(Default(value = "0.0"))]
    pub blur_radius: f32,

    /// Color palette for spots (default: warm night colors)
    #[derivative(Default(value = "vec![
        (255, 220, 180, 150),  // Warm white
        (255, 200, 100, 120),  // Orange
        (255, 180, 80, 100),   // Amber
        (200, 255, 220, 80),   // Cool white
        (255, 150, 150, 90),   // Pink
        (150, 200, 255, 70),   // Blue
        (255, 255, 200, 130),  // Bright yellow
    ]"))]
    pub colors: Vec<(u8, u8, u8, u8)>,

    /// Background color (default: dark night sky)
    #[derivative(Default(value = "(5, 10, 20)"))]
    pub bg_color: (u8, u8, u8),

    /// Animation speed (default: 0.1)
    #[derivative(Default(value = "0.1"))]
    pub animation_speed: f32,

    /// Enable hexagonal bokeh (default: false)
    #[derivative(Default(value = "false"))]
    pub hexagonal_enabled: bool,

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
    spots: Vec<BokehSpot>,
}

impl BokehConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn update_spots(&mut self) {
        let time = self.current_frame as f32 / self.fps as f32;

        self.spots.par_iter_mut().for_each(|spot| {
            // Gentle floating movement
            let oscillation_x = (time * 0.5 + spot.phase).sin() * 0.3;
            let oscillation_y = (time * 0.7 + spot.phase * 1.3).sin() * 0.2;

            spot.x_pct += (spot.dx + oscillation_x * 0.001) * self.animation_speed;
            spot.y_pct += (spot.dy + oscillation_y * 0.001) * self.animation_speed;

            // Wrap around with fade at edges
            if spot.x_pct < -0.1 {
                spot.x_pct = 1.1;
            } else if spot.x_pct > 1.1 {
                spot.x_pct = -0.1;
            }

            if spot.y_pct < -0.1 {
                spot.y_pct = 1.1;
            } else if spot.y_pct > 1.1 {
                spot.y_pct = -0.1;
            }

            // Pulsing brightness (like twinkling lights)
            let twinkle_speed = 1.0 + spot.depth * 2.0;
            spot.opacity = 0.5 + 0.5 * (time * twinkle_speed + spot.phase).sin();

            // Size breathing effect - use base_size as reference
            let size_variation = 1.0 + 0.1 * (time * 0.3 + spot.phase * 2.0).sin();
            spot.size = spot.base_size * size_variation;
        });
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Sort spots by depth for proper rendering order
        let sorted_spots: Vec<&BokehSpot> = self.spots.iter().collect();

        // Draw each spot
        for spot in sorted_spots {
            let x = (spot.x_pct * self.width as f32) as i32;
            let y = (spot.y_pct * self.height as f32) as i32;

            // Depth affects size and opacity
            let depth_factor = 0.5 + spot.depth * 0.5;
            let effective_size = spot.size * depth_factor;

            // Compute blur radius (larger spots = more blur)
            let blur_radius = if self.blur_radius > 0.0 {
                self.blur_radius
            } else {
                effective_size * 0.3 * (1.0 - spot.depth)
            };

            let opacity = spot.opacity * (spot.color.3 as f32 / 255.0) * depth_factor;
            let color = Rgba([
                spot.color.0,
                spot.color.1,
                spot.color.2,
                (opacity * 255.0).min(255.0) as u8,
            ]);

            let radius = (effective_size / 2.0) as i32;
            if radius > 0 {
                match spot.shape {
                    0 | 2 => {
                        // Soft circle with blur
                        Self::draw_soft_circle(&mut img, x, y, radius, blur_radius, color);
                    }
                    1 => {
                        // Hexagonal bokeh
                        Self::draw_hexagon(&mut img, x, y, radius, color);
                    }
                    _ => {}
                }
            }
        }

        img
    }

    fn draw_soft_circle(
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        radius: i32,
        blur_radius: f32,
        color: Rgba<u8>,
    ) {
        let total_radius = (radius as f32 + blur_radius) as i32;

        for dy in -total_radius..=total_radius {
            for dx in -total_radius..=total_radius {
                let px = cx + dx;
                let py = cy + dy;

                if px < 0 || px >= img.width() as i32 || py < 0 || py >= img.height() as i32 {
                    continue;
                }

                let dist = ((dx * dx + dy * dy) as f32).sqrt();

                // Soft edge: gradient from solid center to transparent edge
                let gradient_factor = if dist <= radius as f32 {
                    1.0 // Solid center
                } else if dist <= total_radius as f32 {
                    // Soft blur edge
                    let edge_dist = (dist - radius as f32) / blur_radius;
                    1.0 - edge_dist.powi(2)
                } else {
                    0.0
                };

                if gradient_factor <= 0.0 {
                    continue;
                }

                let base_alpha = color.0[3] as f32 / 255.0;
                let alpha = base_alpha * gradient_factor;

                // Brightness boost at center
                let brightness = 1.0 + 0.3 * (1.0 - dist / total_radius as f32);

                let existing = img.get_pixel(px as u32, py as u32);
                let blend_alpha = alpha;
                let bg_alpha = 1.0 - blend_alpha;

                let new_r = ((existing.0[0] as f32 * bg_alpha
                    + color.0[0] as f32 * blend_alpha * brightness)
                    .min(255.0)) as u8;
                let new_g = ((existing.0[1] as f32 * bg_alpha
                    + color.0[1] as f32 * blend_alpha * brightness)
                    .min(255.0)) as u8;
                let new_b = ((existing.0[2] as f32 * bg_alpha
                    + color.0[2] as f32 * blend_alpha * brightness)
                    .min(255.0)) as u8;

                img.put_pixel(px as u32, py as u32, Rgba([new_r, new_g, new_b, 255]));
            }
        }
    }

    fn draw_hexagon(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
        // Fill hexagon using point-in-polygon test
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px = cx + dx;
                let py = cy + dy;

                if px < 0 || px >= img.width() as i32 || py < 0 || py >= img.height() as i32 {
                    continue;
                }

                // Check if point is inside hexagon
                let inside = Self::point_in_hexagon(dx as f32, dy as f32, radius as f32);

                if !inside {
                    continue;
                }

                // Distance from center for gradient
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                let edge_factor = 1.0 - (dist / radius as f32).powi(3);

                let base_alpha = color.0[3] as f32 / 255.0;
                let alpha = base_alpha * edge_factor;

                let brightness = 1.0 + 0.4 * (1.0 - dist / radius as f32);

                let existing = img.get_pixel(px as u32, py as u32);
                let blend_alpha = alpha;
                let bg_alpha = 1.0 - blend_alpha;

                let new_r = ((existing.0[0] as f32 * bg_alpha
                    + color.0[0] as f32 * blend_alpha * brightness)
                    .min(255.0)) as u8;
                let new_g = ((existing.0[1] as f32 * bg_alpha
                    + color.0[1] as f32 * blend_alpha * brightness)
                    .min(255.0)) as u8;
                let new_b = ((existing.0[2] as f32 * bg_alpha
                    + color.0[2] as f32 * blend_alpha * brightness)
                    .min(255.0)) as u8;

                img.put_pixel(px as u32, py as u32, Rgba([new_r, new_g, new_b, 255]));
            }
        }
    }

    fn point_in_hexagon(x: f32, y: f32, radius: f32) -> bool {
        // Hexagon check using inequalities
        let q = x.abs() / radius;
        let r = y.abs() / radius;
        let s = (x + y).abs() / radius;

        // For a regular hexagon centered at origin
        q <= 1.0 && r <= 1.0 && s <= 1.5
    }
}

impl Iterator for BokehConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        self.update_spots();
        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for BokehConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        if self.colors.is_empty() {
            self.colors = vec![
                (255, 220, 180, 150),
                (255, 200, 100, 120),
                (255, 180, 80, 100),
            ];
        }

        // Initialize spots
        self.spots = Vec::with_capacity(self.spot_count);
        for i in 0..self.spot_count {
            let color_idx = i % self.colors.len();
            let size = self.min_size
                + (self.max_size - self.min_size) * crate::pseudo_phase(i);
            let phase = crate::pseudo_phase(i + 100) * std::f32::consts::TAU;

            // Depth determines focus layer (0 = foreground, 1 = background)
            let depth = crate::pseudo_phase(i + 200);

            // Shape type
            let shape = if self.hexagonal_enabled && i % 3 == 0 {
                1 // Hexagon
            } else if i % 5 == 0 {
                2 // Extra soft
            } else {
                0 // Normal soft circle
            };

            // Movement direction (slow drift)
            let angle = crate::pseudo_phase(i + 300) * std::f32::consts::TAU;
            let speed = 0.002 * (0.3 + crate::pseudo_phase(i + 400) * 0.7);
            let dx = angle.cos() * speed;
            let dy = angle.sin() * speed;

            // Initial position
            let x_pct = crate::pseudo_phase(i + 500);
            let y_pct = crate::pseudo_phase(i + 600);

            self.spots.push(BokehSpot {
                x_pct,
                y_pct,
                size,
                base_size: size,
                dx,
                dy,
                color: self.colors[color_idx],
                phase,
                opacity: 1.0,
                depth,
                shape,
            });
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.spots.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(BokehConfig);