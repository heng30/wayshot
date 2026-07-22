//! Black hole background animation.
//!
//! Creates a mesmerizing black hole effect where particles (stars) are drawn toward
//! a central gravity point. Based on tmp/black-hole.js:
//! - Stars spawn randomly across the canvas and accelerate toward the gravity center
//! - Stars grow and fade as they reach the event horizon
//! - A dark black hole core is rendered with a radial gradient
//! - The hue shifts over time, creating color evolution
//! - Mouse/touch interaction moves the gravity point (configurable center)
//!
//! JS reference uses canvas 2D with semi-transparent fill for motion trails.
//! We replicate this by blending each frame over a dimmed previous frame.

use crate::{AnimationInit, scale_pixel_for_height};
use image::{Rgba, RgbaImage};
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// A single star particle
#[derive(Debug, Clone)]
struct Star {
    /// Hue angle (0-360)
    hue: f32,
    /// Current alpha (0-1)
    alpha: f32,
    /// Circle radius
    size: f32,
    /// Position
    x: f32,
    y: f32,
    /// Speed toward gravity center
    speed: f32,
    /// Whether the star has reached the event horizon and is "consumed"
    consumed: bool,
    /// Frames since consumed (for grow-and-fade animation)
    consume_frame: usize,
    /// Maximum frames for the consume animation
    max_consume_frames: usize,
    /// Frames to wait before the star becomes active (simulates JS setTimeout stagger)
    spawn_delay: usize,
}

/// JS uses 60fps; we normalize alpha/speed rates so they look the same at any fps
const JS_FPS: f32 = 60.0;

impl Star {
    /// Create a new star at a random position with a spawn delay
    fn new(width: f32, height: f32, hue: f32, max_consume_frames: usize, spawn_delay: usize) -> Self {
        let mut rng = rand::rng();
        let size = rand_range(&mut rng, 0.0, 4.0);

        Self {
            hue,
            alpha: 0.0,
            size,
            x: rand_range(&mut rng, 0.0, width),
            y: rand_range(&mut rng, 0.0, height),
            speed: size * 0.05,
            consumed: false,
            consume_frame: 0,
            max_consume_frames,
            spawn_delay,
        }
    }

    /// Reset the star: respawn at a random edge position outside the screen
    /// so it has time to become visible before reaching the black hole.
    fn reset(&mut self, width: f32, height: f32, hue: f32, max_consume_frames: usize, gravity_x: f32, gravity_y: f32) {
        let mut rng = rand::rng();
        let size = rand_range(&mut rng, 0.0, 4.0);

        // Spawn at a random edge, far from the gravity center.
        // Pick a random angle around the gravity center and place the star
        // at a distance of ~0.6–1.0 of the screen diagonal away.
        let diagonal = (width * width + height * height).sqrt();
        let angle = rand_range(&mut rng, 0.0, std::f32::consts::TAU);
        let dist = diagonal * rand_range(&mut rng, 0.45, 0.6);

        self.hue = hue;
        self.alpha = 0.0;
        self.size = size;
        self.x = gravity_x + dist * angle.cos();
        self.y = gravity_y + dist * angle.sin();
        self.speed = size * 0.05;
        self.consumed = false;
        self.consume_frame = 0;
        self.max_consume_frames = max_consume_frames;
        self.spawn_delay = 0; // No delay on respawn — already far away
    }

    /// Update star physics for one frame
    fn update(&mut self, gravity_x: f32, gravity_y: f32, black_hole_size: f32, event_horizon_offset: f32, dt: f32) {
        // Skip if still in spawn delay
        if self.spawn_delay > 0 {
            self.spawn_delay -= 1;
            return;
        }

        // Normalize rates to be fps-independent (JS runs at 60fps)
        let rate = dt * JS_FPS;

        if self.consumed {
            // Consume animation: fade out and grow
            self.alpha *= 0.42_f32.powf(rate);
            self.size += 4.0 * rate;
            self.consume_frame += 1;
        } else {
            // Check if star has reached the event horizon
            let dx = gravity_x - self.x;
            let dy = gravity_y - self.y;
            let dist = dx.hypot(dy);

            if dist < black_hole_size - event_horizon_offset {
                // Star is consumed by the black hole
                self.consumed = true;
            } else {
                // Accelerate toward gravity center
                let angle = dy.atan2(dx);
                self.alpha += 0.01 * rate;
                self.x += self.speed * angle.cos() * rate;
                self.y += self.speed * angle.sin() * rate;
                self.speed += 0.01 * rate;
            }
        }
    }

