//! Galaxy background animation.
//!
//! Creates a mesmerizing 3D rotating galaxy effect based on tmp/galaxy CSS reference:
//! - 200 stars distributed on a sphere via 3D rotation + translation
//! - Each star has a mirrored counterpart (scale -1,-1,-1) on the opposite side
//! - The entire galaxy continuously rotates around the (1,1,1) axis
//! - Stars have colored glow effects (box-shadow equivalent)
//! - An "appear" animation scales from 0 to 1 over the first 10 seconds
//! - A breathing scale animation pulses the galaxy size
//!
//! The CSS uses `perspective: 100vmin` and `transform-style: preserve-3d`.
//! We replicate this by projecting 3D star positions to 2D with perspective projection.

use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// A single star in the galaxy, defined by its spherical position and visual properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GalaxyStar {
    /// Star radius in vmin units (CSS --size)
    size: f32,
    /// Background color (hex RGB)
    color: (u8, u8, u8),
    /// Shadow/glow color (hex RGB)
    shadow_color: (u8, u8, u8),
    /// Shadow spread in vmin units
    shadow_spread: f32,
    /// Shadow blur in vmin units
    shadow_blur: f32,
    /// Euler rotation angles (rx, ry, rz) in degrees for positioning on sphere
    rx: f32,
    ry: f32,
    rz: f32,
    /// Translation distance along Z axis in vmin units
    tz: f32,
}

/// Uniform random in [a, b]
fn rand_range(rng: &mut impl RngExt, a: f32, b: f32) -> f32 {
    rng.random::<f32>() * (b - a) + a
}

/// Parse a hex color string like "#642674" to (u8, u8, u8)
fn parse_hex_color(s: &str) -> (u8, u8, u8) {
    let s = s.trim_start_matches('#');
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
    (r, g, b)
}

/// Default galaxy color palette — matches the CSS reference
const GALAXY_COLORS: &[&str] = &[
    "#642674", "#fff299", "#8380f5", "#f2eda3", "#722b83",
    "#eee784", "#5a2268", "#6b297c", "#ff5880", "#8df48e",
    "#13a9e9", "#12a2e0", "#ea1b90", "#dd1486", "#f4efb0",
    "#f46f6f", "#ff76ad", "#8885f5", "#f3efac", "#8df48e",
    "#fff4ad", "#6c69f3", "#b2f7b3", "#f1f4f7", "#eee784",
    "#7e7bf5", "#f68c8c", "#ff8fbc", "#fbfcfd", "#ff5880",
];

/// Galaxy animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GalaxyConfig {
    /// Number of stars (default: 200, matching CSS reference)
    #[derivative(Default(value = "200"))]
    pub star_count: usize,

    /// Background color (default: deep purple #0e0220)
    #[derivative(Default(value = "(14, 2, 32)"))]
    pub bg_color: (u8, u8, u8),

    /// Rotation period in seconds (default: 10s, matching CSS `rotation 10s`)
    #[derivative(Default(value = "10.0"))]
    pub rotation_period: f32,

    /// Appear animation duration in seconds (default: 10s, matching CSS)
    #[derivative(Default(value = "10.0"))]
    pub appear_duration: f32,

    /// Breathing scale period in seconds (default: 0, disabled)
    /// The CSS has a `scale` animation but it's not applied to any element in the HTML.
    /// Enable this to add a pulsing effect.
    #[derivative(Default(value = "0.0"))]
    pub breathing_period: f32,

    /// Breathing scale minimum (default: 0.75)
    #[derivative(Default(value = "0.75"))]
    pub breathing_min: f32,

    /// Perspective distance in vmin (default: 100, matching CSS `perspective: 100vmin`)
    #[derivative(Default(value = "100.0"))]
    pub perspective: f32,

    /// Minimum star distance in vmin (default: 65)
    #[derivative(Default(value = "65.0"))]
    pub min_distance: f32,

    /// Maximum star distance in vmin (default: 90)
    #[derivative(Default(value = "90.0"))]
    pub max_distance: f32,

    /// Minimum star size in vmin (default: 0.25)
    #[derivative(Default(value = "0.25"))]
    pub min_size: f32,

    /// Maximum star size in vmin (default: 0.75)
    #[derivative(Default(value = "0.75"))]
    pub max_size: f32,

    /// Glow intensity multiplier (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub glow_intensity: f32,

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

    /// Pre-generated star data
    #[setters(skip)]
    #[serde(skip)]
    stars: Vec<GalaxyStar>,

    /// Vmin scale factor (pixels per vmin unit)
    #[setters(skip)]
    #[serde(skip)]
    vmin_scale: f32,

    /// Scaled perspective distance in pixels
    #[setters(skip)]
    #[serde(skip)]
    s_perspective: f32,
}

