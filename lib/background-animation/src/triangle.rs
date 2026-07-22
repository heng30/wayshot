//! Triangle mesh background animation.
//!
//! Creates an animated Delaunay triangulation mesh with gradient coloring.
//! Based on tmp/triangle.js (Thpace):
//! - Generate random points on a grid with noise/bleed for edge coverage
//! - Compute Delaunay triangulation via `delaunator` (same algorithm as JS)
//! - Color each triangle by interpolating two gradient colors based on the
//!   centroid's diagonal position (x + y normalized)
//! - Animate vertices with per-point sinusoidal offsets (cos for X, sin for Y)
//!   where each point's phase is seeded by its X position, creating a wave
//! - Anti-aliased white stroke lines between triangles
//!
//! Optional floating particles add atmosphere (small white circles with
//! slow sinusoidal drift and random opacity/size).

use crate::aa_line::draw_line_segment_aa;
use crate::{AnimationInit, scale_pixel_for_height};
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// Triangle mesh animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TriangleConfig {
    /// Grid cell size at 1080p — controls triangle density (default: 130)
    #[derivative(Default(value = "130.0"))]
    pub triangle_size: f32,

    /// Extra margin beyond canvas edges for point generation (default: 200)
    #[derivative(Default(value = "200.0"))]
    pub bleed: f32,

    /// Random positional noise added to grid points (default: 60)
    #[derivative(Default(value = "60.0"))]
    pub noise: f32,

    /// Left/top gradient color (default: deep purple #360033)
    #[derivative(Default(value = "(54, 0, 51)"))]
    pub color1: (u8, u8, u8),

    /// Right/bottom gradient color (default: teal #0b8793)
    #[derivative(Default(value = "(11, 135, 147)"))]
    pub color2: (u8, u8, u8),

    /// Stroke color (RGBA) — line drawn between triangle edges (default: white)
    #[derivative(Default(value = "(255, 255, 255, 255)"))]
    pub stroke_color: (u8, u8, u8, u8),

    /// Stroke line width at 1080p (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub stroke_width: f32,

    /// Per-point X oscillation amplitude at 1080p (default: 20)
    #[derivative(Default(value = "20.0"))]
    pub point_variation_x: f32,

    /// Per-point Y oscillation amplitude at 1080p (default: 35)
    #[derivative(Default(value = "35.0"))]
    pub point_variation_y: f32,

    /// Point animation speed — radians per second increment (default: 15 → 0.015 rad/frame@25fps)
    #[derivative(Default(value = "15.0"))]
    pub point_animation_speed: f32,

    /// Number of floating particles (default: 80)
    #[derivative(Default(value = "80"))]
    pub particle_count: usize,

    /// Background color (RGB) (default: black)
    #[derivative(Default(value = "(0, 0, 0)"))]
    pub bg_color: (u8, u8, u8),

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

    /// Base points (grid + corner + noise), immutable after init
    #[setters(skip)]
    #[serde(skip)]
    points: Vec<[f64; 2]>,

    /// Delaunay triangles: each is 3 indices into `points`
    #[setters(skip)]
    #[serde(skip)]
    triangles: Vec<[usize; 3]>,

    /// Per-point current animation phase (incremented each frame)
    #[setters(skip)]
    #[serde(skip)]
    anim_phases: Vec<f64>,

    /// Floating particles
    #[setters(skip)]
    #[serde(skip)]
    particles: Vec<Particle>,

    /// Scaled triangle size
    #[setters(skip)]
    #[serde(skip)]
    s_triangle_size: f32,

    /// Scaled bleed
    #[setters(skip)]
    #[serde(skip)]
    s_bleed: f32,

    /// Scaled noise
    #[setters(skip)]
    #[serde(skip)]
    s_noise: f32,

    /// Scaled point variation X
    #[setters(skip)]
    #[serde(skip)]
    s_point_variation_x: f32,

    /// Scaled point variation Y
    #[setters(skip)]
    #[serde(skip)]
    s_point_variation_y: f32,

    /// Scaled point animation speed
    #[setters(skip)]
    #[serde(skip)]
    s_point_animation_speed: f32,

    /// Scaled stroke width
    #[setters(skip)]
    #[serde(skip)]
    s_stroke_width: f32,
}

/// A floating particle for atmosphere
#[derive(Debug, Clone)]
struct Particle {
    /// Origin X
    ox: f32,
    /// Origin Y
    oy: f32,
    /// Current X
    x: f32,
    /// Current Y
    y: f32,
    /// Animation period in milliseconds
    interval: f32,
    /// Drift radius
    limit: f32,
    /// Opacity (0-1)
    opacity: f32,
    /// Circle radius
    r: f32,
    /// Time accumulator in ms
    time_ms: f32,
}