    /// Whether the star should be reset (consume animation finished)
    fn is_done(&self) -> bool {
        self.consumed && self.consume_frame > self.max_consume_frames
    }
}

/// Convert HSL to RGB
///
/// h: hue in degrees (0-360), s: saturation (0-1), l: lightness (0-1)
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// Draw an anti-aliased stroked circle
fn draw_circle_stroke(
    img: &mut RgbaImage,
    cx: f32,
    cy: f32,
    radius: f32,
    color: Rgba<u8>,
    line_width: f32,
) {
    let w = img.width() as f32;
    let h = img.height() as f32;

    // Bounding box with margin for line width
    let margin = radius + line_width + 1.0;
    let x_min = ((cx - margin).max(0.0)) as u32;
    let x_max = ((cx + margin).min(w - 1.0)) as u32;
    let y_min = ((cy - margin).max(0.0)) as u32;
    let y_max = ((cy + margin).min(h - 1.0)) as u32;

    let half_lw = line_width / 2.0;

    for py in y_min..=y_max {
        let dy = py as f32 - cy;
        for px in x_min..=x_max {
            let dx = px as f32 - cx;
            let dist = dx.hypot(dy);

            // Distance from the ideal circle edge
            let edge_dist = (dist - radius).abs();
            let alpha = if edge_dist <= half_lw {
                // Inside the stroke
                1.0
            } else if edge_dist < half_lw + 1.0 {
                // Anti-alias edge
                1.0 - (edge_dist - half_lw)
            } else {
                continue;
            };

            if alpha > 0.0 {
                blend_pixel(img, px, py, color, alpha * (color[3] as f32 / 255.0));
            }
        }
    }
}

/// Draw a filled circle with a radial gradient
fn draw_radial_gradient_circle(
    img: &mut RgbaImage,
    cx: f32,
    cy: f32,
    radius: f32,
    inner_color: Rgba<u8>,
    mid_color: Rgba<u8>,
    outer_color: Rgba<u8>,
) {
    let w = img.width() as f32;
    let h = img.height() as f32;

    let x_min = ((cx - radius - 1.0).max(0.0)) as u32;
    let x_max = ((cx + radius + 1.0).min(w - 1.0)) as u32;
    let y_min = ((cy - radius - 1.0).max(0.0)) as u32;
    let y_max = ((cy + radius + 1.0).min(h - 1.0)) as u32;

    for py in y_min..=y_max {
        let dy = py as f32 - cy;
        for px in x_min..=x_max {
            let dx = px as f32 - cx;
            let dist = dx.hypot(dy);
            let t = (dist / radius).min(1.0); // 0 at center, 1 at edge

            // Anti-alias at the edge
            let edge_alpha = if dist > radius - 1.0 {
                (radius - dist).max(0.0)
            } else {
                1.0
            };

            let color = if t < 0.9 {
                // Inner to mid: inner_color → mid_color
                let lt = t / 0.9;
                lerp_color(inner_color, mid_color, lt)
            } else {
                // Mid to outer: mid_color → outer_color
                let lt = (t - 0.9) / 0.1;
                lerp_color(mid_color, outer_color, lt)
            };

            if edge_alpha > 0.0 {
                blend_pixel(img, px, py, color, edge_alpha);
            }
        }
    }
}

