//! Cross-line animation: moving lines that intersect and mark their crossing points.
//!
//! This animation creates horizontal and vertical lines that move across the canvas.
//! When lines intersect, their crossing points are marked with small circles.

use crate::{AnimationInit, scale_pixel_for_height};
use crate::aa_line::draw_line_segment_aa;
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// Line orientation
#[derive(Debug, Clone, Copy, PartialEq)]
enum LineOrientation {
    /// Horizontal line (moves vertically)
    Horizontal,
    /// Vertical line (moves horizontally)
    Vertical,
}

/// A single moving line
#[derive(Debug, Clone)]
struct Line {
    /// Line orientation
    orientation: LineOrientation,
    /// Point A coordinates
    a: (f32, f32),
    /// Point B coordinates
    b: (f32, f32),
    /// Velocity for point A
    va: f32,
    /// Velocity for point B
    vb: f32,
}

impl Line {
    /// Create a new line with random position and velocity
    fn new(width: f32, height: f32, orientation: LineOrientation, speed_min: f32, speed_max: f32) -> Self {
        let mut rng = rand::rng();

        let (a, b) = match orientation {
            LineOrientation::Vertical => {
                // Vertical line: spans from top (y=0) to bottom (y=height)
                let x1 = rand_range(&mut rng, 0.0, width);
                let x2 = rand_range(&mut rng, 0.0, width);
                ((x1, 0.0), (x2, height))
            }
            LineOrientation::Horizontal => {
                // Horizontal line: spans from left (x=0) to right (x=width)
                let y1 = rand_range(&mut rng, 0.0, height);
                let y2 = rand_range(&mut rng, 0.0, height);
                ((0.0, y1), (width, y2))
            }
        };

        // Random velocity for each endpoint (0.25 to 1.0 in JS)
        let va = rand_range(&mut rng, speed_min, speed_max);
        let vb = rand_range(&mut rng, speed_min, speed_max);

        Self {
            orientation,
            a,
            b,
            va,
            vb,
        }
    }

    /// Update line position and handle edge bouncing
    fn update(&mut self, width: f32, height: f32) {
        match self.orientation {
            LineOrientation::Vertical => {
                // Move x coordinates
                self.a.0 += self.va;
                self.b.0 += self.vb;

                // Bounce off left/right edges
                if self.a.0 < 0.0 || self.a.0 > width {
                    self.va *= -1.0;
                }
                if self.b.0 < 0.0 || self.b.0 > width {
                    self.vb *= -1.0;
                }
            }
            LineOrientation::Horizontal => {
                // Move y coordinates
                self.a.1 += self.va;
                self.b.1 += self.vb;

                // Bounce off top/bottom edges
                if self.a.1 < 0.0 || self.a.1 > height {
                    self.va *= -1.0;
                }
                if self.b.1 < 0.0 || self.b.1 > height {
                    self.vb *= -1.0;
                }
            }
        }
    }

    /// Draw the line
    fn draw(&self, img: &mut RgbaImage, color: Rgba<u8>, line_width: f32) {
        draw_line_segment_aa(img, self.a, self.b, color, line_width);
    }
}

/// Calculate intersection point of two line segments
/// Returns None if lines are parallel or intersection is outside segments
fn intersect_lines(l1: &Line, l2: &Line) -> Option<(f32, f32)> {
    let p1 = l1.a;
    let p2 = l1.b;
    let p3 = l2.a;
    let p4 = l2.b;

    // Line segment intersection using parametric form
    let denominator = (p4.1 - p3.1) * (p2.0 - p1.0) - (p4.0 - p3.0) * (p2.1 - p1.1);

    // Lines are parallel
    if denominator.abs() < 1e-10 {
        return None;
    }

    let ua = ((p4.0 - p3.0) * (p1.1 - p3.1) - (p4.1 - p3.1) * (p1.0 - p3.0)) / denominator;
    let ub = ((p2.0 - p1.0) * (p1.1 - p3.1) - (p2.1 - p1.1) * (p1.0 - p3.0)) / denominator;

    // Check if intersection is within both segments (ua > 0 and ub > 0 in JS)
    // We use a small epsilon to include endpoints
    if ua >= 0.0 && ua <= 1.0 && ub >= 0.0 && ub <= 1.0 {
        let x = p1.0 + ua * (p2.0 - p1.0);
        let y = p1.1 + ua * (p2.1 - p1.1);
        Some((x, y))
    } else {
        None
    }
}

