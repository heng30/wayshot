//! Wave background animation.
//!
//! Creates animated waves at the bottom of the screen with a flowing gradient background.
//! Based on the CSS wave animation pattern from tmp/wave.js:
//! - Animated diagonal gradient background (background-size: 400% with position animation)
//! - Multiple semi-transparent wave layers with sinusoidal curves
//! - Each wave has different speed, direction, and starting phase
//!
//! The CSS uses `border-radius: 1000% 1000% 0 0` on a 200%-width element,
//! which browsers clamp to a semi-ellipse (flat bottom, curved top).
//! The animation is purely horizontal translateX, creating the rolling wave effect.
//! We replicate this with sinusoidal curves that shift horizontally over time.

use crate::{AnimationInit, scale_pixel_for_height};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Wave animation configuration
#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct WaveConfig {
    /// Number of wave layers (default: 3)
    #[derivative(Default(value = "3"))]
    pub wave_count: usize,

    /// Wave height in pixels at 1080p (default: 200)
    /// Corresponds to CSS: height: 12em
    #[derivative(Default(value = "200.0"))]
    pub wave_height: f32,

    /// Wave animation period in seconds (default: 10)
    /// Corresponds to CSS: animation: wave 10s ...
    #[derivative(Default(value = "10.0"))]
    pub duration: f32,

    /// Wave color (RGB) (default: white)
    /// Corresponds to CSS: background: rgb(255 255 255 / 25%)
    #[derivative(Default(value = "(255, 255, 255)"))]
    pub wave_color: (u8, u8, u8),

    /// Wave base opacity (default: 0.25)
    /// Corresponds to CSS: background: rgb(255 255 255 / 25%)
    #[derivative(Default(value = "0.25"))]
    pub wave_opacity: f32,

    /// Per-wave opacity overrides (default: [0.8, 0.8, 0.9] matching CSS)
    /// Corresponds to CSS opacity on each .wave:nth-of-type
    #[derivative(Default(value = "vec![0.8, 0.8, 0.9]"))]
    pub wave_opacities: Vec<f32>,

    /// Background gradient colors (default: purple-blue-cyan-red from CSS)
    /// Corresponds to CSS: linear-gradient(315deg, ...)
    #[derivative(Default(value = "vec![
        (101, 0, 94),   // rgba(101, 0, 94, 1) 3%
        (60, 132, 206), // rgba(60, 132, 206, 1) 38%
        (48, 238, 226), // rgba(48, 238, 226, 1) 68%
        (255, 25, 25),  // rgba(255, 25, 25, 1) 98%
    ]"))]
    pub gradient_colors: Vec<(u8, u8, u8)>,

    /// Gradient stop positions 0..1 (must match gradient_colors length)
    #[derivative(Default(value = "vec![0.03, 0.38, 0.68, 0.98]"))]
    pub gradient_stops: Vec<f32>,

    /// Gradient animation duration in seconds (default: 15)
    /// Corresponds to CSS: animation: gradient 15s ease infinite
    #[derivative(Default(value = "15.0"))]
    pub gradient_duration: f32,

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

    /// Scaled wave height
    #[setters(skip)]
    #[serde(skip)]
    s_wave_height: f32,

    /// Per-wave animation configurations
    #[setters(skip)]
    #[serde(skip)]
    waves: Vec<WaveLayer>,
}

/// A single wave layer's animated state
#[derive(Debug, Clone)]
struct WaveLayer {
    /// Animation period in seconds (CSS: wave 10s / 18s / 20s)
    period: f32,
    /// Animation delay in seconds (CSS: -3s / 0s / -1s)
    delay: f32,
    /// Direction: 1.0 = forward, -1.0 = reverse (CSS: normal / reverse)
    direction: f32,
    /// Vertical offset from bottom (CSS: bottom: 0 / -1.25em / -2.5em)
    y_offset: f32,
    /// Opacity override (CSS: opacity: 0.8 / 0.8 / 0.9)
    opacity: f32,
}

