use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
pub struct LightFlare {
    /// Position as percentage of width/height
    x_pct: f32,
    y_pct: f32,
    /// Size in pixels
    size: f32,
    /// Movement direction
    dx: f32,
    dy: f32,
    /// Color
    color: (u8, u8, u8, u8),
    /// Phase for animation variation
    phase: f32,
    /// Type of flare (0: circular, 1: elliptical, 2: gradient band)
    flare_type: u32,
    /// Opacity
    opacity: f32,
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct LightEffectsConfig {
    /// Number of light flares (default: 5)
    #[derivative(Default(value = "5"))]
    pub flare_count: usize,

    /// Minimum flare size in pixels (default: 50)
    #[derivative(Default(value = "50.0"))]
    pub min_size: f32,

    /// Maximum flare size in pixels (default: 200)
    #[derivative(Default(value = "200.0"))]
    pub max_size: f32,

    #[derivative(Default(value = "1.0"))]
    pub movement_speed: f32,

    /// Color palette for flares (default: warm light leak colors)
    #[derivative(Default(value = "vec![
        (255, 200, 100, 80),  // Warm orange
        (255, 150, 50, 60),   // Orange
        (255, 100, 50, 50),   // Red-orange
        (200, 255, 150, 70),  // Green-yellow
        (150, 200, 255, 60),  // Blue-white
        (255, 255, 200, 90),  // Bright yellow
    ]"))]
    pub colors: Vec<(u8, u8, u8, u8)>,

    /// Background color (default: dark)
    #[derivative(Default(value = "(15, 15, 25)"))]
    pub bg_color: (u8, u8, u8),

    /// Enable elliptical flares (default: true)
    #[derivative(Default(value = "true"))]
    pub elliptical_enabled: bool,

    /// Enable gradient bands (default: true)
    #[derivative(Default(value = "true"))]
    pub bands_enabled: bool,

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
    flares: Vec<LightFlare>,
}

impl LightEffectsConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn update_flares(&mut self) {
        self.flares.iter_mut().for_each(|flare| {
            // Animate position with slight oscillation
            let time = self.current_frame as f32 / self.fps as f32;
            let oscillation = (time * 2.0 + flare.phase).sin() * 0.5;

            flare.x_pct += flare.dx * self.movement_speed * (1.0 + oscillation);
            flare.y_pct += flare.dy * self.movement_speed;

            // Wrap around
            if flare.x_pct < -0.2 {
                flare.x_pct = 1.2;
            } else if flare.x_pct > 1.2 {
                flare.x_pct = -0.2;
            }

            if flare.y_pct < -0.2 {
                flare.y_pct = 1.2;
            } else if flare.y_pct > 1.2 {
                flare.y_pct = -0.2;
            }

            // Pulsing opacity
            flare.opacity = 0.6 + 0.4 * (time * 1.5 + flare.phase).sin();
        });
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Draw each flare
        for flare in &self.flares {
            let x = (flare.x_pct * self.width as f32) as i32;
            let y = (flare.y_pct * self.height as f32) as i32;

            let opacity = flare.opacity * (flare.color.3 as f32 / 255.0);
            let color = Rgba([
                flare.color.0,
                flare.color.1,
                flare.color.2,
                (opacity * 255.0) as u8,
            ]);

            match flare.flare_type {
                0 => {
                    // Circular flare
                    let radius = (flare.size / 2.0) as i32;
                    if radius > 0 {
                        Self::draw_gradient_circle(&mut img, x, y, radius, color);
                    }
                }
                1 => {
                    // Elliptical flare
                    let rx = (flare.size / 2.0) as i32;
                    let ry = (flare.size / 3.0) as i32;
                    if rx > 0 && ry > 0 {
                        Self::draw_ellipse(&mut img, x, y, rx, ry, color);
                    }
                }
                2 => {
                    // Gradient band (horizontal or diagonal)
                    Self::draw_gradient_band(&mut img, y, flare.size, color, flare.phase);
                }
                _ => {}
            }
        }

