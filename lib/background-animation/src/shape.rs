use crate::{AnimationInit, scale_pixel_for_height};
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// Color palette for shapes (RGB)
const COLORS: [(u8, u8, u8); 6] = [
    (52, 168, 83),   // Green
    (117, 95, 147),  // Purple
    (199, 108, 23),  // Orange
    (194, 62, 55),   // Red
    (0, 172, 212),   // Cyan
    (120, 120, 120), // Gray
];

/// Fill style for a circle
#[derive(Debug, Clone, Copy, PartialEq)]
enum FillStyle {
    /// No fill, stroke only
    None,
    /// Fully filled circle
    Full,
    /// Concentric circle (inner ring at half radius)
    Concentric,
}

/// A single shape (circle) in the animation
#[derive(Debug, Clone)]
struct Shape {
    /// X position (centered at 0,0, relative to canvas center)
    x: f32,
    /// Y position (centered at 0,0, relative to canvas center)
    y: f32,
    /// Radius in pixels
    radius: f32,
    /// Fill style
    filled: FillStyle,
    /// Color index into COLORS
    color_idx: usize,
    /// Border color index
    border_color_idx: usize,
    /// Current opacity (fades in)
    opacity: f32,
    /// X velocity
    speedx: f32,
    /// Y velocity
    speedy: f32,
    /// Time to live (frames until respawn)
    ttl: i32,
    /// Whether this is a background shape
    background: bool,
}

impl Shape {
    fn new(
        width: f32,
        height: f32,
        background: bool,
        rad_min: f32,
        rad_max: f32,
        speed_min: f32,
        speed_max: f32,
        background_mult: f32,
        rad_threshold: f32,
        filled_circle_pct: u32,
        concentric_circle_pct: u32,
    ) -> Self {
        let mut rng = rand::rng();

        let x = rand_range(&mut rng, -width / 2.0, width / 2.0);
        let y = rand_range(&mut rng, -height / 2.0, height / 2.0);

        let radius = if background {
            hyper_range(&mut rng, rad_min, rad_max) * background_mult
        } else {
            hyper_range(&mut rng, rad_min, rad_max)
        };

        let filled = if radius < rad_threshold {
            if rng.random_range(0..100) > filled_circle_pct {
                FillStyle::None
            } else {
                FillStyle::Full
            }
        } else if rng.random_range(0..100) > concentric_circle_pct {
            FillStyle::None
        } else {
            FillStyle::Concentric
        };

        let color_idx = rng.random_range(0..COLORS.len());
        let border_color_idx = rng.random_range(0..COLORS.len());

        let speed = if background {
            rand_range(&mut rng, speed_min, speed_max) / background_mult
        } else {
            rand_range(&mut rng, speed_min, speed_max)
        };

        let speed_angle = rng.random::<f32>() * std::f32::consts::TAU;
        let speedx = speed_angle.cos() * speed;
        let speedy = speed_angle.sin() * speed;

        // Calculate TTL: how many frames until shape exits canvas
        let spacex =
            ((x - if speedx < 0.0 { -1.0 } else { 1.0 } * (width / 2.0 + radius)) / speedx).abs();
        let spacey =
            ((y - if speedy < 0.0 { -1.0 } else { 1.0 } * (height / 2.0 + radius)) / speedy).abs();
        let ttl = spacex.min(spacey) as i32;

        Self {
            x,
            y,
            radius,
            filled,
            color_idx,
            border_color_idx,
            opacity: 0.05,
            speedx,
            speedy,
            ttl,
            background,
        }
    }

    fn reinit(
        &mut self,
        width: f32,
        height: f32,
        rad_min: f32,
        rad_max: f32,
        speed_min: f32,
        speed_max: f32,
        background_mult: f32,
        rad_threshold: f32,
        filled_circle_pct: u32,
        concentric_circle_pct: u32,
    ) {
        *self = Self::new(
            width,
            height,
            self.background,
            rad_min,
            rad_max,
            speed_min,
            speed_max,
            background_mult,
            rad_threshold,
            filled_circle_pct,
            concentric_circle_pct,
        );
    }

