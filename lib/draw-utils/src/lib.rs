//! Drawing utility functions for 2D rasterization with anti-aliasing.
//!
//! Provides common primitives for pixel-level rendering:
//! - Alpha compositing (`blend_pixel`, `blend_pixel_inplace`)
//! - Color interpolation (`blend_colors`)
//! - Rounded rectangle SDF queries (`inside_rounded_rect`, `distance_to_rounded_rect_edge`)
//! - Smoothstep helper for AA transitions (`smoothstep`)
//! - Separable box blur (`box_blur_horizontal`, `box_blur_vertical`, `apply_box_blur`)
//! - Distance-based blur factor with smoothstep transition (`blur_factor`)
//! - Alpha mask operations (`dilate_mask`, `fill_from_mask`)
//! - Image compositing (`composite_image`)
//!
//! # Pixel center sampling
//!
//! When using SDF + smoothstep for anti-aliased rendering, sample at pixel centers
//! to avoid half-pixel edge shifts:
//!
//! ```ignore
//! let sdf = distance_to_rounded_rect_edge(x as f32 + 0.5, y as f32 + 0.5, w, h, r);
//! let alpha = smoothstep(-0.5, 0.5, sdf);
//! ```

use image::{Rgba, RgbaImage};

/// Blend a pixel onto the buffer using alpha compositing ("over" operation).
///
/// `opacity` is an additional multiplier on top of the color's own alpha channel,
/// clamped to `[0, 1]` to prevent overflow.
///
/// This is the standard Porter-Duff "over" compositing with non-premultiplied alpha
/// (straight alpha): source and destination RGB are kept in straight form, and the
/// output is divided by the composite alpha to produce a straight-alpha result.
#[inline]
pub fn blend_pixel(buffer: &mut RgbaImage, x: u32, y: u32, color: &Rgba<u8>, opacity: f32) {
    if x >= buffer.width() || y >= buffer.height() {
        return;
    }
    let src_alpha = (opacity * (color[3] as f32 / 255.0)).clamp(0.0, 1.0);
    if src_alpha <= 0.0 {
        return;
    }
    let pixel = buffer.get_pixel_mut(x, y);
    let dst_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha > 0.0 {
        pixel[0] = ((color[0] as f32 * src_alpha + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha)
            .round() as u8;
        pixel[1] = ((color[1] as f32 * src_alpha + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha)
            .round() as u8;
        pixel[2] = ((color[2] as f32 * src_alpha + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha)
            .round() as u8;
        pixel[3] = (out_alpha * 255.0).round() as u8;
    }
}

/// Linearly interpolate between two colors.
///
/// `t = 0.0` returns `c1`, `t = 1.0` returns `c2`.
/// Values outside `[0, 1]` are clamped.
///
/// Note: this performs interpolation in sRGB space (not linearized), which is
/// the common approach in most GUI toolkits. For perceptually correct blending,
/// colors should be converted to linear space first, interpolated, then converted back.
///
/// For gradients involving transparent colors, premultiplied-alpha interpolation
/// (pre-multiply RGB by A, lerp, then un-pre-multiply) avoids gray halos at
/// transparent-to-opaque transitions.
#[inline]
pub fn blend_colors(c1: Rgba<u8>, c2: Rgba<u8>, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    Rgba([
        (c1[0] as f32 * (1.0 - t) + c2[0] as f32 * t).round() as u8,
        (c1[1] as f32 * (1.0 - t) + c2[1] as f32 * t).round() as u8,
        (c1[2] as f32 * (1.0 - t) + c2[2] as f32 * t).round() as u8,
        (c1[3] as f32 * (1.0 - t) + c2[3] as f32 * t).round() as u8,
    ])
}

/// Check if point `(x, y)` is inside a rounded rectangle with local-space
/// dimensions `(w, h)` and corner radius `r`.
///
/// The rectangle spans from `(0, 0)` to `(w, h)` with quarter-circle arcs
/// of radius `r` at each corner. The radius is clamped to `[0, min(w/2, h/2)]`
/// to prevent degenerate geometry.
///
/// This is implemented as `distance_to_rounded_rect_edge(...) >= 0.0` to guarantee
/// exact consistency between the inside test and the SDF.
#[inline]
pub fn inside_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    distance_to_rounded_rect_edge(x, y, w, h, r) >= 0.0
}

/// Compute the signed distance from point `(x, y)` to the edge of a rounded
/// rectangle with local-space dimensions `(w, h)` and corner radius `r`.
///
/// **Positive** values mean the point is inside the rectangle;
/// **negative** values mean it is outside.
///
/// This implements the standard Inigo Quilez rounded rectangle SDF, which
/// produces true Euclidean signed distances in all regions. This ensures:
/// - `inside_rounded_rect(x, y, w, h, r) == (sdf >= 0.0)` for all points
/// - Consistent anti-aliasing in all directions when used with `smoothstep`
///
/// The radius is clamped to `[0, min(w/2, h/2)]`.
/// Returns `-INFINITY` for degenerate rectangles (`w <= 0` or `h <= 0`).
#[inline]
pub fn distance_to_rounded_rect_edge(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    if w <= 0.0 || h <= 0.0 {
        return -f32::INFINITY;
    }

    let r = r.max(0.0).min(w * 0.5).min(h * 0.5);

    // IQ's rounded rect SDF: https://iquilezles.org/articles/distfunctions2d/
    // The formula returns positive outside, negative inside.
    // We negate to match our convention: positive inside, negative outside.
    // q = abs(p - center) - half_size + r
    let qx = (x - w * 0.5).abs() - (w * 0.5 - r);
    let qy = (y - h * 0.5).abs() - (h * 0.5 - r);

    // Outside: Euclidean distance to the nearest corner arc
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    let outside = (ox * ox + oy * oy).sqrt();
    // Inside: distance to the nearest flat edge (negative when inside)
    let inside = qx.max(qy).min(0.0);

    r - (outside + inside)
}

/// Hermite smoothstep interpolation.
///
/// Returns `0.0` when `x <= edge0`, `1.0` when `x >= edge1`,
/// and a smooth S-curve in between: `3t² - 2t³` where `t = (x - edge0) / (edge1 - edge0)`.
///
/// This is the standard GLSL `smoothstep` function, useful for anti-aliasing
/// transitions where linear interpolation would produce visible banding.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Calculate a blur/transition factor based on distance from a center region.
///
/// Returns a value from `0.0` (fully sharp/inside) to `1.0` (fully blurred/outside):
/// - Within `focus_radius`: `0.0` (sharp)
/// - In the feather transition zone: smooth `0.0 → 1.0` (smoothstep)
/// - Beyond `focus_radius + feather`: `1.0` (fully blurred)
///
/// Useful for depth-of-field effects, radial fades, or any distance-based
/// transition with a sharp inner region and smooth falloff.
#[inline]
pub fn blur_factor(dist: f32, focus_radius: f32, feather: f32) -> f32 {
    if dist <= focus_radius {
        0.0
    } else if feather <= 0.0 || dist >= focus_radius + feather {
        1.0
    } else {
        let t = (dist - focus_radius) / feather;
        t * t * (3.0 - 2.0 * t)
    }
}

/// Apply horizontal box blur pass on RGBA pixel data.
///
/// Each output pixel is the average of all pixels within `[x - half_box, x + half_box]`
/// in the horizontal direction. Parallelized across rows using rayon.
///
/// - `src`: Raw RGBA pixel data (4 bytes per pixel, row-major)
/// - `width`, `height`: Image dimensions
/// - `half_box`: Half-width of the box kernel (blur radius)
pub fn box_blur_horizontal(src: &[u8], width: u32, height: u32, half_box: usize) -> Vec<u8> {
    use rayon::prelude::*;

    let w = width as usize;

    let rows: Vec<Vec<[u8; 4]>> = (0..height as usize)
        .into_par_iter()
        .map(|y| {
            let row_offset = y * w * 4;
            (0..w)
                .map(|x| {
                    let x0 = x.saturating_sub(half_box);
                    let x1 = (x + half_box).min(w - 1);
                    let count = (x1 - x0 + 1) as f32;

                    let mut r = 0.0f32;
                    let mut g = 0.0f32;
                    let mut b = 0.0f32;
                    let mut a = 0.0f32;

                    for sx in x0..=x1 {
                        let off = row_offset + sx * 4;
                        r += src[off] as f32;
                        g += src[off + 1] as f32;
                        b += src[off + 2] as f32;
                        a += src[off + 3] as f32;
                    }

                    [
                        (r / count) as u8,
                        (g / count) as u8,
                        (b / count) as u8,
                        (a / count) as u8,
                    ]
                })
                .collect()
        })
        .collect();

    rows.into_iter().flatten().flatten().collect()
}

/// Apply vertical box blur pass on RGBA pixel data.
///
/// Each output pixel is the average of all pixels within `[y - half_box, y + half_box]`
/// in the vertical direction. Parallelized across rows using rayon.
///
/// - `src`: Raw RGBA pixel data (4 bytes per pixel, row-major)
/// - `width`, `height`: Image dimensions
/// - `half_box`: Half-height of the box kernel (blur radius)
pub fn box_blur_vertical(src: &[u8], width: u32, height: u32, half_box: usize) -> Vec<u8> {
    use rayon::prelude::*;

    let w = width as usize;

    let rows: Vec<Vec<[u8; 4]>> = (0..height as usize)
        .into_par_iter()
        .map(|y| {
            (0..w)
                .map(|x| {
                    let y0 = y.saturating_sub(half_box);
                    let y1 = (y + half_box).min(height as usize - 1);
                    let count = (y1 - y0 + 1) as f32;

                    let mut r = 0.0f32;
                    let mut g = 0.0f32;
                    let mut b = 0.0f32;
                    let mut a = 0.0f32;

                    for sy in y0..=y1 {
                        let off = sy * w * 4 + x * 4;
                        r += src[off] as f32;
                        g += src[off + 1] as f32;
                        b += src[off + 2] as f32;
                        a += src[off + 3] as f32;
                    }

                    [
                        (r / count) as u8,
                        (g / count) as u8,
                        (b / count) as u8,
                        (a / count) as u8,
                    ]
                })
                .collect()
        })
        .collect();

    rows.into_iter().flatten().flatten().collect()
}

/// Apply separable box blur to an RGBA image.
///
/// Multiple passes approximate Gaussian/disc shape:
/// - 3 passes ≈ Gaussian blur
/// - 2 passes ≈ disc/bokeh shape
///
/// This implementation uses 3 passes (horizontal → vertical → horizontal)
/// with `half_box = blur_radius` per pass, which produces an effective
/// sigma ≈ `blur_radius * sqrt(passes/3)`.
///
/// - `buffer`: Source image (cloned internally, not modified)
/// - `blur_radius`: Blur intensity in pixels; `half_box` per pass = this value
/// - Returns: A new blurred image, or a clone of the input if `blur_radius == 0`
pub fn apply_box_blur(buffer: &RgbaImage, blur_radius: u32) -> RgbaImage {
    let width = buffer.width();
    let height = buffer.height();

    if blur_radius == 0 {
        return buffer.clone();
    }

    let half_box = blur_radius as usize;

    let src = buffer.as_raw().clone();
    let pass1 = box_blur_horizontal(&src, width, height, half_box);
    let pass2 = box_blur_vertical(&pass1, width, height, half_box);
    let pass3 = box_blur_horizontal(&pass2, width, height, half_box);

    RgbaImage::from_raw(width, height, pass3).expect("Buffer size matches image dimensions")
}

/// Blend a color onto a pixel in-place using standard alpha compositing ("over" operation).
///
/// This is the low-level building block for compositing operations.
/// `src_alpha` is the source opacity in `[0, 1]`, already incorporating the color's alpha.
///
/// Uses Porter-Duff "over" with non-premultiplied (straight) alpha:
/// the output is divided by the composite alpha to produce a straight-alpha result.
#[inline]
pub fn blend_pixel_inplace(pixel: &mut Rgba<u8>, color: Rgba<u8>, src_alpha: f32) {
    if src_alpha <= 0.0 {
        return;
    }

    let dst_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha > 0.0 {
        pixel[0] = ((color[0] as f32 * src_alpha
            + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[1] = ((color[1] as f32 * src_alpha
            + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[2] = ((color[2] as f32 * src_alpha
            + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[3] = (out_alpha * 255.0) as u8;
    }
}

/// Dilate an alpha mask by the given radius using max filter within a circular kernel.
///
/// Each output pixel takes the maximum alpha value found within radius `radius` in the input.
/// This produces a clean, solid expanded shape without blur from overlapping
/// anti-aliased renders — ideal for generating crisp text outlines.
///
/// - `mask`: Flat alpha mask buffer, one byte per pixel, row-major
/// - `width`, `height`: Mask dimensions
/// - `radius`: Dilation radius in pixels
pub fn dilate_mask(mask: &[u8], width: u32, height: u32, radius: u32) -> Vec<u8> {
    let mut dilated = vec![0u8; mask.len()];
    let r = radius as i32;
    let r_sq = r * r;
    let w = width as i32;
    let h = height as i32;

    for y in 0..h {
        for x in 0..w {
            let mut max_alpha = 0u8;
            'search: for dy in -r..=r {
                let sy = y + dy;
                if sy < 0 || sy >= h {
                    continue;
                }
                for dx in -r..=r {
                    if dx * dx + dy * dy > r_sq {
                        continue;
                    }
                    let sx = x + dx;
                    if sx < 0 || sx >= w {
                        continue;
                    }
                    let off = (sy as u32 * width + sx as u32) as usize;
                    max_alpha = max_alpha.max(mask[off]);
                    if max_alpha == 255 {
                        break 'search;
                    }
                }
            }
            dilated[(y as u32 * width + x as u32) as usize] = max_alpha;
        }
    }

    dilated
}

/// Fill image pixels from an alpha mask with a solid color, using standard alpha blending.
///
/// This is a single-pass operation — no overlapping renders, so the result is crisp.
/// Each pixel's alpha is taken from the corresponding mask value; the color's own
/// alpha channel is ignored (mask alpha is used instead).
///
/// - `img`: Target RGBA image (must have `width * height` pixels)
/// - `mask`: Flat alpha mask, one byte per pixel, same dimensions as `img`
/// - `color`: Fill color (RGB channels used; alpha overridden by mask)
pub fn fill_from_mask(img: &mut RgbaImage, mask: &[u8], color: Rgba<u8>) {
    for (pixel, &alpha) in img.pixels_mut().zip(mask.iter()) {
        if alpha == 0 {
            continue;
        }
        let src_alpha = alpha as f32 / 255.0;
        let dst_alpha = pixel[3] as f32 / 255.0;
        let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
        if out_alpha > 0.0 {
            pixel[0] = ((color[0] as f32 * src_alpha
                + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                / out_alpha) as u8;
            pixel[1] = ((color[1] as f32 * src_alpha
                + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                / out_alpha) as u8;
            pixel[2] = ((color[2] as f32 * src_alpha
                + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                / out_alpha) as u8;
            pixel[3] = (out_alpha * 255.0) as u8;
        }
    }
}

/// Composite a source image onto a destination image at the given offset.
///
/// Iterates over all source pixels and alpha-blends them onto the destination.
/// Pixels with zero alpha are skipped. Out-of-bounds pixels are clipped.
///
/// - `dst`: Destination image (modified in-place)
/// - `src`: Source image (read-only)
/// - `x`, `y`: Top-left offset of the source image on the destination
pub fn composite_image(dst: &mut RgbaImage, src: &RgbaImage, x: i32, y: i32) {
    let dst_w = dst.width() as i32;
    let dst_h = dst.height() as i32;

    for (sy, row) in src.enumerate_rows() {
        let dy = y + sy as i32;
        if dy < 0 || dy >= dst_h {
            continue;
        }
        for (sx, _, pixel) in row {
            let dx = x + sx as i32;
            if dx < 0 || dx >= dst_w {
                continue;
            }
            let src_alpha = pixel[3] as f32 / 255.0;
            if src_alpha <= 0.0 {
                continue;
            }
            let dst_pixel = dst.get_pixel_mut(dx as u32, dy as u32);
            blend_pixel_inplace(dst_pixel, *pixel, src_alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_pixel_transparent() {
        let mut buf = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        blend_pixel(&mut buf, 5, 5, &Rgba([255, 0, 0, 255]), 1.0);
        let p = buf.get_pixel(5, 5);
        assert_eq!(p[0], 255);
        assert_eq!(p[3], 255);
    }

    #[test]
    fn test_blend_pixel_semi_transparent() {
        let mut buf = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        blend_pixel(&mut buf, 5, 5, &Rgba([255, 255, 255, 255]), 0.5);
        let p = buf.get_pixel(5, 5);
        assert!(p[3] > 0 && p[3] < 255);
    }

    #[test]
    fn test_blend_pixel_rounding() {
        // Ensure rounding is correct: 0.999999 * 255 should round to 255, not truncate to 254
        let mut buf = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        blend_pixel(&mut buf, 0, 0, &Rgba([255, 255, 255, 255]), 1.0);
        let p = buf.get_pixel(0, 0);
        assert_eq!(p[3], 255);
    }

    #[test]
    fn test_blend_pixel_opacity_clamp() {
        // opacity > 1.0 should be clamped, not overflow
        let mut buf = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        blend_pixel(&mut buf, 0, 0, &Rgba([255, 0, 0, 255]), 2.0);
        let p = buf.get_pixel(0, 0);
        assert_eq!(p[0], 255);
        assert_eq!(p[3], 255);

        // opacity < 0 should produce no change
        let mut buf2 = RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 255]));
        blend_pixel(&mut buf2, 0, 0, &Rgba([255, 0, 0, 255]), -1.0);
        let p2 = buf2.get_pixel(0, 0);
        assert_eq!(p2[0], 100); // unchanged
    }

    #[test]
    fn test_blend_colors() {
        let c = blend_colors(Rgba([0, 0, 0, 255]), Rgba([255, 255, 255, 255]), 0.5);
        assert!(c[0] > 120 && c[0] < 135);
    }

    #[test]
    fn test_inside_rounded_rect_sharp() {
        // Sharp corners (r=0): equivalent to a simple rect test
        assert!(inside_rounded_rect(5.0, 5.0, 10.0, 10.0, 0.0));
        assert!(!inside_rounded_rect(15.0, 5.0, 10.0, 10.0, 0.0));
    }

    #[test]
    fn test_inside_rounded_rect_rounded() {
        // Point inside the rounded corner arc
        assert!(inside_rounded_rect(3.0, 3.0, 20.0, 20.0, 5.0));
        // Point in the corner but outside the arc
        assert!(!inside_rounded_rect(1.0, 1.0, 20.0, 20.0, 5.0));
    }

    #[test]
    fn test_inside_rounded_rect_radius_clamp() {
        // Radius larger than half the width should be clamped
        // With r=20, w=10, h=10 → r clamped to 5 → becomes a circle
        assert!(inside_rounded_rect(5.0, 5.0, 10.0, 10.0, 20.0));
        // Point outside the clamped circle
        assert!(!inside_rounded_rect(0.0, 0.0, 10.0, 10.0, 20.0));
    }

    #[test]
    fn test_inside_rounded_rect_negative_radius() {
        // Negative radius should be clamped to 0 → sharp rectangle
        assert!(inside_rounded_rect(5.0, 5.0, 10.0, 10.0, -5.0));
        assert!(!inside_rounded_rect(-1.0, 5.0, 10.0, 10.0, -5.0));
    }

    #[test]
    fn test_inside_rounded_rect_degenerate() {
        // Zero or negative dimensions → nothing is inside
        assert!(!inside_rounded_rect(0.0, 0.0, 0.0, 10.0, 0.0));
        assert!(!inside_rounded_rect(0.0, 0.0, 10.0, 0.0, 0.0));
        assert!(!inside_rounded_rect(0.0, 0.0, -5.0, 10.0, 0.0));
    }

    #[test]
    fn test_distance_to_rounded_rect_edge() {
        // Center of the rectangle: distance to nearest edge
        let d = distance_to_rounded_rect_edge(50.0, 50.0, 100.0, 100.0, 0.0);
        assert!((d - 50.0).abs() < 0.1);

        // Near the left edge
        let d2 = distance_to_rounded_rect_edge(2.0, 50.0, 100.0, 100.0, 0.0);
        assert!((d2 - 2.0).abs() < 0.1);

        // Outside the rectangle
        let d3 = distance_to_rounded_rect_edge(-3.0, 50.0, 100.0, 100.0, 0.0);
        assert!(d3 < 0.0);
    }

    #[test]
    fn test_distance_sdf_consistency() {
        // SDF should be consistent with inside_rounded_rect:
        // inside == (sdf >= 0) for all points
        let w = 100.0;
        let h = 60.0;
        let r = 15.0;

        // Test a grid of points
        for xi in -10i32..110 {
            for yi in -10i32..70 {
                let x = xi as f32;
                let y = yi as f32;
                let inside = inside_rounded_rect(x, y, w, h, r);
                let sdf = distance_to_rounded_rect_edge(x, y, w, h, r);
                // inside_rounded_rect is now defined as sdf >= 0, so they must agree exactly
                if inside {
                    assert!(sdf >= 0.0, "inside but sdf={sdf} at ({x},{y})");
                } else {
                    assert!(sdf < 0.0, "outside but sdf={sdf} at ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn test_distance_sdf_euclidean_outside_corner() {
        // Point outside a corner should have true Euclidean distance
        // Rect: (0,0)-(100,100), r=20. Corner center at (20,20).
        // Point at (-3,-3): distance to corner center = sqrt(23²+23²) ≈ 32.53
        // SDF = -(32.53 - 20) ≈ -12.53 (negative, outside)
        let sdf = distance_to_rounded_rect_edge(-3.0, -3.0, 100.0, 100.0, 20.0);
        let expected = (23.0_f32 * 23.0 + 23.0 * 23.0).sqrt() - 20.0;
        assert!(
            (sdf - (-expected)).abs() < 0.1,
            "sdf={sdf}, expected={}",
            -expected
        );
        assert!(sdf < 0.0, "should be outside");
    }

    #[test]
    fn test_distance_sdf_euclidean_outside_edge() {
        // Point outside the left edge (not in corner region)
        let sdf = distance_to_rounded_rect_edge(-3.0, 50.0, 100.0, 100.0, 20.0);
        assert!((sdf - (-3.0)).abs() < 0.1, "sdf={sdf}, expected -3.0");
    }

    #[test]
    fn test_distance_sdf_inside_corner() {
        // Point inside near a corner: should be positive
        // Rect (0,0)-(100,100), r=20. Corner center at (20,20).
        // Point (10,10): distance to corner center = sqrt(10²+10²) ≈ 14.14 < r=20, so inside corner arc.
        let sdf = distance_to_rounded_rect_edge(10.0, 10.0, 100.0, 100.0, 20.0);
        assert!(sdf > 0.0, "should be inside, sdf={sdf}");
    }

    #[test]
    fn test_distance_radius_clamp() {
        // Radius larger than half the size should be clamped
        // With r=60, w=100, h=100 → r clamped to 50 → becomes a circle
        let sdf_center = distance_to_rounded_rect_edge(50.0, 50.0, 100.0, 100.0, 60.0);
        // Center of a circle with r=50: distance = 50
        assert!((sdf_center - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_distance_negative_radius() {
        // Negative radius should be clamped to 0 → sharp rectangle
        let sdf_inside = distance_to_rounded_rect_edge(5.0, 5.0, 10.0, 10.0, -5.0);
        assert!(sdf_inside > 0.0, "should be inside sharp rect");
        let sdf_outside = distance_to_rounded_rect_edge(-1.0, 5.0, 10.0, 10.0, -5.0);
        assert!(sdf_outside < 0.0, "should be outside sharp rect");
    }

    #[test]
    fn test_distance_degenerate() {
        // Zero or negative dimensions → -INFINITY
        let d1 = distance_to_rounded_rect_edge(5.0, 5.0, 0.0, 10.0, 0.0);
        assert!(d1.is_infinite() && d1 < 0.0);
        let d2 = distance_to_rounded_rect_edge(5.0, 5.0, -5.0, 10.0, 0.0);
        assert!(d2.is_infinite() && d2 < 0.0);
    }

    #[test]
    fn test_smoothstep() {
        assert!((smoothstep(0.0, 1.0, -0.5) - 0.0).abs() < 0.001);
        assert!((smoothstep(0.0, 1.0, 0.0) - 0.0).abs() < 0.001);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 0.001);
        assert!((smoothstep(0.0, 1.0, 1.0) - 1.0).abs() < 0.001);
        assert!((smoothstep(0.0, 1.0, 1.5) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_blur_factor_sharp_inside_focus() {
        assert_eq!(blur_factor(50.0, 100.0, 30.0), 0.0);
        assert_eq!(blur_factor(0.0, 100.0, 30.0), 0.0);
        assert_eq!(blur_factor(99.9, 100.0, 30.0), 0.0);
    }

    #[test]
    fn test_blur_factor_fully_blurred_outside() {
        assert_eq!(blur_factor(131.0, 100.0, 30.0), 1.0);
        assert_eq!(blur_factor(200.0, 100.0, 30.0), 1.0);
    }

    #[test]
    fn test_blur_factor_transition_zone() {
        let factor = blur_factor(115.0, 100.0, 30.0);
        assert!(factor > 0.0 && factor < 1.0);
        // Midpoint should be around 0.5 (smoothstep at t=0.5 = 0.5)
        assert!((factor - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_blur_factor_no_feather() {
        assert_eq!(blur_factor(99.0, 100.0, 0.0), 0.0);
        assert_eq!(blur_factor(101.0, 100.0, 0.0), 1.0);
    }

    #[test]
    fn test_box_blur_zero_radius() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([128, 64, 32, 255]));
        let result = apply_box_blur(&img, 0);
        // Zero radius should return a clone
        assert_eq!(result.as_raw(), img.as_raw());
    }

    #[test]
    fn test_box_blur_uniform_image() {
        // Uniform image should remain unchanged after blur
        let img = RgbaImage::from_pixel(10, 10, Rgba([100, 150, 200, 255]));
        let result = apply_box_blur(&img, 3);
        for pixel in result.pixels() {
            assert_eq!(pixel[0], 100);
            assert_eq!(pixel[1], 150);
            assert_eq!(pixel[2], 200);
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn test_blend_pixel_inplace_opaque() {
        let mut pixel = Rgba([0, 0, 0, 0]);
        blend_pixel_inplace(&mut pixel, Rgba([255, 0, 0, 255]), 1.0);
        assert_eq!(pixel[0], 255);
        assert_eq!(pixel[3], 255);
    }

    #[test]
    fn test_blend_pixel_inplace_zero_alpha() {
        let mut pixel = Rgba([100, 100, 100, 255]);
        blend_pixel_inplace(&mut pixel, Rgba([255, 0, 0, 255]), 0.0);
        assert_eq!(pixel[0], 100); // unchanged
    }

    #[test]
    fn test_dilate_mask_single_pixel() {
        // Single bright pixel in a 5x5 mask, dilate by 1 (circular kernel)
        let mut mask = vec![0u8; 25];
        mask[12] = 255; // center pixel (2,2)
        let dilated = dilate_mask(&mask, 5, 5, 1);
        // Center and 4-connected neighbors should be 255 (dx²+dy² <= 1)
        let expected_bright: Vec<(i32, i32)> = vec![(2, 2), (1, 2), (3, 2), (2, 1), (2, 3)];
        for (x, y) in &expected_bright {
            let idx = (y * 5 + x) as usize;
            assert_eq!(dilated[idx], 255, "pixel at ({},{}) should be 255", x, y);
        }
        // Diagonal neighbors should remain 0 (dx²+dy² = 2 > 1)
        assert_eq!(dilated[(1 * 5 + 1) as usize], 0, "pixel at (1,1) should be 0");
        assert_eq!(dilated[(1 * 5 + 3) as usize], 0, "pixel at (3,1) should be 0");
        assert_eq!(dilated[(3 * 5 + 1) as usize], 0, "pixel at (1,3) should be 0");
        assert_eq!(dilated[(3 * 5 + 3) as usize], 0, "pixel at (3,3) should be 0");
        // Corner pixels should remain 0
        assert_eq!(dilated[0], 0);
        assert_eq!(dilated[4], 0);
        assert_eq!(dilated[20], 0);
        assert_eq!(dilated[24], 0);
    }

    #[test]
    fn test_dilate_mask_zero_radius() {
        let mask = vec![128u8; 16];
        let dilated = dilate_mask(&mask, 4, 4, 0);
        assert_eq!(dilated, mask); // no change with radius 0
    }

    #[test]
    fn test_fill_from_mask_opaque() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        let mask = vec![255u8; 16];
        fill_from_mask(&mut img, &mask, Rgba([255, 0, 0, 255]));
        for pixel in img.pixels() {
            assert_eq!(pixel[0], 255);
            assert_eq!(pixel[1], 0);
            assert_eq!(pixel[2], 0);
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn test_fill_from_mask_zero_alpha() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([100, 100, 100, 255]));
        let mask = vec![0u8; 16];
        fill_from_mask(&mut img, &mask, Rgba([255, 0, 0, 255]));
        // All pixels should remain unchanged
        for pixel in img.pixels() {
            assert_eq!(pixel[0], 100);
        }
    }

    #[test]
    fn test_composite_image_basic() {
        let mut dst = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        let src = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255]));
        composite_image(&mut dst, &src, 3, 3);
        // Pixel at (5,5) should be red
        let p = dst.get_pixel(5, 5);
        assert_eq!(p[0], 255);
        assert_eq!(p[3], 255);
        // Pixel at (0,0) should still be transparent
        let p0 = dst.get_pixel(0, 0);
        assert_eq!(p0[3], 0);
    }

    #[test]
    fn test_composite_image_clipping() {
        let mut dst = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        let src = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255]));
        // Source extends beyond destination bounds
        composite_image(&mut dst, &src, 8, 8);
        // Only the overlapping part should be drawn
        let p = dst.get_pixel(9, 9);
        assert_eq!(p[0], 255);
        // Out-of-bounds part should not crash
    }
}