        img
    }

    fn draw_gradient_circle(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
        // Draw filled circle with gradient from center
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px = cx + dx;
                let py = cy + dy;

                if px < 0 || px >= img.width() as i32 || py < 0 || py >= img.height() as i32 {
                    continue;
                }

                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist > radius as f32 {
                    continue;
                }

                // Radial gradient: center is brightest, edges fade
                let gradient_factor = 1.0 - (dist / radius as f32).powi(2);

                let base_alpha = color.0[3] as f32 / 255.0;
                let alpha = base_alpha * gradient_factor;

                // Blend with existing pixel
                let existing = img.get_pixel(px as u32, py as u32);
                let blend_alpha = alpha;
                let bg_alpha = 1.0 - blend_alpha;

                let new_r =
                    (existing.0[0] as f32 * bg_alpha + color.0[0] as f32 * blend_alpha) as u8;
                let new_g =
                    (existing.0[1] as f32 * bg_alpha + color.0[1] as f32 * blend_alpha) as u8;
                let new_b =
                    (existing.0[2] as f32 * bg_alpha + color.0[2] as f32 * blend_alpha) as u8;

                img.put_pixel(px as u32, py as u32, Rgba([new_r, new_g, new_b, 255]));
            }
        }
    }

    fn draw_ellipse(img: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba<u8>) {
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let px = cx + dx;
                let py = cy + dy;

                if px < 0 || px >= img.width() as i32 || py < 0 || py >= img.height() as i32 {
                    continue;
                }

                // Ellipse equation: (x/rx)^2 + (y/ry)^2 <= 1
                let ellipse_dist =
                    (dx as f32 / rx as f32).powi(2) + (dy as f32 / ry as f32).powi(2);
                if ellipse_dist > 1.0 {
                    continue;
                }

                // Gradient based on distance from center
                let gradient_factor = 1.0 - ellipse_dist.powi(1);

                let base_alpha = color.0[3] as f32 / 255.0;
                let alpha = base_alpha * gradient_factor;

                let existing = img.get_pixel(px as u32, py as u32);
                let blend_alpha = alpha;
                let bg_alpha = 1.0 - blend_alpha;

                let new_r =
                    (existing.0[0] as f32 * bg_alpha + color.0[0] as f32 * blend_alpha) as u8;
                let new_g =
                    (existing.0[1] as f32 * bg_alpha + color.0[1] as f32 * blend_alpha) as u8;
                let new_b =
                    (existing.0[2] as f32 * bg_alpha + color.0[2] as f32 * blend_alpha) as u8;

                img.put_pixel(px as u32, py as u32, Rgba([new_r, new_g, new_b, 255]));
            }
        }
    }

    fn draw_gradient_band(
        img: &mut RgbaImage,
        y_center: i32,
        size: f32,
        color: Rgba<u8>,
        phase: f32,
    ) {
        let half_size = (size / 2.0) as i32;

        for y in (y_center - half_size).max(0)..(y_center + half_size).min(img.height() as i32) {
            // Gradient strength based on distance from center
            let dist = (y - y_center).abs() as f32 / half_size as f32;
            let gradient_factor = 1.0 - dist.powi(2);

            // Add slight horizontal variation
            let time_var = phase * 0.1;

            for x in 0..img.width() {
                let x_variation = (x as f32 * 0.01 + time_var).sin() * 0.3 + 0.7;
                let final_factor = gradient_factor * x_variation;

                let base_alpha = color.0[3] as f32 / 255.0;
                let alpha = base_alpha * final_factor;

                let existing = img.get_pixel(x, y as u32);
                let blend_alpha = alpha;
                let bg_alpha = 1.0 - blend_alpha;

                let new_r =
                    (existing.0[0] as f32 * bg_alpha + color.0[0] as f32 * blend_alpha) as u8;
                let new_g =
                    (existing.0[1] as f32 * bg_alpha + color.0[1] as f32 * blend_alpha) as u8;
                let new_b =
                    (existing.0[2] as f32 * bg_alpha + color.0[2] as f32 * blend_alpha) as u8;

                img.put_pixel(x, y as u32, Rgba([new_r, new_g, new_b, 255]));
            }
        }
    }
}

impl Iterator for LightEffectsConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        self.update_flares();
        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for LightEffectsConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        if self.colors.is_empty() {
            self.colors = vec![(255, 200, 100, 80), (255, 150, 50, 60), (255, 100, 50, 50)];
        }

        // Initialize flares
        self.flares = Vec::with_capacity(self.flare_count);
        for i in 0..self.flare_count {
            let color_idx = i % self.colors.len();
            let size = self.min_size + (self.max_size - self.min_size) * crate::pseudo_phase(i);
            let phase = crate::pseudo_phase(i + 100) * std::f32::consts::TAU;

            // Determine flare type
            let flare_type = if self.bands_enabled && i % 3 == 2 {
                2 // Gradient band
            } else if self.elliptical_enabled && i % 2 == 1 {
                1 // Elliptical
            } else {
                0 // Circular
            };

            // Movement direction
            let angle = crate::pseudo_phase(i + 200) * std::f32::consts::TAU;
            let dx = angle.cos() * 0.005;
            let dy = angle.sin() * 0.005;

            // Initial position (spread across the canvas)
            let x_pct = crate::pseudo_phase(i + 300);
            let y_pct = crate::pseudo_phase(i + 400);

            self.flares.push(LightFlare {
                x_pct,
                y_pct,
                size,
                dx,
                dy,
                color: self.colors[color_idx],
                phase,
                flare_type,
                opacity: 1.0,
            });
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.flares.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(LightEffectsConfig);