impl WaveConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Interpolate gradient at a position along the diagonal (0..1)
    fn sample_gradient(&self, pos: f32) -> (u8, u8, u8) {
        let colors = &self.gradient_colors;
        let stops = &self.gradient_stops;

        if colors.is_empty() {
            return self.bg_color;
        }
        if colors.len() == 1 {
            return colors[0];
        }

        // Find the two stops bracketing pos
        let pos = pos.clamp(0.0, 1.0);
        for i in 0..stops.len() - 1 {
            if pos >= stops[i] && pos <= stops[i + 1] {
                let t = if (stops[i + 1] - stops[i]).abs() < 1e-6 {
                    0.0
                } else {
                    (pos - stops[i]) / (stops[i + 1] - stops[i])
                };
                let c1 = colors[i];
                let c2 = colors[i + 1];
                return (
                    (c1.0 as f32 * (1.0 - t) + c2.0 as f32 * t) as u8,
                    (c1.1 as f32 * (1.0 - t) + c2.1 as f32 * t) as u8,
                    (c1.2 as f32 * (1.0 - t) + c2.2 as f32 * t) as u8,
                );
            }
        }
        colors[colors.len() - 1]
    }

    /// Draw the animated gradient background.
    ///
    /// CSS uses `background-size: 400% 400%` with animated `background-position`.
    /// We simulate this by sampling the gradient along a diagonal that shifts over time.
    fn draw_gradient_background(&self, img: &mut RgbaImage, time: f32) {
        // CSS animation: gradient 15s ease infinite
        // 0% → position 0,0  50% → position 100%,100%  100% → position 0,0
        let cycle_t = (time % self.gradient_duration) / self.gradient_duration;
        // Ease: use sine for smooth back-and-forth
        let gradient_pos = (1.0 - (cycle_t * std::f32::consts::TAU).cos()) / 2.0; // 0 → 1 → 0

        // CSS: background-size: 400% 400% means gradient spans 4x the viewport
        // Diagonal gradient (315deg ≈ from top-right to bottom-left)
        let w = img.width() as f32;
        let h = img.height() as f32;

        for y in 0..img.height() {
            let y_norm = y as f32 / h;
            for x in 0..img.width() {
                let x_norm = x as f32 / w;

                // Diagonal position (315deg: top-right=0, bottom-left=1)
                // In 400% space, shift by gradient_pos
                let diag = (x_norm + y_norm) / 2.0;
                let pos_400 = (diag * 4.0 + gradient_pos * 3.0) % 4.0 / 4.0;

                let color = self.sample_gradient(pos_400);
                img.put_pixel(x, y, Rgba([color.0, color.1, color.2, 255]));
            }
        }
    }

    /// Draw a single wave layer.
    ///
    /// CSS uses a 200%-width element with `border-radius: 1000% 1000% 0 0`.
    /// Browsers clamp border-radius to the element's half-width, producing a semi-ellipse.
    /// The animation is purely horizontal (translateX), creating the rolling wave effect.
    ///
    /// We replicate this by:
    /// 1. Drawing a sinusoidal wave curve (multiple periods across 200% width)
    /// 2. Shifting the curve horizontally over time (translateX animation)
    /// 3. Filling from the curve down to the bottom of the screen
    fn draw_wave_layer(
        img: &mut RgbaImage,
        wave: &WaveLayer,
        time: f32,
        wave_height: f32,
        wave_color: (u8, u8, u8),
        base_opacity: f32,
        width: u32,
        height: u32,
    ) {
        let w = width as f32;
        let h = height as f32;

        // Apply animation delay
        let effective_time = time + wave.delay;

        // CSS keyframes: wave 10s linear infinite
        // 2%: translateX(1)  25%: translateX(-25%)  50%: translateX(-50%)
        // 75%: translateX(-25%)  100%: translateX(1)
        // This is a smooth back-and-forth oscillation of the 200%-wide element.
        // The element moves between translateX(0) and translateX(-50%),
        // i.e. it shifts by up to half its width (= one viewport width).
        let cycle_t = (effective_time / wave.period) % 1.0;
        // Approximate the CSS keyframe path with cosine
        // Normal: shifts left then back; Reverse: shifts right then back
        let translate_fraction = if wave.direction > 0.0 {
            // 0 → -0.5 → 0 (shift left then back)
            -0.25 * (1.0 - (cycle_t * std::f32::consts::TAU).cos())
        } else {
            // 0 → +0.5 → 0 (shift right then back)
            0.25 * (1.0 - (cycle_t * std::f32::consts::TAU).cos())
        };
        // translate_fraction is in units of viewport width
        // Since the wave element is 200% wide, translateX(-50%) = -1 viewport width
        let shift_x = translate_fraction * w;

        // Wave base Y: bottom of screen minus wave height plus offset
        let wave_base_y = h - wave_height + wave.y_offset;

        // The wave shape: a semi-elliptical arc from border-radius: 1000% 1000% 0 0
        // on a 200%-width element. Browsers clamp border-radius to the element's
        // half-width (for 200%-width = viewport width), producing a semi-ellipse
        // that spans the entire element width (2w) with horizontal radius = w.
        //
        // In CSS, the viewport only sees a portion of this huge ellipse — always
        // the gentle middle part. To replicate this, we use a horizontal radius
        // of 2w so the viewport (even at max shift ±0.5w) always stays within
        // the flat center of the ellipse, never reaching the steep endpoints.
        //
        // shift_x ranges ±0.5w, so the furthest viewport edge from the ellipse
        // center is at |shift_x + w ± 0.5w| = 1.5w. With rx=2w the max
        // dx_ratio = 1.5w/2w = 0.75 — still in the gentle region.

        let ellipse_cx = shift_x + w; // Center of the 200%-wide element
        let rx = 2.0 * w; // Horizontal radius: keep viewport in the gentle center
        let ry = wave_height * 0.5; // Vertical radius scaled to match visible arc height

        let effective_opacity = base_opacity * wave.opacity;

        for x in 0..width {
            let x_f = x as f32;
            let dx = x_f - ellipse_cx;
            let dx_ratio = dx / rx;
            let dx_ratio_sq = dx_ratio * dx_ratio;

            // Skip if outside the ellipse
            if dx_ratio_sq >= 1.0 {
                continue;
            }

            // Semi-ellipse top edge: y = base_y - ry * sqrt(1 - (dx/rx)^2)
            let arc_y = ry * (1.0 - dx_ratio_sq).sqrt();
            let top_y = wave_base_y - arc_y;

            // Anti-aliased fill: blend the 1-2px band at the arc edge
            // with sub-pixel alpha, then fill the rest at full opacity.
            let y_floor = top_y.floor();
            let fract = top_y - y_floor; // 0..1 sub-pixel fraction

            // The pixel at y_floor gets partial coverage (1 - fract)
            // The pixels from y_floor+1 onward get full coverage
            let aa_y = y_floor as u32;

            // Partial coverage at arc edge
            if aa_y < height {
                let edge_alpha = effective_opacity * (1.0 - fract);
                Self::blend_pixel(img, aa_y, x, wave_color, edge_alpha);
            }

            // Full coverage from aa_y+1 to bottom
            let y_start = (aa_y + 1).min(height);
            for y in y_start..height {
                Self::blend_pixel(img, y, x, wave_color, effective_opacity);
            }
        }
    }

    /// Blend wave color onto an existing pixel
    fn blend_pixel(img: &mut RgbaImage, y: u32, x: u32, color: (u8, u8, u8), alpha: f32) {
        if alpha <= 0.0 || x >= img.width() || y >= img.height() {
            return;
        }
        let alpha = alpha.min(1.0);
        let existing = img.get_pixel(x, y);
        let bg_alpha = 1.0 - alpha;

        let new_r = ((existing.0[0] as f32 * bg_alpha + color.0 as f32 * alpha).min(255.0)) as u8;
        let new_g = ((existing.0[1] as f32 * bg_alpha + color.1 as f32 * alpha).min(255.0)) as u8;
        let new_b = ((existing.0[2] as f32 * bg_alpha + color.2 as f32 * alpha).min(255.0)) as u8;

        img.put_pixel(x, y, Rgba([new_r, new_g, new_b, 255]));
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);
        let time = self.current_frame as f32 / self.fps as f32;

        // Draw gradient background
        self.draw_gradient_background(&mut img, time);

        // Draw waves from back to front (last wave drawn is on top)
        for wave in &self.waves {
            Self::draw_wave_layer(
                &mut img,
                wave,
                time,
                self.s_wave_height,
                self.wave_color,
                self.wave_opacity,
                self.width,
                self.height,
            );
        }

        img
    }
}