    fn update(&mut self, max_opacity: f32) {
        // Move
        self.x += self.speedx;
        self.y += self.speedy;

        // Fade in
        let target_opacity = if self.background { max_opacity } else { 1.0 };
        if self.opacity < target_opacity {
            self.opacity += 0.01;
        }

        self.ttl -= 1;
    }
}

/// Uniform random in [a, b]
fn rand_range(rng: &mut impl RngExt, a: f32, b: f32) -> f32 {
    rng.random::<f32>() * (b - a) + a
}

/// Hyper-weighted random toward low values (cubic bias)
/// Matches JS: Math.random() * Math.random() * Math.random() * (b - a) + a
fn hyper_range(rng: &mut impl RngExt, a: f32, b: f32) -> f32 {
    let r = rng.random::<f32>();
    let r2 = rng.random::<f32>();
    let r3 = rng.random::<f32>();
    r * r2 * r3 * (b - a) + a
}

/// Shape animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ShapeConfig {
    /// Maximum number of foreground circles (default: 12)
    #[derivative(Default(value = "12"))]
    pub max_circles: usize,

    /// Minimum radius in pixels at 1080p (default: 5)
    #[derivative(Default(value = "5.0"))]
    pub rad_min: f32,

    /// Maximum radius in pixels at 1080p (default: 125)
    #[derivative(Default(value = "125.0"))]
    pub rad_max: f32,

    /// Percentage threshold for filled circles (default: 60)
    /// Small circles with random > filled_circle_pct are filled
    #[derivative(Default(value = "60"))]
    pub filled_circle_pct: u32,

    /// Percentage threshold for concentric circles (default: 30)
    /// Large circles with random > concentric_circle_pct are concentric
    #[derivative(Default(value = "30"))]
    pub concentric_circle_pct: u32,

    /// Radius threshold separating small/large circles at 1080p (default: 25)
    #[derivative(Default(value = "25.0"))]
    pub rad_threshold: f32,

    /// Minimum movement speed at 1080p (default: 0.3)
    #[derivative(Default(value = "0.3"))]
    pub speed_min: f32,

    /// Maximum movement speed at 1080p (default: 2.5)
    #[derivative(Default(value = "2.5"))]
    pub speed_max: f32,

    /// Maximum opacity for background shapes (default: 0.6)
    #[derivative(Default(value = "0.6"))]
    pub max_opacity: f32,

    /// Border width for circle outlines at 1080p (default: 10)
    #[derivative(Default(value = "10.0"))]
    pub circle_border: f32,

    /// Background shape size multiplier (default: 0.85)
    #[derivative(Default(value = "0.85"))]
    pub background_mult: f32,

    /// Line width for connecting lines at 1080p (default: 2.5)
    #[derivative(Default(value = "2.5"))]
    pub line_border: f32,

    /// Link distance as fraction of min(width,height) (default: 1/2.4)
    /// In JS: Math.min(canvas.width, canvas.height) / 2.4
    #[derivative(Default(value = "1.0 / 2.4"))]
    pub link_dist_fraction: f32,

    /// Color palette (RGB tuples) (default: 6-color palette)
    #[derivative(Default(value = "vec![
        (52, 168, 83),
        (117, 95, 147),
        (199, 108, 23),
        (194, 62, 55),
        (0, 172, 212),
        (120, 120, 120),
    ]"))]
    pub colors: Vec<(u8, u8, u8)>,

    /// Background color (RGB) (default: dark gray)
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
    foreground_shapes: Vec<Shape>,

    #[setters(skip)]
    #[serde(skip)]
    background_shapes: Vec<Shape>,

    /// Scaled parameters (computed in init)
    #[setters(skip)]
    #[serde(skip)]
    s_rad_min: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_rad_max: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_rad_threshold: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_speed_min: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_speed_max: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_circle_border: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_line_border: f32,

    #[setters(skip)]
    #[serde(skip)]
    s_link_dist: f32,
}