/// Linear interpolation between two colors
fn lerp_color(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    Rgba([
        (a[0] as f32 * (1.0 - t) + b[0] as f32 * t).round() as u8,
        (a[1] as f32 * (1.0 - t) + b[1] as f32 * t).round() as u8,
        (a[2] as f32 * (1.0 - t) + b[2] as f32 * t).round() as u8,
        (a[3] as f32 * (1.0 - t) + b[3] as f32 * t).round() as u8,
    ])
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

/// Uniform random in [a, b]
fn rand_range(rng: &mut impl RngExt, a: f32, b: f32) -> f32 {
    rng.random::<f32>() * (b - a) + a
}

/// Black hole animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct BlackHoleConfig {
    /// Number of star particles (default: 200)
    #[derivative(Default(value = "200"))]
    pub star_count: usize,

    /// Black hole visual radius at 1080p (default: 40)
    #[derivative(Default(value = "40.0"))]
    pub black_hole_size: f32,

    /// Event horizon distance offset (default: 16)
    /// Stars are consumed when distance < black_hole_size - event_horizon_offset
    #[derivative(Default(value = "16.0"))]
    pub event_horizon_offset: f32,

    /// Consume animation frames before star resets (default: 50)
    #[derivative(Default(value = "50"))]
    pub max_consume_frames: usize,

    /// Hue rotation speed in degrees per second (default: 36, i.e. 360° in 10s)
    #[derivative(Default(value = "36.0"))]
    pub hue_speed: f32,

    /// Star saturation for HSL color (default: 0.5)
    #[derivative(Default(value = "0.5"))]
    pub star_saturation: f32,

    /// Star lightness for HSL color (default: 0.85)
    #[derivative(Default(value = "0.85"))]
    pub star_lightness: f32,

    /// Trail fade alpha per frame (default: 0.3)
    /// Corresponds to JS: `rgba(10,20,30,.3)` overlay
    #[derivative(Default(value = "0.3"))]
    pub trail_alpha: f32,

    /// Trail overlay color (RGB) (default: dark blue-black (10, 20, 30))
    #[derivative(Default(value = "(10, 20, 30)"))]
    pub trail_color: (u8, u8, u8),

    /// Background color (RGB) (default: very dark (10, 20, 30))
    #[derivative(Default(value = "(10, 20, 30)"))]
    pub bg_color: (u8, u8, u8),

    /// Black hole center X position as fraction of width (0-1, default: 0.5 = center)
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    /// Black hole center Y position as fraction of height (0-1, default: 0.5 = center)
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    /// Black hole stroke color (default: near-black)
    #[derivative(Default(value = "(20, 20, 20)"))]
    pub hole_stroke_color: (u8, u8, u8),

    /// Black hole inner fill color (gradient center)
    #[derivative(Default(value = "(0, 0, 0)"))]
    pub hole_inner_color: (u8, u8, u8),

    /// Black hole mid fill color (gradient 90%)
    #[derivative(Default(value = "(33, 33, 33)"))]
    pub hole_mid_color: (u8, u8, u8),

    /// Black hole outer fill color (gradient edge)
    #[derivative(Default(value = "(19, 20, 22)"))]
    pub hole_outer_color: (u8, u8, u8),

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

    /// Current global hue
    #[setters(skip)]
    #[serde(skip)]
    hue: f32,

    /// Star particles
    #[setters(skip)]
    #[serde(skip)]
    stars: Vec<Star>,

    /// Previous frame for motion trails
    #[setters(skip)]
    #[serde(skip)]
    prev_frame: Option<RgbaImage>,

    /// Scaled black hole size
    #[setters(skip)]
    #[serde(skip)]
    s_black_hole_size: f32,

    /// Scaled event horizon offset
    #[setters(skip)]
    #[serde(skip)]
    s_event_horizon_offset: f32,

    /// Gravity center position in pixels
    #[setters(skip)]
    #[serde(skip)]
    gravity_x: f32,

    #[setters(skip)]
    #[serde(skip)]
    gravity_y: f32,
}

impl BlackHoleConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create stars with staggered spawn delays, mimicking JS's setTimeout(i * 8).
    /// Each star gets a delay of ~i frames at the target fps (8ms * i → frame count).
    fn create_stars(&mut self) {
        self.stars.clear();
        let stagger_ms = 8.0; // JS uses setTimeout delay of 8ms per star
        let frame_duration_ms = 1000.0 / self.fps as f32;
        let delay_per_star = (stagger_ms / frame_duration_ms).ceil() as usize;

        for i in 0..self.star_count {
            let spawn_delay = i * delay_per_star;
            let star = Star::new(
                self.width as f32,
                self.height as f32,
                self.hue,
                self.max_consume_frames,
                spawn_delay,
            );
            self.stars.push(star);
        }
    }

    /// Apply the trail fade effect by overlaying a semi-transparent dark rectangle.
    /// This mimics the JS: `$.fillStyle = 'rgba(10,20,30,.3)'; $.fillRect(0,0,w,h);`
    fn apply_trail_fade(&self, img: &mut RgbaImage) {
        let fade = Rgba([
            self.trail_color.0,
            self.trail_color.1,
            self.trail_color.2,
            (self.trail_alpha * 255.0).round() as u8,
        ]);

        // Blend the fade color over the entire image
        for pixel in img.pixels_mut() {
            let src_a = fade[3] as f32 / 255.0;
            let dst_a = pixel[3] as f32 / 255.0;
            let out_a = src_a + dst_a * (1.0 - src_a);

            if out_a > 0.0 {
                pixel[0] = ((fade[0] as f32 * src_a + pixel[0] as f32 * dst_a * (1.0 - src_a))
                    / out_a)
                    .round() as u8;
                pixel[1] = ((fade[1] as f32 * src_a + pixel[1] as f32 * dst_a * (1.0 - src_a))
                    / out_a)
                    .round() as u8;
                pixel[2] = ((fade[2] as f32 * src_a + pixel[2] as f32 * dst_a * (1.0 - src_a))
                    / out_a)
                    .round() as u8;
                pixel[3] = (out_a * 255.0).round() as u8;
            }
        }
    }

    /// Draw the black hole core with radial gradient
    fn draw_black_hole(&self, img: &mut RgbaImage) {
        let radius = self.s_black_hole_size / 2.0;

        // Draw the filled radial gradient circle (the black hole body)
        draw_radial_gradient_circle(
            img,
            self.gravity_x,
            self.gravity_y,
            radius,
            Rgba([
                self.hole_inner_color.0,
                self.hole_inner_color.1,
                self.hole_inner_color.2,
                255,
            ]),
            Rgba([
                self.hole_mid_color.0,
                self.hole_mid_color.1,
                self.hole_mid_color.2,
                255,
            ]),
            Rgba([
                self.hole_outer_color.0,
                self.hole_outer_color.1,
                self.hole_outer_color.2,
                255,
            ]),
        );

        // Draw the stroke outline
        draw_circle_stroke(
            img,
            self.gravity_x,
            self.gravity_y,
            radius,
            Rgba([
                self.hole_stroke_color.0,
                self.hole_stroke_color.1,
                self.hole_stroke_color.2,
                255,
            ]),
            1.0,
        );
    }
}