impl Particle {
    fn new(width: f32, height: f32) -> Self {
        let mut rng = rand::rng();
        let ox = rand_range(&mut rng, 0.0, width);
        let oy = rand_range(&mut rng, 0.0, height);
        Self {
            ox,
            oy,
            x: ox,
            y: oy,
            interval: rand_range(&mut rng, 1000.0, 5000.0),
            limit: rand_range(&mut rng, 5.0, 15.0),
            opacity: rand_range(&mut rng, 0.1, 0.7),
            r: rand_range(&mut rng, 1.0, 2.0),
            time_ms: 0.0,
        }
    }

    fn update(&mut self, dt_ms: f32) {
        self.time_ms += dt_ms;
        self.x = self.ox + (self.time_ms / self.interval).cos() * self.limit;
        self.y = self.oy + (self.time_ms / self.interval).sin() * self.limit * 0.5;
    }

    fn draw(&self, img: &mut RgbaImage) {
        let alpha = self.opacity;
        if alpha <= 0.01 {
            return;
        }
        let r = self.r;
        let x_min = (self.x - r - 1.0).max(0.0) as u32;
        let x_max = (self.x + r + 1.0).min(img.width() as f32 - 1.0) as u32;
        let y_min = (self.y - r - 1.0).max(0.0) as u32;
        let y_max = (self.y + r + 1.0).min(img.height() as f32 - 1.0) as u32;

        let r_sq = r * r;
        for py in y_min..=y_max {
            let dy = py as f32 - self.y;
            for px in x_min..=x_max {
                let dx = px as f32 - self.x;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= r_sq {
                    let dist = dist_sq.sqrt();
                    let edge = r - dist;
                    let aa = if edge >= 0.5 {
                        1.0
                    } else if edge > -0.5 {
                        edge + 0.5
                    } else {
                        continue;
                    };
                    blend_pixel(img, px, py, Rgba([255, 255, 255, 255]), alpha * aa);
                }
            }
        }
    }
}

/// Uniform random in [a, b]
fn rand_range(rng: &mut impl RngExt, a: f32, b: f32) -> f32 {
    rng.random::<f32>() * (b - a) + a
}

/// Blend a pixel with alpha onto the image
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

/// Interpolate two RGB colors by factor t (0..1, 0=color1, 1=color2)
fn lerp_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (c1.0 as f32 * (1.0 - t) + c2.0 as f32 * t).round() as u8,
        (c1.1 as f32 * (1.0 - t) + c2.1 as f32 * t).round() as u8,
        (c1.2 as f32 * (1.0 - t) + c2.2 as f32 * t).round() as u8,
    )
}

/// Fill a triangle with a solid color using scanline rasterization.
/// Edge anti-aliasing is handled separately by the stroke lines.
fn fill_triangle(img: &mut RgbaImage, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), color: (u8, u8, u8)) {
    let w = img.width();
    let h = img.height();

    // Sort vertices by Y (flat-bottom / flat-top split)
    let mut verts = [p0, p1, p2];
    verts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let [v0, v1, v2] = verts;

    // Edge function: signed area of triangle (a,b,p)
    let edge = |ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32| -> f32 {
        (bx - ax) * (py - ay) - (by - ay) * (px - ax)
    };

    // 2× signed area — skip degenerate triangles
    let area = edge(v0.0, v0.1, v2.0, v2.1, v1.0, v1.1);
    if area.abs() < 1e-6 {
        return;
    }

    // Bounding box (clamped to image)
    let x_min = verts.iter().map(|v| v.0).fold(f32::MAX, f32::min).floor().max(0.0) as u32;
    let x_max = verts.iter().map(|v| v.0).fold(f32::MIN, f32::max).ceil().min(w as f32 - 1.0) as u32;
    let y_min = v0.1.floor().max(0.0) as u32;
    let y_max = v2.1.ceil().min(h as f32 - 1.0) as u32;

    let rgba = Rgba([color.0, color.1, color.2, 255]);

    for py in y_min..=y_max {
        let py_c = py as f32 + 0.5; // pixel center
        for px in x_min..=x_max {
            let px_c = px as f32 + 0.5;

            // Barycentric weights (unnormalised)
            let w0 = edge(v1.0, v1.1, v2.0, v2.1, px_c, py_c);
            let w1 = edge(v2.0, v2.1, v0.0, v0.1, px_c, py_c);
            let w2 = edge(v0.0, v0.1, v1.0, v1.1, px_c, py_c);

            // Inside if all same sign
            let inside = (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0)
                || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);

            if inside {
                img.put_pixel(px, py, rgba);
            }
        }
    }
}