impl GalaxyConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate random stars with positions on a sphere
    fn create_stars(&mut self) {
        self.stars.clear();
        let mut rng = rand::rng();

        for i in 0..self.star_count {
            // Random rotation angles for positioning on the sphere
            let rx = rand_range(&mut rng, 0.0, 360.0);
            let ry = rand_range(&mut rng, 0.0, 360.0);
            let rz = rand_range(&mut rng, 0.0, 360.0);

            // Random distance from center
            let tz = rand_range(&mut rng, self.min_distance, self.max_distance);

            // Random size
            let size = rand_range(&mut rng, self.min_size, self.max_size);

            // Pick color from palette
            let color_hex = GALAXY_COLORS[i % GALAXY_COLORS.len()];
            let color = parse_hex_color(color_hex);

            // Shadow color is slightly brighter/different shade
            // In the CSS, shadow color varies per star; we'll lighten the base color
            let shadow_color = lighten_color(color, 0.2);

            // Shadow blur and spread proportional to star size
            let shadow_blur = size * rand_range(&mut rng, 0.4, 1.4);
            let shadow_spread = size * rand_range(&mut rng, 0.03, 0.45);

            self.stars.push(GalaxyStar {
                size,
                color,
                shadow_color,
                shadow_spread,
                shadow_blur,
                rx,
                ry,
                rz,
                tz,
            });
        }
    }

    /// Compute the 3D position of a star after its positioning rotation,
    /// then apply the global galaxy rotation, and project to 2D.
    ///
    /// Returns (screen_x, screen_y, screen_scale, depth) where:
    /// - screen_x, screen_y: projected position in pixel coordinates
    /// - screen_scale: perspective scale factor (closer = larger)
    /// - depth: Z coordinate after all transforms (for depth sorting)
    fn project_star(
        &self,
        star: &GalaxyStar,
        global_angle: f32,
        mirror: bool,
    ) -> (f32, f32, f32, f32) {
        let vmin = self.vmin_scale;
        let cx = self.width as f32 / 2.0;
        let cy = self.height as f32 / 2.0;

        // Step 1: Position star on sphere via individual rotation + Z-translation
        // CSS: rotate3d(1,0,0,rx) rotate3d(0,1,0,ry) rotate3d(0,0,1,rz) translate3d(0,0,tz)
        // Start at (0, 0, tz * vmin)
        let tz_px = star.tz * vmin;
        let mut pos = [0.0f32, 0.0, tz_px];

        // Apply individual rotation rz around Z axis
        pos = rotate_z(pos, star.rz.to_radians());
        // Apply individual rotation ry around Y axis
        pos = rotate_y(pos, star.ry.to_radians());
        // Apply individual rotation rx around X axis
        pos = rotate_x(pos, star.rx.to_radians());

        // If mirror (the ::after pseudo-element), apply scale3d(-1,-1,-1)
        if mirror {
            pos[0] = -pos[0];
            pos[1] = -pos[1];
            pos[2] = -pos[2];
        }

        // Step 2: Apply global galaxy rotation around axis (1,1,1) normalized
        // CSS: rotation animation = rotate3d(1, 1, 1, angle)
        let axis = [1.0f32, 1.0, 1.0];
        let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        let norm_axis = [axis[0] / len, axis[1] / len, axis[2] / len];
        pos = rotate_around_axis(pos, norm_axis, global_angle);

        // Step 3: Perspective projection
        // CSS perspective: 100vmin means the viewer is at z = -perspective
        // Projection: x' = x * d / (d + z), y' = y * d / (d + z)
        // where d = perspective distance
        let d = self.s_perspective;
        let denom = d + pos[2];

        let screen_scale = if denom > 0.1 { d / denom } else { d / 0.1 };
        let screen_x = cx + pos[0] * screen_scale;
        let screen_y = cy + pos[1] * screen_scale;

        (screen_x, screen_y, screen_scale, pos[2])
    }

    /// Draw a single star with glow effect
    fn draw_star(
        &self,
        img: &mut RgbaImage,
        sx: f32,
        sy: f32,
        scale: f32,
        star: &GalaxyStar,
    ) {
        let vmin = self.vmin_scale;
        let radius = star.size * vmin * scale * 0.5;
        let glow_blur = star.shadow_blur * vmin * scale * self.glow_intensity;
        let glow_spread = star.shadow_spread * vmin * scale * self.glow_intensity;

        // Total rendering radius: star + glow
        let total_radius = radius + glow_blur + glow_spread;

        let w = img.width() as f32;
        let h = img.height() as f32;

        let x_min = ((sx - total_radius - 1.0).max(0.0)) as u32;
        let x_max = ((sx + total_radius + 1.0).min(w - 1.0)) as u32;
        let y_min = ((sy - total_radius - 1.0).max(0.0)) as u32;
        let y_max = ((sy + total_radius + 1.0).min(h - 1.0)) as u32;

        let star_color = Rgba([star.color.0, star.color.1, star.color.2, 255]);
        let glow_color = Rgba([star.shadow_color.0, star.shadow_color.1, star.shadow_color.2, 255]);

        for py in y_min..=y_max {
            let dy = py as f32 - sy;
            for px in x_min..=x_max {
                let dx = px as f32 - sx;
                let dist = dx.hypot(dy);

                // Core of the star (filled circle)
                let core_alpha = if dist <= radius {
                    1.0
                } else if dist < radius + 1.0 {
                    radius + 1.0 - dist // anti-alias
                } else {
                    0.0
                };

                // Glow effect (gaussian-like falloff)
                let glow_alpha = if glow_blur > 0.0 && dist > 0.0 {
                    // Gaussian: exp(-dist^2 / (2 * sigma^2))
                    // sigma is derived from glow_blur
                    let sigma = glow_blur * 0.5;
                    let falloff = (-(dist * dist) / (2.0 * sigma * sigma)).exp();
                    falloff
                } else {
                    0.0
                };

                // Add spread (uniform disc that extends to glow_spread + glow_blur)
                let spread_alpha = if glow_spread > 0.0 && dist < glow_spread + radius {
                    let spread_falloff = 1.0 - ((dist - radius) / (glow_spread + radius)).max(0.0).min(1.0);
                    spread_falloff * 0.5
                } else {
                    0.0
                };

                let total_glow = (glow_alpha + spread_alpha).min(1.0);

                if core_alpha > 0.0 {
                    blend_pixel(img, px, py, star_color, core_alpha);
                }
                if total_glow > 0.01 {
                    blend_pixel(img, px, py, glow_color, total_glow * 0.6);
                }
            }
        }
    }

    fn generate_frame(&mut self) -> RgbaImage {
        let time = self.current_frame as f32 / self.fps as f32;

        // Global rotation angle: full rotation per rotation_period
        let global_angle = (time / self.rotation_period) * std::f32::consts::TAU;

        // Appear animation: scale from 0 to 1 over appear_duration
        let appear_scale = if self.appear_duration > 0.0 {
            (time / self.appear_duration).min(1.0)
        } else {
            1.0
        };

        // Breathing scale (optional)
        let breathing_scale = if self.breathing_period > 0.0 {
            let t = (time / self.breathing_period) * std::f32::consts::TAU;
            let mid = (1.0 + self.breathing_min) / 2.0;
            let amp = (1.0 - self.breathing_min) / 2.0;
            mid + amp * t.cos()
        } else {
            1.0
        };

        let overall_scale = appear_scale * breathing_scale;

        // Create background
        let mut img = RgbaImage::new(self.width, self.height);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Project all stars (both normal and mirror) and collect for depth sorting
        #[allow(dead_code)]
        struct ProjectedStar {
            sx: f32,
            sy: f32,
            scale: f32,
            depth: f32,
            star_idx: usize,
            #[allow(dead_code)]
            mirror: bool,
        }

        let mut projected: Vec<ProjectedStar> = Vec::with_capacity(self.stars.len() * 2);

        for (i, star) in self.stars.iter().enumerate() {
            // Normal star (::before)
            let (sx, sy, scale, depth) = self.project_star(star, global_angle, false);
            if scale > 0.0 {
                projected.push(ProjectedStar {
                    sx: sx * overall_scale + (1.0 - overall_scale) * self.width as f32 / 2.0,
                    sy: sy * overall_scale + (1.0 - overall_scale) * self.height as f32 / 2.0,
                    scale: scale * overall_scale,
                    depth,
                    star_idx: i,
                    mirror: false,
                });
            }

            // Mirror star (::after with scale3d(-1,-1,-1))
            let (sx, sy, scale, depth) = self.project_star(star, global_angle, true);
            if scale > 0.0 {
                projected.push(ProjectedStar {
                    sx: sx * overall_scale + (1.0 - overall_scale) * self.width as f32 / 2.0,
                    sy: sy * overall_scale + (1.0 - overall_scale) * self.height as f32 / 2.0,
                    scale: scale * overall_scale,
                    depth,
                    star_idx: i,
                    mirror: true,
                });
            }
        }

        // Sort by depth (farthest first, so closer stars render on top)
        projected.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

        // Draw all stars
        for p in &projected {
            let star = &self.stars[p.star_idx];
            self.draw_star(&mut img, p.sx, p.sy, p.scale, star);
        }

        img
    }
}