/// Draw a single star onto the image
fn draw_star_impl(img: &mut RgbaImage, star: &Star, saturation: f32, lightness: f32) {
    let (r, g, b) = hsl_to_rgb(star.hue % 360.0, saturation, lightness);
    let alpha = star.alpha.min(1.0);

    if alpha <= 0.01 {
        return;
    }

    draw_circle_stroke(
        img,
        star.x,
        star.y,
        star.size,
        Rgba([r, g, b, (alpha * 255.0).round() as u8]),
        0.5,
    );
}

impl BlackHoleConfig {
    fn generate_frame(&mut self) -> RgbaImage {
        let dt = 1.0 / self.fps as f32;

        // Update global hue
        self.hue += self.hue_speed * dt;

        // Start from previous frame (for motion trails) or fresh background
        let mut img = if let Some(prev) = self.prev_frame.take() {
            prev
        } else {
            let mut img = RgbaImage::new(self.width, self.height);
            for pixel in img.pixels_mut() {
                *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
            }
            img
        };

        // Apply trail fade (semi-transparent overlay to create motion trails)
        self.apply_trail_fade(&mut img);

        // Update all stars (dt-normalized so speed is consistent across fps)
        let gravity_x = self.gravity_x;
        let gravity_y = self.gravity_y;
        let s_black_hole_size = self.s_black_hole_size;
        let s_event_horizon_offset = self.s_event_horizon_offset;
        for star in &mut self.stars {
            star.update(gravity_x, gravity_y, s_black_hole_size, s_event_horizon_offset, dt);
        }

        // Draw all stars
        let star_saturation = self.star_saturation;
        let star_lightness = self.star_lightness;
        for star in &self.stars {
            draw_star_impl(&mut img, star, star_saturation, star_lightness);
        }

        // Reset consumed stars that finished their animation
        let w = self.width as f32;
        let h = self.height as f32;
        let hue = self.hue;
        let max_consume_frames = self.max_consume_frames;
        for star in &mut self.stars {
            if star.is_done() {
                star.reset(w, h, hue, max_consume_frames, gravity_x, gravity_y);
            }
        }

        // Draw the black hole on top
        self.draw_black_hole(&mut img);

        // Store frame for next iteration's trail effect
        self.prev_frame = Some(img.clone());

        img
    }
}

impl Iterator for BlackHoleConfig {
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

impl AnimationInit for BlackHoleConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Scale pixel-based parameters for target resolution
        self.s_black_hole_size = scale_pixel_for_height(self.black_hole_size, height);
        self.s_event_horizon_offset = scale_pixel_for_height(self.event_horizon_offset, height);

        // Set gravity center position
        self.gravity_x = self.center_x * width as f32;
        self.gravity_y = self.center_y * height as f32;

        self.create_stars();
        self.prev_frame = None;
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.hue = 0.0;
        self.stars.clear();
        self.prev_frame = None;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(BlackHoleConfig);
