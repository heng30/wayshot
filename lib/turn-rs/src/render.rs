//! Pixel-level page flip rendering using the perpendicular bisector fold model.
//!
//! The core API is [`render_flip`]: given a [`FlipConfig`] (direction, total
//! duration, current time, corner) and two page images (front and back),
//! it returns the frame at that point in time.

use crate::flip::Corner;
use image::{Rgba, RgbaImage};

/// Flip animation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlipDirection {
    /// Forward flip: page flips from front to back (progress 0 → 1).
    #[default]
    Forward,
    /// Backward flip: page flips from back to front (progress 1 → 0).
    Backward,
    /// Round-trip flip: forward then backward (progress 0 → 1 → 0).
    RoundTrip,
}

/// Flip axis: determines whether the page turns horizontally or vertically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlipAxis {
    /// Horizontal flip: book-style left/right page turn (default).
    #[default]
    Horizontal,
    /// Vertical flip: calendar-style up/down page turn.
    Vertical,
}

/// Configuration for the flip effect.
///
/// Use `time_ms` to request a specific point in the animation.
/// The library computes the corresponding progress based on `direction`
/// and `duration_ms`.
#[derive(Debug, Clone)]
pub struct FlipConfig {
    /// Which corner the flip originates from.
    pub corner: Corner,
    /// Total animation duration in milliseconds.
    pub duration_ms: u32,
    /// Current time in milliseconds (0 .. duration_ms).
    /// For `RoundTrip`, the full duration covers both forward and backward.
    pub time_ms: u32,
    /// Whether to render shadow/highlight gradients.
    pub shadow: bool,
    /// Flip direction: Forward, Backward, or RoundTrip.
    pub direction: FlipDirection,
    /// Flip axis: Horizontal (book-style) or Vertical (calendar-style).
    pub axis: FlipAxis,
    /// Flip extent: how far the page flips, in range 0.0–1.0.
    ///
    /// - `1.0` (default): full flip — the page turns all the way over,
    ///   fully revealing the back page.
    /// - `0.5`: half flip — the page only folds halfway, creating a
    ///   bent/curl effect without turning over.
    /// - `0.0`: no flip — the page stays flat (identical to progress = 0).
    ///
    /// This scales the effective progress so that the maximum fold
    /// displacement is `flip_extent` of the full flip.
    pub flip_extent: f64,
}

impl Default for FlipConfig {
    fn default() -> Self {
        FlipConfig {
            corner: Corner::BottomRight,
            duration_ms: 600,
            time_ms: 0,
            shadow: true,
            direction: FlipDirection::Forward,
            axis: FlipAxis::Horizontal,
            flip_extent: 1.0,
        }
    }
}

impl FlipConfig {
    /// Compute the animation progress (0.0 .. 1.0) from `time_ms`.
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        let t = (self.time_ms as f64 / self.duration_ms as f64).clamp(0.0, 1.0);
        let raw = match self.direction {
            FlipDirection::Forward => t,
            FlipDirection::Backward => 1.0 - t,
            FlipDirection::RoundTrip => {
                // 0 → 1 in first half, 1 → 0 in second half
                if t <= 0.5 { t * 2.0 } else { 2.0 * (1.0 - t) }
            }
        };
        // Scale by flip_extent: extent=1 → full flip, extent=0.5 → half flip
        let extent = self.flip_extent.clamp(0.0, 1.0);
        raw * extent
    }
}

#[derive(Debug, Clone, Copy)]
struct Pt {
    x: f64,
    y: f64,
}

impl Pt {
    fn new(x: f64, y: f64) -> Self {
        Pt { x, y }
    }
}

/// Precomputed fold line geometry for a corner flip.
struct FoldLine {
    corner_pt: Pt,
    mid: Pt,
    dir: Pt,
}

