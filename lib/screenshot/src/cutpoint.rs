use image::RgbaImage;

use crate::template::content_roi;
use crate::util::to_grayscale_vec;

/// Find a flat row near the overlap boundary to use as the cut point.
///
/// Searches rows from the original cut position (row `overlap` in the current frame)
/// upward into the overlap region. Returns the number of rows to shift the cut point
/// (0 = no shift).
///
/// A flat row has low horizontal total variation (adjacent pixels are similar),
/// meaning it is visually uniform — e.g. a solid background color rather than text.
pub fn find_flat_cutpoint(
    frame: &RgbaImage,
    overlap: u32,
    search_radius: u32,
    flatness_threshold: f32,
) -> u32 {
    if overlap == 0 || search_radius == 0 {
        return 0;
    }

    let width = frame.width();
    let height = frame.height();
    let gray = to_grayscale_vec(frame);

    let (roi_x, _, roi_w, _) = content_roi(width, height);
    if roi_w < 2 {
        return 0;
    }

    let radius = search_radius.min(overlap.saturating_sub(1));

    let mut best_delta = 0u32;
    let mut best_flatness = f32::MAX;

    for delta in 0..=radius {
        let row = overlap.saturating_sub(delta);
        if row >= height {
            continue;
        }

        let flatness = row_total_variation(&gray, row, width, roi_x, roi_w);

        if flatness < best_flatness {
            best_flatness = flatness;
            best_delta = delta;
        }

        if flatness < flatness_threshold {
            break;
        }
    }

    best_delta
}

/// Compute average absolute difference between adjacent pixels in a row within a ROI.
///
/// Lower values = more uniform row. Solid background ~0.0, text row = high.
fn row_total_variation(gray: &[f32], row: u32, width: u32, roi_x: u32, roi_w: u32) -> f32 {
    let start = (row as usize) * (width as usize) + roi_x as usize;
    let end = start + roi_w as usize;

    if end > gray.len() || roi_w < 2 {
        return f32::MAX;
    }

    let mut tv = 0.0f32;
    for i in start..end - 1 {
        tv += (gray[i + 1] - gray[i]).abs();
    }

    tv / (roi_w - 1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid_frame(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    fn frame_with_text_row(w: u32, h: u32, text_y: u32) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(w, h, Rgba([245, 245, 245, 255]));
        // Draw a "text" line: alternating dark/light pixels
        for x in 20..w.saturating_sub(20) {
            let val = if (x / 3) % 2 == 0 { 30u8 } else { 245u8 };
            img.put_pixel(x, text_y, Rgba([val, val, val, 255]));
            if text_y + 1 < h {
                img.put_pixel(x, text_y + 1, Rgba([val, val, val, 255]));
            }
        }
        img
    }

    #[test]
    fn flat_cutpoint_solid_background() {
        let frame = solid_frame(400, 400, [245, 245, 245, 255]);
        // Overlap row is solid — no shift needed
        let delta = find_flat_cutpoint(&frame, 200, 30, 3.0);
        assert_eq!(delta, 0, "solid background should need no shift");
    }

    #[test]
    fn flat_cutpoint_text_at_boundary() {
        // Text row at the overlap boundary, flat row at overlap-5
        let frame = frame_with_text_row(400, 400, 200);
        let delta = find_flat_cutpoint(&frame, 200, 30, 3.0);
        assert!(delta > 0, "should shift away from text row, got delta={delta}");
    }

    #[test]
    fn flat_cutpoint_zero_overlap() {
        let frame = solid_frame(400, 400, [245, 245, 245, 255]);
        let delta = find_flat_cutpoint(&frame, 0, 30, 3.0);
        assert_eq!(delta, 0, "zero overlap should return 0");
    }

    #[test]
    fn row_tv_solid() {
        let frame = solid_frame(200, 100, [128, 128, 128, 255]);
        let gray = to_grayscale_vec(&frame);
        let tv = row_total_variation(&gray, 50, 200, 24, 152);
        assert!(tv < 0.01, "solid row TV should be ~0, got {tv}");
    }

    #[test]
    fn row_tv_checkerboard() {
        let mut img = RgbaImage::new(200, 100);
        for y in 0..100 {
            for x in 0..200 {
                let v = if x % 2 == 0 { 0u8 } else { 255u8 };
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        let gray = to_grayscale_vec(&img);
        let tv = row_total_variation(&gray, 50, 200, 24, 152);
        assert!(tv > 100.0, "checkerboard row TV should be very high, got {tv}");
    }
}