/// Lighten a color by blending toward white
fn lighten_color(color: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    let r = color.0 as f32 + (255.0 - color.0 as f32) * amount;
    let g = color.1 as f32 + (255.0 - color.1 as f32) * amount;
    let b = color.2 as f32 + (255.0 - color.2 as f32) * amount;
    (r.round() as u8, g.round() as u8, b.round() as u8)
}

/// Blend a pixel with alpha onto the image (premultiplied alpha blending)
fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>, alpha: f32) {
    let alpha = alpha.min(1.0);
    if alpha <= 0.0 || x >= img.width() || y >= img.height() {
        return;
    }

    let src_a = alpha * (color[3] as f32 / 255.0);
    let pixel = img.get_pixel_mut(x, y);
    let dst_a = pixel[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);

    if out_a > 0.0 {
        pixel[0] = ((color[0] as f32 * src_a + pixel[0] as f32 * dst_a * (1.0 - src_a)) / out_a)
            .round() as u8;
        pixel[1] = ((color[1] as f32 * src_a + pixel[1] as f32 * dst_a * (1.0 - src_a)) / out_a)
            .round() as u8;
        pixel[2] = ((color[2] as f32 * src_a + pixel[2] as f32 * dst_a * (1.0 - src_a)) / out_a)
            .round() as u8;
        pixel[3] = (out_a * 255.0).round() as u8;
    }
}