impl FoldLine {
    fn new(corner: Corner, fold_pt: Pt, page_w: f64, page_h: f64) -> Self {
        let corner_pt = match corner {
            Corner::BottomRight => Pt::new(page_w, page_h),
            Corner::BottomLeft => Pt::new(0.0, page_h),
            Corner::TopRight => Pt::new(page_w, 0.0),
            Corner::TopLeft => Pt::new(0.0, 0.0),
        };
        let mid = Pt::new(
            (fold_pt.x + corner_pt.x) / 2.0,
            (fold_pt.y + corner_pt.y) / 2.0,
        );
        let seg_dx = corner_pt.x - fold_pt.x;
        let seg_dy = corner_pt.y - fold_pt.y;
        let dir = Pt::new(-seg_dy, seg_dx);
        FoldLine {
            corner_pt,
            mid,
            dir,
        }
    }
}

fn fold_point_at_progress(
    page_w: f64,
    page_h: f64,
    progress: f64,
    corner: Corner,
    axis: FlipAxis,
) -> Pt {
    let t = ease_out(progress);

    match axis {
        FlipAxis::Horizontal => {
            // Book-style: fold point sweeps horizontally
            let lift = page_h * 0.10 * (4.0 * t * (1.0 - t));
            match corner {
                Corner::BottomRight => {
                    let fx = page_w * (1.0 - t);
                    let fy = page_h * (1.0 - t) - lift;
                    Pt::new(fx, fy.max(0.0))
                }
                Corner::BottomLeft => {
                    let fx = page_w * t;
                    let fy = page_h * (1.0 - t) - lift;
                    Pt::new(fx, fy.max(0.0))
                }
                Corner::TopRight => {
                    let fx = page_w * (1.0 - t);
                    let fy = page_h * t + lift;
                    Pt::new(fx, fy.min(page_h))
                }
                Corner::TopLeft => {
                    let fx = page_w * t;
                    let fy = page_h * t + lift;
                    Pt::new(fx, fy.min(page_h))
                }
            }
        }
        FlipAxis::Vertical => {
            // Calendar-style: fold point sweeps vertically
            let lift = page_w * 0.10 * (4.0 * t * (1.0 - t));
            match corner {
                Corner::TopRight => {
                    // Flip down: fold point starts at top-right, sweeps down
                    let fx = page_w * (1.0 - t) - lift;
                    let fy = page_h * t;
                    Pt::new(fx.max(0.0), fy)
                }
                Corner::TopLeft => {
                    // Flip down: fold point starts at top-left, sweeps down
                    let fx = page_w * t + lift;
                    let fy = page_h * t;
                    Pt::new(fx.min(page_w), fy)
                }
                Corner::BottomRight => {
                    // Flip up: fold point starts at bottom-right, sweeps up
                    let fx = page_w * (1.0 - t) - lift;
                    let fy = page_h * (1.0 - t);
                    Pt::new(fx.max(0.0), fy)
                }
                Corner::BottomLeft => {
                    // Flip up: fold point starts at bottom-left, sweeps up
                    let fx = page_w * t + lift;
                    let fy = page_h * (1.0 - t);
                    Pt::new(fx.min(page_w), fy)
                }
            }
        }
    }
}

fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn reflect(px: f64, py: f64, ax: f64, ay: f64, dx: f64, dy: f64) -> (f64, f64) {
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return (px, py);
    }
    let apx = px - ax;
    let apy = py - ay;
    let t = (apx * dx + apy * dy) / len_sq;
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    (2.0 * proj_x - px, 2.0 * proj_y - py)
}

fn on_flat_side(px: f64, py: f64, fl: &FoldLine) -> bool {
    let cross_pt = fl.dir.x * (py - fl.mid.y) - fl.dir.y * (px - fl.mid.x);
    let cross_corner =
        fl.dir.x * (fl.corner_pt.y - fl.mid.y) - fl.dir.y * (fl.corner_pt.x - fl.mid.x);
    cross_pt * cross_corner < 0.0
}