impl ShapeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_shape(&self, background: bool) -> Shape {
        Shape::new(
            self.width as f32,
            self.height as f32,
            background,
            self.s_rad_min,
            self.s_rad_max,
            self.s_speed_min,
            self.s_speed_max,
            self.background_mult,
            self.s_rad_threshold,
            self.filled_circle_pct,
            self.concentric_circle_pct,
        )
    }

    fn update_shapes(&mut self) {
        // Update foreground shapes
        for shape in &mut self.foreground_shapes {
            shape.update(self.max_opacity);
            if shape.ttl < -20 {
                shape.reinit(
                    self.width as f32,
                    self.height as f32,
                    self.s_rad_min,
                    self.s_rad_max,
                    self.s_speed_min,
                    self.s_speed_max,
                    self.background_mult,
                    self.s_rad_threshold,
                    self.filled_circle_pct,
                    self.concentric_circle_pct,
                );
            }
        }

        // Update background shapes
        for shape in &mut self.background_shapes {
            shape.update(self.max_opacity);
            if shape.ttl < -20 {
                shape.reinit(
                    self.width as f32,
                    self.height as f32,
                    self.s_rad_min,
                    self.s_rad_max,
                    self.s_speed_min,
                    self.s_speed_max,
                    self.background_mult,
                    self.s_rad_threshold,
                    self.filled_circle_pct,
                    self.concentric_circle_pct,
                );
            }
        }
    }

    fn draw_circle(
        img: &mut RgbaImage,
        cx: f32,
        cy: f32,
        radius: f32,
        shape: &Shape,
        colors: &[(u8, u8, u8)],
        circle_border: f32,
        rad_min: f32,
        rad_max: f32,
    ) {
        if radius <= 0.0 {
            return;
        }

        let color = if colors.is_empty() {
            COLORS[shape.color_idx % COLORS.len()]
        } else {
            colors[shape.color_idx % colors.len()]
        };
        let border_color = if colors.is_empty() {
            COLORS[shape.border_color_idx % COLORS.len()]
        } else {
            colors[shape.border_color_idx % colors.len()]
        };

        let line_width = (circle_border * (rad_min - radius) / (rad_min - rad_max)).max(1.0);

        // Draw filled circle
        if shape.filled == FillStyle::Full {
            let fill_opacity = if shape.background {
                shape.opacity * 0.8
            } else {
                shape.opacity
            };
            Self::draw_filled_circle(img, cx, cy, radius, border_color, fill_opacity);
        }

        // Draw stroke
        Self::draw_circle_stroke(img, cx, cy, radius, line_width, border_color, shape.opacity);

        // Draw concentric inner ring
        if shape.filled == FillStyle::Concentric {
            let inner_r = radius / 2.0;
            if inner_r > 0.0 {
                let inner_line_width =
                    (circle_border * (rad_min - radius) / (rad_min - rad_max)).max(1.0);
                Self::draw_circle_stroke(
                    img,
                    cx,
                    cy,
                    inner_r,
                    inner_line_width,
                    color,
                    shape.opacity,
                );
            }
        }
    }

    /// Draw a filled circle with alpha blending (subpixel-accurate)
    fn draw_filled_circle(
        img: &mut RgbaImage,
        cx: f32,
        cy: f32,
        radius: f32,
        color: (u8, u8, u8),
        opacity: f32,
    ) {
        let w = img.width() as f32;
        let h = img.height() as f32;

        let x_min = (cx - radius - 0.5).floor().max(0.0) as u32;
        let x_max = (cx + radius + 0.5).ceil().min(w - 1.0) as u32;
        let y_min = (cy - radius - 0.5).floor().max(0.0) as u32;
        let y_max = (cy + radius + 0.5).ceil().min(h - 1.0) as u32;

        let r_sq = radius * radius;

        for py in y_min..=y_max {
            let dy = py as f32 - cy;
            for px in x_min..=x_max {
                let dx = px as f32 - cx;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= r_sq {
                    // Anti-alias at the edge: smooth transition over ~1px band
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
                        Self::blend_pixel(img, px, py, color, opacity * alpha);
                    }
                }
            }
        }
    }

    /// Draw a circle stroke (outline) with subpixel-accurate anti-aliasing
    fn draw_circle_stroke(
        img: &mut RgbaImage,
        cx: f32,
        cy: f32,
        radius: f32,
        line_width: f32,
        color: (u8, u8, u8),
        opacity: f32,
    ) {
        let w = img.width() as f32;
        let h = img.height() as f32;

        let outer_r = radius + line_width / 2.0 + 0.5;
        let inner_r = (radius - line_width / 2.0 - 0.5).max(0.0);

        let x_min = (cx - outer_r).floor().max(0.0) as u32;
        let x_max = (cx + outer_r).ceil().min(w - 1.0) as u32;
        let y_min = (cy - outer_r).floor().max(0.0) as u32;
        let y_max = (cy + outer_r).ceil().min(h - 1.0) as u32;

        let half_width = line_width / 2.0;

        for py in y_min..=y_max {
            let dy = py as f32 - cy;
            for px in x_min..=x_max {
                let dx = px as f32 - cx;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist >= inner_r && dist <= outer_r {
                    let center_dist = (dist - radius).abs();
                    let alpha = if center_dist < half_width - 0.5 {
                        1.0
                    } else if center_dist < half_width + 0.5 {
                        half_width + 0.5 - center_dist
                    } else {
                        0.0
                    };

                    if alpha > 0.0 {
                        Self::blend_pixel(img, px, py, color, opacity * alpha);
                    }
                }
            }
        }
    }

    /// Draw a line between two shapes with distance-based opacity
    fn draw_link_line(
        img: &mut RgbaImage,
        s1: &Shape,
        s2: &Shape,
        link_dist: f32,
        line_border: f32,
        colors: &[(u8, u8, u8)],
    ) {
        let dx = s1.x - s2.x;
        let dy = s1.y - s2.y;
        let dist = (dx * dx + dy * dy).sqrt();

        // Skip if circles overlap or too far
        if dist <= s1.radius + s2.radius || dist >= link_dist {
            return;
        }

        // Calculate line endpoints at circle edges
        let xi = if s1.x < s2.x { 1.0 } else { -1.0 } * (s1.radius * dx / dist).abs();
        let yi = if s1.y < s2.y { 1.0 } else { -1.0 } * (s1.radius * dy / dist).abs();
        let xj = if s1.x < s2.x { -1.0 } else { 1.0 } * (s2.radius * dx / dist).abs();
        let yj = if s1.y < s2.y { -1.0 } else { 1.0 } * (s2.radius * dy / dist).abs();

        // Line start/end points (relative to canvas center, need to offset)
        let x0 = s1.x + xi;
        let y0 = s1.y + yi;
        let x1 = s2.x + xj;
        let y1 = s2.y + yj;

        // Opacity based on distance
        let dist_factor = (link_dist - dist) / link_dist;
        let opacity = s1.opacity.min(s2.opacity) * dist_factor;

        let color = if colors.is_empty() {
            COLORS[s1.border_color_idx % COLORS.len()]
        } else {
            colors[s1.border_color_idx % colors.len()]
        };

        let width = if s1.background || s2.background {
            line_border * 0.85
        } else {
            line_border
        } * dist_factor;

        Self::draw_line_aa(img, x0, y0, x1, y1, color, opacity, width);
    }

    /// Anti-aliased line drawing using Xiaolin Wu's algorithm
    fn draw_line_aa(
        img: &mut RgbaImage,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: (u8, u8, u8),
        opacity: f32,
        width: f32,
    ) {
        let hw = img.width() as f32 / 2.0;
        let hh = img.height() as f32 / 2.0;

        // Convert from center-relative to absolute coordinates
        let mut ax = x0 + hw;
        let mut ay = y0 + hh;
        let mut bx = x1 + hw;
        let mut by = y1 + hh;

        let steep = (by - ay).abs() > (bx - ax).abs();

        if steep {
            std::mem::swap(&mut ax, &mut ay);
            std::mem::swap(&mut bx, &mut by);
        }

        if ax > bx {
            std::mem::swap(&mut ax, &mut bx);
            std::mem::swap(&mut ay, &mut by);
        }

        let dx = bx - ax;
        let dy = by - ay;
        let gradient = if dx.abs() < 1e-6 { 1.0 } else { dy / dx };

        // Handle first endpoint
        let xend = ax.round();
        let yend = ay + gradient * (xend - ax);
        let xpxl1 = xend as i32;
        let mut intery = yend + gradient;

        // Handle second endpoint
        let xpxl2 = bx.round() as i32;

        let alpha_factor = width.min(1.0);

        // Main loop
        for x in xpxl1..=xpxl2 {
            let fy = intery;
            let ipart = fy.floor() as i32;
            let fpart = fy - ipart as f32;

            if steep {
                Self::blend_pixel(
                    img,
                    x as u32,
                    ipart as u32,
                    color,
                    opacity * (1.0 - fpart) * alpha_factor,
                );
                Self::blend_pixel(
                    img,
                    x as u32,
                    (ipart + 1) as u32,
                    color,
                    opacity * fpart * alpha_factor,
                );
            } else {
                Self::blend_pixel(
                    img,
                    ipart as u32,
                    x as u32,
                    color,
                    opacity * (1.0 - fpart) * alpha_factor,
                );
                Self::blend_pixel(
                    img,
                    (ipart + 1) as u32,
                    x as u32,
                    color,
                    opacity * fpart * alpha_factor,
                );
            }

            intery += gradient;
        }
    }

    /// Blend a pixel with alpha
    fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: (u8, u8, u8), alpha: f32) {
        let alpha = alpha.min(1.0);
        if alpha <= 0.0 {
            return;
        }

        if x >= img.width() || y >= img.height() {
            return;
        }

        let existing = img.get_pixel(x, y);
        let bg_alpha = 1.0 - alpha;

        let new_r = ((existing.0[0] as f32 * bg_alpha + color.0 as f32 * alpha).min(255.0)) as u8;
        let new_g = ((existing.0[1] as f32 * bg_alpha + color.1 as f32 * alpha).min(255.0)) as u8;
        let new_b = ((existing.0[2] as f32 * bg_alpha + color.2 as f32 * alpha).min(255.0)) as u8;

        img.put_pixel(x, y, Rgba([new_r, new_g, new_b, 255]));
    }

    fn render_shapes(
        img: &mut RgbaImage,
        shapes: &[Shape],
        link_dist: f32,
        line_border: f32,
        colors: &[(u8, u8, u8)],
        circle_border: f32,
        rad_min: f32,
        rad_max: f32,
    ) {
        let hw = (img.width() as f32) / 2.0;
        let hh = (img.height() as f32) / 2.0;

        // Draw circles
        for shape in shapes {
            let cx = shape.x + hw;
            let cy = shape.y + hh;

            Self::draw_circle(
                img,
                cx,
                cy,
                shape.radius,
                shape,
                colors,
                circle_border,
                rad_min,
                rad_max,
            );
        }

        // Draw connecting lines between shapes
        for i in 0..shapes.len() {
            for j in (i + 1)..shapes.len() {
                Self::draw_link_line(img, &shapes[i], &shapes[j], link_dist, line_border, colors);
            }
        }
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Render background shapes first (they appear behind)
        Self::render_shapes(
            &mut img,
            &self.background_shapes,
            self.s_link_dist,
            self.s_line_border,
            &self.colors,
            self.s_circle_border,
            self.s_rad_min,
            self.s_rad_max,
        );

        // Render foreground shapes on top
        Self::render_shapes(
            &mut img,
            &self.foreground_shapes,
            self.s_link_dist,
            self.s_line_border,
            &self.colors,
            self.s_circle_border,
            self.s_rad_min,
            self.s_rad_max,
        );

        img
    }
}

impl Iterator for ShapeConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        self.update_shapes();
        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for ShapeConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Scale pixel-based parameters for target resolution
        self.s_rad_min = scale_pixel_for_height(self.rad_min, height);
        self.s_rad_max = scale_pixel_for_height(self.rad_max, height);
        self.s_rad_threshold = scale_pixel_for_height(self.rad_threshold, height);
        self.s_speed_min = scale_pixel_for_height(self.speed_min, height);
        self.s_speed_max = scale_pixel_for_height(self.speed_max, height);
        self.s_circle_border = scale_pixel_for_height(self.circle_border, height);
        self.s_line_border = scale_pixel_for_height(self.line_border, height);
        self.s_link_dist = self.link_dist_fraction * width.min(height) as f32;

        // Create foreground shapes (maxCircles * 2 in JS)
        self.foreground_shapes.clear();
        for _ in 0..(self.max_circles * 2) {
            self.foreground_shapes.push(self.create_shape(false));
        }

        // Create background shapes (maxCircles in JS)
        self.background_shapes.clear();
        for _ in 0..self.max_circles {
            self.background_shapes.push(self.create_shape(true));
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.foreground_shapes.clear();
        self.background_shapes.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(ShapeConfig);