impl Iterator for WaveConfig {
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

impl AnimationInit for WaveConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Scale wave height for target resolution
        self.s_wave_height = scale_pixel_for_height(self.wave_height, height);

        // Initialize wave layers matching the CSS defaults:
        // .wave         → animation: wave 10s -3s linear infinite;       bottom: 0;      opacity: 0.8
        // .wave:nth(2)  → animation: wave 18s linear reverse infinite;   bottom: -1.25em; opacity: 0.8
        // .wave:nth(3)  → animation: wave 20s -1s reverse infinite;      bottom: -2.5em;  opacity: 0.9
        self.waves.clear();

        // Scale 1em ≈ 16px at 1080p, then scale for target height
        let em_px = scale_pixel_for_height(16.0, height);

        for i in 0..self.wave_count {
            let (period, delay, direction, y_offset_em, opacity) = match i {
                0 => (self.duration, -3.0, 1.0, 0.0, 0.8),
                1 => (self.duration * 1.8, 0.0, -1.0, -1.25, 0.8),
                2 => (self.duration * 2.0, -1.0, -1.0, -2.5, 0.9),
                // For additional waves beyond 3, generate reasonable defaults
                _ => (
                    self.duration * (1.0 + i as f32 * 0.3),
                    -(i as f32),
                    if i % 2 == 0 { 1.0 } else { -1.0 },
                    -(i as f32 * 1.25),
                    0.7 + 0.05 * i as f32,
                ),
            };

            // Override opacity if provided in wave_opacities
            let opacity = self.wave_opacities.get(i).copied().unwrap_or(opacity);

            self.waves.push(WaveLayer {
                period,
                delay,
                direction,
                y_offset: y_offset_em * em_px,
                opacity,
            });
        }

        // Validate gradient stops match colors
        if self.gradient_stops.len() != self.gradient_colors.len() {
            let n = self.gradient_colors.len();
            self.gradient_stops = (0..n)
                .map(|i| i as f32 / (n - 1).max(1) as f32)
                .collect();
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.waves.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(WaveConfig);