fn fold_x_at_y(y: f64, mid: Pt, dir: Pt) -> Option<f64> {
    if dir.y.abs() < 1e-10 {
        return None;
    }
    let t = (y - mid.y) / dir.y;
    Some(mid.x + t * dir.x)
}

fn fold_y_at_x(x: f64, mid: Pt, dir: Pt) -> Option<f64> {
    if dir.x.abs() < 1e-10 {
        return None;
    }
    let t = (x - mid.x) / dir.x;
    Some(mid.y + t * dir.y)
}

// ============================================================================
// Public API
// ============================================================================

/// Render a single page flip frame at the time specified by `config.time_ms`.
///
/// - `front` = the current page (being flipped away)
/// - `back` = the page revealed behind the fold
/// - Returns an `RgbaImage` with the same dimensions as the input images.
///
/// The animation progress is derived from `config.time_ms / config.duration_ms`,
/// adjusted by `config.direction` (Forward / Backward / RoundTrip) and
/// scaled by `config.flip_extent`.
///
/// # Example
///
/// ```rust
/// use image::{Rgba, RgbaImage};
/// use turn_rs::{Corner, FlipAxis, FlipConfig, FlipDirection, render_flip};
///
/// let w = 400u32;
/// let h = 600u32;
/// let front = RgbaImage::from_pixel(w, h, Rgba([70, 130, 220, 255]));
/// let back  = RgbaImage::from_pixel(w, h, Rgba([80, 180, 80, 255]));
///
/// let config = FlipConfig {
///     corner: Corner::BottomRight,
///     duration_ms: 800,
///     time_ms: 400,      // halfway through
///     shadow: true,
///     direction: FlipDirection::Forward,
///     axis: FlipAxis::Horizontal,
///     flip_extent: 1.0,  // full flip
/// };
///
/// let frame = render_flip(&front, &back, &config);
/// assert_eq!(frame.width(), w);
/// assert_eq!(frame.height(), h);
/// ```
pub fn render_flip(front: &RgbaImage, back: &RgbaImage, config: &FlipConfig) -> RgbaImage {
    let progress = config.progress();
    let page_w = front.width() as f64;
    let page_h = front.height() as f64;
    let out_w = front.width();
    let out_h = front.height();

    let mut canvas = RgbaImage::from_pixel(out_w, out_h, Rgba([0, 0, 0, 0]));

    if progress <= 0.001 {
        copy_image_offset(&mut canvas, front, 0);
        return canvas;
    }

    if progress >= 0.99 {
        copy_image_offset(&mut canvas, back, 0);
        return canvas;
    }

    let fold_pt = fold_point_at_progress(page_w, page_h, progress, config.corner, config.axis);
    let fl = FoldLine::new(config.corner, fold_pt, page_w, page_h);

    // Layer 1: Back page fully visible (revealed behind the fold)
    copy_image_offset(&mut canvas, back, 0);

    // Layer 2: Visible front page (flat side of fold line)
    draw_visible_front(&mut canvas, front, &fl);

    // Layer 3: Folded-over portion (flap on flat side near crease)
    draw_folded_over(&mut canvas, front, &fl, page_w);

    // Layer 4: Gradients
    if config.shadow {
        draw_gradients(&mut canvas, &fl, page_w, page_h, progress, config.axis);
    }

    canvas
}

// ============================================================================
// Rendering helpers
// ============================================================================

fn draw_visible_front(canvas: &mut RgbaImage, front: &RgbaImage, fl: &FoldLine) {
    let fw = front.width() as f64;
    let fh = front.height() as f64;

    for y in 0..front.height() {
        let yf = y as f64;
        for x in 0..front.width() {
            let xf = x as f64;
            if !on_flat_side(xf, yf, fl) {
                continue;
            }
            let (sx, sy) = reflect(xf, yf, fl.mid.x, fl.mid.y, fl.dir.x, fl.dir.y);
            if sx >= 0.0 && sx < fw && sy >= 0.0 && sy < fh && !on_flat_side(sx, sy, fl) {
                continue;
            }
            canvas.put_pixel(x, y, *front.get_pixel(x, y));
        }
    }
}