/// Draw a small filled circle at the intersection point
fn mark_point(img: &mut RgbaImage, p: (f32, f32), color: Rgba<u8>, radius: f32) {
    let (x, y) = p;
    let r_sq = radius * radius;

    let x_min = (x - radius - 0.5).max(0.0) as u32;
    let x_max = (x + radius + 0.5).min(img.width() as f32 - 1.0) as u32;
    let y_min = (y - radius - 0.5).max(0.0) as u32;
    let y_max = (y + radius + 0.5).min(img.height() as f32 - 1.0) as u32;

    for py in y_min..=y_max {
        let dy = py as f32 - y;
        for px in x_min..=x_max {
            let dx = px as f32 - x;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq <= r_sq {
                // Anti-alias at edge
                let dist = dist_sq.sqrt();
                let edge_dist = radius - dist;
                let alpha = if edge_dist >= 0.5 {
                    1.0
                } else if edge_dist > -0.5 {
                    edge_dist + 0.5
                } else {
                    continue;
                };

                if alpha > 0.0 {
                    blend_pixel(img, px, py, color, alpha);
                }
            }
        }
    }
}

/// Blend a pixel with alpha
fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>, alpha: f32) {
    let alpha = alpha.min(1.0);
    if alpha <= 0.0 || x >= img.width() || y >= img.height() {
        return;
    }

    let src_alpha = alpha * (color[3] as f32 / 255.0);
    let pixel = img.get_pixel_mut(x, y);
    let dst_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha > 0.0 {
        pixel[0] = ((color[0] as f32 * src_alpha + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[1] = ((color[1] as f32 * src_alpha + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[2] = ((color[2] as f32 * src_alpha + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[3] = (out_alpha * 255.0) as u8;
    }
}

/// Uniform random in [a, b]
fn rand_range(rng: &mut impl RngExt, a: f32, b: f32) -> f32 {
    rng.random::<f32>() * (b - a) + a
}

/// Cross-line animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct CrossLineConfig {
    /// Number of lines (default: 16)
    #[derivative(Default(value = "16"))]
    pub lines_num: usize,

    /// Minimum velocity (default: 0.25)
    #[derivative(Default(value = "0.25"))]
    pub speed_min: f32,

    /// Maximum velocity (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub speed_max: f32,

    /// Line color (RGBA) (default: light gray)
    #[derivative(Default(value = "(200, 200, 200, 255)"))]
    pub line_color: (u8, u8, u8, u8),

    /// Line width at 1080p (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub line_width: f32,

    /// Intersection point color (RGBA) (default: white)
    #[derivative(Default(value = "(255, 255, 255, 255)"))]
    pub point_color: (u8, u8, u8, u8),

    /// Intersection point radius at 1080p (default: 2.0)
    #[derivative(Default(value = "2.0"))]
    pub point_radius: f32,

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

    #[setters(skip)]
    #[serde(skip)]
    lines: Vec<Line>,

    /// Scaled line width (computed in init)
    #[setters(skip)]
    #[serde(skip)]
    s_line_width: f32,

    /// Scaled point radius (computed in init)
    #[setters(skip)]
    #[serde(skip)]
    s_point_radius: f32,

    /// Scaled speed min (computed in init)
    #[setters(skip)]
    #[serde(skip)]
    s_speed_min: f32,

    /// Scaled speed max (computed in init)
    #[setters(skip)]
    #[serde(skip)]
    s_speed_max: f32,
}

impl CrossLineConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_lines(&mut self) {
        self.lines.clear();

        for i in 0..self.lines_num {
            // Alternate between horizontal and vertical (even = horizontal, odd = vertical)
            let orientation = if i % 2 == 0 {
                LineOrientation::Horizontal
            } else {
                LineOrientation::Vertical
            };

            self.lines.push(Line::new(
                self.width as f32,
                self.height as f32,
                orientation,
                self.s_speed_min,
                self.s_speed_max,
            ));
        }
    }

    fn update_lines(&mut self) {
        let w = self.width as f32;
        let h = self.height as f32;

        for line in &mut self.lines {
            line.update(w, h);
        }
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        let line_color = Rgba([
            self.line_color.0,
            self.line_color.1,
            self.line_color.2,
            self.line_color.3,
        ]);

        let point_color = Rgba([
            self.point_color.0,
            self.point_color.1,
            self.point_color.2,
            self.point_color.3,
        ]);

        // Draw all lines
        for line in &self.lines {
            line.draw(&mut img, line_color, self.s_line_width);
        }

        // Find and mark all intersections
        for i in 0..self.lines.len() {
            for j in (i + 1)..self.lines.len() {
                if let Some(p) = intersect_lines(&self.lines[i], &self.lines[j]) {
                    mark_point(&mut img, p, point_color, self.s_point_radius);
                }
            }
        }

        img
    }
}

impl Iterator for CrossLineConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        self.update_lines();
        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for CrossLineConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Scale pixel-based parameters for target resolution
        self.s_line_width = scale_pixel_for_height(self.line_width, height);
        self.s_point_radius = scale_pixel_for_height(self.point_radius, height);

        // Speed scales with resolution (lines should move proportionally)
        let speed_scale = height as f32 / 1080.0;
        self.s_speed_min = self.speed_min * speed_scale;
        self.s_speed_max = self.speed_max * speed_scale;

        self.create_lines();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.lines.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(CrossLineConfig);