impl TriangleConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate the base point grid and Delaunay triangulation
    fn generate_mesh(&mut self) {
        let w = self.width as f64;
        let h = self.height as f64;
        let bleed = self.s_bleed as f64;
        let size = self.s_triangle_size as f64;
        let noise = self.s_noise as f64;

        let mut rng = rand::rng();
        let mut points = Vec::new();

        // Four corners to ensure full coverage
        points.push([0.0, 0.0]);
        points.push([0.0, h]);
        points.push([w, 0.0]);
        points.push([w, h]);

        // Grid of noisy points
        let mut x = -bleed;
        while x < w + bleed {
            let mut y = -bleed;
            while y < h + bleed {
                let nx = x + rng.random::<f64>() * noise;
                let ny = y + rng.random::<f64>() * noise;
                points.push([nx, ny]);
                y += size;
            }
            x += size;
        }

        // Compute Delaunay triangulation
        let delaunator_points: Vec<delaunator::Point> = points.iter().map(|&p| p.into()).collect();
        let triangulation = delaunator::triangulate(&delaunator_points);

        // Convert flat triangle index list to [[i0, i1, i2], ...]
        let triangles = triangulation
            .triangles
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect();

        // Build per-point animation phases: seeded by X position
        // JS: `baseCoordinateTable[x][y] = per * 2 * Math.PI`
        // where `per = x / width`
        let anim_phases: Vec<f64> = points
            .iter()
            .map(|p| (p[0] / w) * 2.0 * std::f64::consts::PI)
            .collect();

        self.points = points;
        self.triangles = triangles;
        self.anim_phases = anim_phases;
    }

    /// Create floating particles
    fn create_particles(&mut self) {
        self.particles.clear();
        let w = self.width as f32;
        let h = self.height as f32;
        for _ in 0..self.particle_count {
            self.particles.push(Particle::new(w, h));
        }
    }

    /// Compute the animated position of a point given its index
    fn animated_point(&self, idx: usize) -> (f32, f32) {
        let base = &self.points[idx];
        let phase = self.anim_phases[idx];
        let dx = phase.cos() as f32 * self.s_point_variation_x;
        let dy = phase.sin() as f32 * self.s_point_variation_y;
        (base[0] as f32 + dx, base[1] as f32 + dy)
    }

    /// Compute centroid of a triangle from three points
    fn centroid(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) -> (f32, f32) {
        ((p0.0 + p1.0 + p2.0) / 3.0, (p0.1 + p1.1 + p2.1) / 3.0)
    }

    /// Compute gradient factor for a position (0..1 diagonal blend)
    fn gradient_factor(&self, cx: f32, cy: f32) -> f32 {
        let w = self.width as f32;
        let h = self.height as f32;
        let per_x = cx / w;
        let per_y = cy / h;
        ((per_x + per_y) / 2.0).clamp(0.0, 1.0)
    }

    fn generate_frame(&mut self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        let stroke_color = Rgba([
            self.stroke_color.0,
            self.stroke_color.1,
            self.stroke_color.2,
            self.stroke_color.3,
        ]);

        // Draw all triangles: fill + stroke
        for &[i0, i1, i2] in &self.triangles {
            let p0 = self.animated_point(i0);
            let p1 = self.animated_point(i1);
            let p2 = self.animated_point(i2);

            let (cx, cy) = Self::centroid(p0, p1, p2);
            let t = self.gradient_factor(cx, cy);
            let color = lerp_color(self.color1, self.color2, t);

            // Fill triangle with gradient color
            fill_triangle(&mut img, p0, p1, p2, color);

            // Stroke triangle edges with anti-aliased lines
            draw_line_segment_aa(&mut img, p0, p1, stroke_color, self.s_stroke_width);
            draw_line_segment_aa(&mut img, p1, p2, stroke_color, self.s_stroke_width);
            draw_line_segment_aa(&mut img, p2, p0, stroke_color, self.s_stroke_width);
        }

        // Update and draw particles
        let dt_ms = 1000.0 / self.fps as f32;
        for p in &mut self.particles {
            p.update(dt_ms);
        }
        for p in &self.particles {
            p.draw(&mut img);
        }

        // Advance animation phases
        // JS: `baseCoordinateTable[x][y] += pointAnimationSpeed / 1000`
        let increment = self.s_point_animation_speed as f64 / 1000.0;
        for phase in &mut self.anim_phases {
            *phase += increment;
        }

        img
    }
}

impl Iterator for TriangleConfig {
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

impl AnimationInit for TriangleConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Scale pixel-based parameters for target resolution
        self.s_triangle_size = scale_pixel_for_height(self.triangle_size, height);
        self.s_bleed = scale_pixel_for_height(self.bleed, height);
        self.s_noise = scale_pixel_for_height(self.noise, height);
        self.s_point_variation_x = scale_pixel_for_height(self.point_variation_x, height);
        self.s_point_variation_y = scale_pixel_for_height(self.point_variation_y, height);
        self.s_stroke_width = scale_pixel_for_height(self.stroke_width, height);

        // Speed scales with resolution
        let speed_scale = height as f32 / 1080.0;
        self.s_point_animation_speed = self.point_animation_speed * speed_scale;

        self.generate_mesh();
        self.create_particles();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.points.clear();
        self.triangles.clear();
        self.anim_phases.clear();
        self.particles.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(TriangleConfig);