fn draw_folded_over(canvas: &mut RgbaImage, front: &RgbaImage, fl: &FoldLine, page_w: f64) {
    let fw = front.width() as f64;
    let fh = front.height() as f64;
    let pw = page_w as u32;
    let ph = fh as u32;

    for y in 0..ph {
        let yf = y as f64;
        for x in 0..pw {
            let xf = x as f64;
            if !on_flat_side(xf, yf, fl) {
                continue;
            }
            let (sx, sy) = reflect(xf, yf, fl.mid.x, fl.mid.y, fl.dir.x, fl.dir.y);
            if on_flat_side(sx, sy, fl) {
                continue;
            }
            if sx >= 0.0 && sx < fw && sy >= 0.0 && sy < fh {
                let pixel = sample_bilinear(front, sx, sy);
                let darkened = darken(pixel, 0.75);
                // Blend the flap over the front image at this position.
                // This prevents dark semi-transparent fringes at the boundary
                // between opaque content and transparent padding — when the
                // flap pixel has low alpha (from content edge sampling), it
                // blends with the opaque front pixel instead of appearing
                // as a dark border on a transparent canvas.
                let base = *front.get_pixel(x, y);
                let blended = alpha_blend(base, darkened);
                canvas.put_pixel(x, y, blended);
            }
        }
    }
}