/// Rotate point around X axis
fn rotate_x(p: [f32; 3], angle: f32) -> [f32; 3] {
    let c = angle.cos();
    let s = angle.sin();
    [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
}

/// Rotate point around Y axis
fn rotate_y(p: [f32; 3], angle: f32) -> [f32; 3] {
    let c = angle.cos();
    let s = angle.sin();
    [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
}

/// Rotate point around Z axis
fn rotate_z(p: [f32; 3], angle: f32) -> [f32; 3] {
    let c = angle.cos();
    let s = angle.sin();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
}

/// Rotate point around an arbitrary axis (Rodrigues' rotation formula)
fn rotate_around_axis(p: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let c = angle.cos();
    let s = angle.sin();
    let t = 1.0 - c;

    let [ux, uy, uz] = axis;

    // Rotation matrix from Rodrigues' formula
    let x = (t * ux * ux + c) * p[0]
        + (t * ux * uy - s * uz) * p[1]
        + (t * ux * uz + s * uy) * p[2];

    let y = (t * ux * uy + s * uz) * p[0]
        + (t * uy * uy + c) * p[1]
        + (t * uy * uz - s * ux) * p[2];

    let z = (t * ux * uz - s * uy) * p[0]
        + (t * uy * uz + s * ux) * p[1]
        + (t * uz * uz + c) * p[2];

    [x, y, z]
}

impl Iterator for GalaxyConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for GalaxyConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // 1 vmin = min(width, height) / 100 (CSS definition)
        self.vmin_scale = width.min(height) as f32 / 100.0;
        self.s_perspective = self.perspective * self.vmin_scale;

        self.create_stars();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.stars.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(GalaxyConfig);