/// Alpha-blend `src` over `dst` (src-over compositing).
fn alpha_blend(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
    let sa = src.0[3] as f64 / 255.0;
    let da = dst.0[3] as f64 / 255.0;
    let one_minus_sa = 1.0 - sa;
    let out_a = sa + da * one_minus_sa;
    if out_a < 1e-6 {
        return Rgba([0, 0, 0, 0]);
    }
    let r = (src.0[0] as f64 * sa + dst.0[0] as f64 * da * one_minus_sa) / out_a;
    let g = (src.0[1] as f64 * sa + dst.0[1] as f64 * da * one_minus_sa) / out_a;
    let b = (src.0[2] as f64 * sa + dst.0[2] as f64 * da * one_minus_sa) / out_a;
    Rgba([
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

fn draw_gradients(
    canvas: &mut RgbaImage,
    fl: &FoldLine,
    page_w: f64,
    page_h: f64,
    progress: f64,
    axis: FlipAxis,
) {
    match axis {
        FlipAxis::Horizontal => draw_gradients_horizontal(canvas, fl, page_w, progress),
        FlipAxis::Vertical => draw_gradients_vertical(canvas, fl, page_h, progress),
    }
}

fn draw_gradients_horizontal(canvas: &mut RgbaImage, fl: &FoldLine, page_w: f64, progress: f64) {
    let shadow_w = (40.0 * progress).min(page_w * 0.12);
    let hl_w = (25.0 * progress).min(page_w * 0.08);
    if shadow_w < 1.0 {
        return;
    }

    let shadow_max = 0.35 * progress;
    let hl_max = 0.18 * progress;
    let fw = page_w as u32;
    let ch = canvas.height();

    for y in 0..ch {
        let yf = y as f64;
        if let Some(fx) = fold_x_at_y(yf, fl.mid, fl.dir) {
            let s_end = fx.min(page_w).max(0.0) as u32;
            let s_start = (fx - shadow_w).max(0.0) as u32;

            for x in s_start..s_end.min(fw) {
                let xf = x as f64;
                if !on_flat_side(xf, yf, fl) {
                    continue;
                }
                let (sx, sy) = reflect(xf, yf, fl.mid.x, fl.mid.y, fl.dir.x, fl.dir.y);
                if sx >= 0.0
                    && sx < page_w
                    && sy >= 0.0
                    && sy < ch as f64
                    && !on_flat_side(sx, sy, fl)
                {
                    continue;
                }
                let dist = (fx - xf).max(0.0);
                let t = (1.0 - dist / shadow_w).clamp(0.0, 1.0);
                let alpha = (shadow_max * t * t * 255.0) as u8;
                if alpha > 0 {
                    let p = *canvas.get_pixel(x, y);
                    canvas.put_pixel(x, y, blend_shadow(p, alpha));
                }
            }

            if hl_w >= 1.0 {
                let h_start = (fx - hl_w).max(0.0) as u32;
                let h_end = fx.ceil().min(page_w).max(0.0) as u32;
                for x in h_start..h_end.min(fw) {
                    let xf = x as f64;
                    if !on_flat_side(xf, yf, fl) {
                        continue;
                    }
                    let (sx, sy) = reflect(xf, yf, fl.mid.x, fl.mid.y, fl.dir.x, fl.dir.y);
                    if sx < 0.0
                        || sx >= page_w
                        || sy < 0.0
                        || sy >= ch as f64
                        || on_flat_side(sx, sy, fl)
                    {
                        continue;
                    }
                    let dist = (fx - xf).max(0.0);
                    let t = (1.0 - dist / hl_w).clamp(0.0, 1.0);
                    let alpha = (hl_max * t * 255.0) as u8;
                    if alpha > 0 {
                        let p = *canvas.get_pixel(x, y);
                        canvas.put_pixel(x, y, blend_highlight(p, alpha));
                    }
                }
            }
        }
    }
}

fn draw_gradients_vertical(canvas: &mut RgbaImage, fl: &FoldLine, page_h: f64, progress: f64) {
    let shadow_h = (40.0 * progress).min(page_h * 0.12);
    let hl_h = (25.0 * progress).min(page_h * 0.08);
    if shadow_h < 1.0 {
        return;
    }

    let shadow_max = 0.35 * progress;
    let hl_max = 0.18 * progress;
    let fh = page_h as u32;
    let cw = canvas.width();

    for x in 0..cw {
        let xf = x as f64;
        if let Some(fy) = fold_y_at_x(xf, fl.mid, fl.dir) {
            let s_end = fy.min(page_h).max(0.0) as u32;
            let s_start = (fy - shadow_h).max(0.0) as u32;

            for y in s_start..s_end.min(fh) {
                let yf = y as f64;
                if !on_flat_side(xf, yf, fl) {
                    continue;
                }
                let (sx, sy) = reflect(xf, yf, fl.mid.x, fl.mid.y, fl.dir.x, fl.dir.y);
                if sx >= 0.0
                    && sx < cw as f64
                    && sy >= 0.0
                    && sy < page_h
                    && !on_flat_side(sx, sy, fl)
                {
                    continue;
                }
                let dist = (fy - yf).max(0.0);
                let t = (1.0 - dist / shadow_h).clamp(0.0, 1.0);
                let alpha = (shadow_max * t * t * 255.0) as u8;
                if alpha > 0 {
                    let p = *canvas.get_pixel(x, y);
                    canvas.put_pixel(x, y, blend_shadow(p, alpha));
                }
            }

            if hl_h >= 1.0 {
                let h_start = (fy - hl_h).max(0.0) as u32;
                let h_end = fy.ceil().min(page_h).max(0.0) as u32;
                for y in h_start..h_end.min(fh) {
                    let yf = y as f64;
                    if !on_flat_side(xf, yf, fl) {
                        continue;
                    }
                    let (sx, sy) = reflect(xf, yf, fl.mid.x, fl.mid.y, fl.dir.x, fl.dir.y);
                    if sx < 0.0
                        || sx >= cw as f64
                        || sy < 0.0
                        || sy >= page_h
                        || on_flat_side(sx, sy, fl)
                    {
                        continue;
                    }
                    let dist = (fy - yf).max(0.0);
                    let t = (1.0 - dist / hl_h).clamp(0.0, 1.0);
                    let alpha = (hl_max * t * 255.0) as u8;
                    if alpha > 0 {
                        let p = *canvas.get_pixel(x, y);
                        canvas.put_pixel(x, y, blend_highlight(p, alpha));
                    }
                }
            }
        }
    }
}

fn copy_image_offset(canvas: &mut RgbaImage, page: &RgbaImage, x_offset: u32) {
    let w = page.width().min(canvas.width().saturating_sub(x_offset));
    let h = page.height().min(canvas.height());
    for y in 0..h {
        for x in 0..w {
            canvas.put_pixel(x_offset + x, y, *page.get_pixel(x, y));
        }
    }
}

fn sample_bilinear(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let w = img.width();
    let h = img.height();
    let p00 = if x0 < w && y0 < h {
        *img.get_pixel(x0, y0)
    } else {
        Rgba([0, 0, 0, 0])
    };
    let p10 = if x1 < w && y0 < h {
        *img.get_pixel(x1, y0)
    } else {
        Rgba([0, 0, 0, 0])
    };
    let p01 = if x0 < w && y1 < h {
        *img.get_pixel(x0, y1)
    } else {
        Rgba([0, 0, 0, 0])
    };
    let p11 = if x1 < w && y1 < h {
        *img.get_pixel(x1, y1)
    } else {
        Rgba([0, 0, 0, 0])
    };

    // Premultiplied alpha interpolation to avoid dark fringes at
    // boundaries between opaque content and transparent padding.
    // Without this, linear interpolation of (R,G,B,A) between e.g.
    // (128,128,128,255) and (0,0,0,0) produces dark semi-transparent
    // pixels like (64,64,64,128) which appear as black borders.
    let a00 = p00.0[3] as f64 / 255.0;
    let a10 = p10.0[3] as f64 / 255.0;
    let a01 = p01.0[3] as f64 / 255.0;
    let a11 = p11.0[3] as f64 / 255.0;

    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    let a_out = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
    if a_out < 1e-6 {
        return Rgba([0, 0, 0, 0]);
    }

    // Interpolate premultiplied RGB, then un-premultiply
    let r_pm = p00.0[0] as f64 * a00 * w00
        + p10.0[0] as f64 * a10 * w10
        + p01.0[0] as f64 * a01 * w01
        + p11.0[0] as f64 * a11 * w11;
    let g_pm = p00.0[1] as f64 * a00 * w00
        + p10.0[1] as f64 * a10 * w10
        + p01.0[1] as f64 * a01 * w01
        + p11.0[1] as f64 * a11 * w11;
    let b_pm = p00.0[2] as f64 * a00 * w00
        + p10.0[2] as f64 * a10 * w10
        + p01.0[2] as f64 * a01 * w01
        + p11.0[2] as f64 * a11 * w11;

    let inv_a = 1.0 / a_out;
    Rgba([
        (r_pm * inv_a).round().clamp(0.0, 255.0) as u8,
        (g_pm * inv_a).round().clamp(0.0, 255.0) as u8,
        (b_pm * inv_a).round().clamp(0.0, 255.0) as u8,
        (a_out * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

fn darken(p: Rgba<u8>, f: f64) -> Rgba<u8> {
    Rgba([
        (p.0[0] as f64 * f).round().clamp(0.0, 255.0) as u8,
        (p.0[1] as f64 * f).round().clamp(0.0, 255.0) as u8,
        (p.0[2] as f64 * f).round().clamp(0.0, 255.0) as u8,
        p.0[3],
    ])
}

fn blend_shadow(p: Rgba<u8>, a: u8) -> Rgba<u8> {
    let a = a as f64 / 255.0;
    Rgba([
        (p.0[0] as f64 * (1.0 - a)).round().clamp(0.0, 255.0) as u8,
        (p.0[1] as f64 * (1.0 - a)).round().clamp(0.0, 255.0) as u8,
        (p.0[2] as f64 * (1.0 - a)).round().clamp(0.0, 255.0) as u8,
        p.0[3],
    ])
}

fn blend_highlight(p: Rgba<u8>, a: u8) -> Rgba<u8> {
    let a = a as f64 / 255.0;
    Rgba([
        (p.0[0] as f64 + (255.0 - p.0[0] as f64) * a)
            .round()
            .clamp(0.0, 255.0) as u8,
        (p.0[1] as f64 + (255.0 - p.0[1] as f64) * a)
            .round()
            .clamp(0.0, 255.0) as u8,
        (p.0[2] as f64 + (255.0 - p.0[2] as f64) * a)
            .round()
            .clamp(0.0, 255.0) as u8,
        p.0[3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flip_extent_half() {
        let w = 200u32;
        let h = 200u32;
        let front = RgbaImage::from_pixel(w, h, Rgba([128, 128, 128, 255]));
        let back = RgbaImage::from_pixel(w, h, Rgba([80, 180, 80, 255]));

        // At full flip (extent=1.0), progress=1.0 should reveal the back page
        let full = FlipConfig {
            corner: Corner::BottomRight,
            duration_ms: 600,
            time_ms: 600,
            shadow: false,
            direction: FlipDirection::Forward,
            axis: FlipAxis::Horizontal,
            flip_extent: 1.0,
        };
        let result_full = render_flip(&front, &back, &full);
        let green_count_full = count_color(&result_full, 80, 180, 80);

        // At half flip (extent=0.5), progress is capped at 0.5 → page only folds halfway
        let half = FlipConfig {
            corner: Corner::BottomRight,
            duration_ms: 600,
            time_ms: 600,
            shadow: false,
            direction: FlipDirection::Forward,
            axis: FlipAxis::Horizontal,
            flip_extent: 0.5,
        };
        let result_half = render_flip(&front, &back, &half);
        let green_count_half = count_color(&result_half, 80, 180, 80);

        // Full flip should reveal more back page than half flip
        assert!(
            green_count_full > green_count_half,
            "Full flip should reveal more back page than half flip ({} vs {})",
            green_count_full,
            green_count_half,
        );

        // At extent=0, the image should be unchanged (no flip at all)
        let no_flip = FlipConfig {
            corner: Corner::BottomRight,
            duration_ms: 600,
            time_ms: 600,
            shadow: false,
            direction: FlipDirection::Forward,
            axis: FlipAxis::Horizontal,
            flip_extent: 0.0,
        };
        let result_no = render_flip(&front, &back, &no_flip);
        for y in 0..h {
            for x in 0..w {
                let p = result_no.get_pixel(x, y);
                let f = front.get_pixel(x, y);
                assert_eq!(
                    p, f,
                    "Pixel at ({}, {}) should match front when flip_extent=0",
                    x, y
                );
            }
        }
    }

    #[test]
    fn test_flip_extent_progress_clamp() {
        let mut config = FlipConfig {
            corner: Corner::BottomRight,
            duration_ms: 1000,
            time_ms: 500,
            shadow: false,
            direction: FlipDirection::Forward,
            axis: FlipAxis::Horizontal,
            flip_extent: 0.5,
        };
        // At time_ms=500/1000, raw progress = 0.5, with extent=0.5 → effective = 0.25
        let p = config.progress();
        assert!((p - 0.25).abs() < 1e-6, "Expected progress 0.25, got {}", p);

        // extent=1.0 → effective = 0.5
        config.flip_extent = 1.0;
        let p = config.progress();
        assert!((p - 0.5).abs() < 1e-6, "Expected progress 0.5, got {}", p);

        // extent=0.0 → effective = 0.0
        config.flip_extent = 0.0;
        let p = config.progress();
        assert!((p - 0.0).abs() < 1e-6, "Expected progress 0.0, got {}", p);
    }

    fn count_color(img: &RgbaImage, r: u8, g: u8, b: u8) -> usize {
        let mut count = 0;
        for y in 0..img.height() {
            for x in 0..img.width() {
                let p = img.get_pixel(x, y);
                if p[0] == r && p[1] == g && p[2] == b {
                    count += 1;
                }
            }
        }
        count
    }
}
