// Genie Effect filter — the classic macOS window minimization animation.
//
// When a window is minimized on macOS, it appears to be "sucked" into the Dock
// icon like a genie being pulled into a lamp. The image shrinks from full-size
// down to a tiny point at the anchor (suck-in), or expands from a tiny point
// up to full-size (pop-out), with a funnel distortion that makes the bottom
// narrow faster than the top.
//
// Key visual behavior:
//   Suck-in:  Full rectangle → shrinks → tiny funnel → disappears into anchor point
//   Pop-out:  Tiny point at anchor → grows → funnel expands → full rectangle

use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{EasingFunction, EffectPosition, VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use std::time::Duration;

/// Anchor position at the bottom edge where the image gets sucked into (the "Dock" position).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum GenieAnchor {
    #[default]
    BottomCenter = 0,
    BottomLeft = 1,
    BottomRight = 2,
}

impl GenieAnchor {
    /// Returns the normalized X position (0.0-1.0) for this anchor.
    pub fn anchor_x(&self) -> f32 {
        match self {
            GenieAnchor::BottomLeft => 0.0,
            GenieAnchor::BottomCenter => 0.5,
            GenieAnchor::BottomRight => 1.0,
        }
    }
}

#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[serde(default)]
#[non_exhaustive]
pub struct GenieFilter {
    /// Whether this filter acts on the start or end of the clip.
    /// Start = "pop out from Dock" (small → big)
    /// End   = "sucked into Dock"  (big → small)
    #[derivative(Default(value = "EffectPosition::Start"))]
    pub position: EffectPosition,

    /// Duration of the contraction/expansion animation.
    #[derivative(Default(value = "Duration::from_secs_f32(0.8)"))]
    pub duration: Duration,

    /// Anchor point at the bottom edge — where the image gets sucked into / pops out from.
    #[derivative(Default(value = "GenieAnchor::BottomCenter"))]
    pub anchor: GenieAnchor,

    /// Funnel curve power (1.0-4.0). Controls the shape of the narrowing.
    /// 1.0 = straight-line trapezoid,
    /// 2.0 = parabolic funnel (classic macOS Genie),
    /// 3.0+ = more dramatic bottom pinch.
    #[derivative(Default(value = "2.0"))]
    pub funnel_power: f32,

    /// Shadow intensity along funnel edges (0.0-1.0).
    #[derivative(Default(value = "0.3"))]
    pub shadow: f32,

    /// Easing function for the main contraction/expansion motion.
    #[derivative(Default(value = "EasingFunction::EaseOut"))]
    pub easing: EasingFunction,
}

impl GenieFilter {
    pub const NAME: &'static str = "genie";

    pub fn new(
        position: EffectPosition,
        duration: Duration,
        anchor: GenieAnchor,
        funnel_power: f32,
    ) -> Self {
        Self {
            position,
            duration,
            anchor,
            funnel_power: funnel_power.clamp(1.0, 4.0),
            shadow: 0.3,
            easing: EasingFunction::EaseOut,
        }
    }

    /// Bilinear interpolation sampling from source image.
    fn sample_bilinear(source: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
        let width = source.width();
        let height = source.height();

        let x = x.clamp(0.0, (width - 1) as f32);
        let y = y.clamp(0.0, (height - 1) as f32);

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);

        let dx = x - x0 as f32;
        let dy = y - y0 as f32;

        let p00 = source.get_pixel(x0, y0);
        let p01 = source.get_pixel(x1, y0);
        let p10 = source.get_pixel(x0, y1);
        let p11 = source.get_pixel(x1, y1);

        fn interpolate(v00: u8, v01: u8, v10: u8, v11: u8, dx: f32, dy: f32) -> u8 {
            let v0 = v00 as f32 * (1.0 - dx) + v01 as f32 * dx;
            let v1 = v10 as f32 * (1.0 - dx) + v11 as f32 * dx;
            ((v0 * (1.0 - dy) + v1 * dy).clamp(0.0, 255.0)) as u8
        }

        Rgba([
            interpolate(p00[0], p01[0], p10[0], p11[0], dx, dy),
            interpolate(p00[1], p01[1], p10[1], p11[1], dx, dy),
            interpolate(p00[2], p01[2], p10[2], p11[2], dx, dy),
            interpolate(p00[3], p01[3], p10[3], p11[3], dx, dy),
        ])
    }

    fn apply_easing(&self, t: f32) -> f32 {
        match self.easing {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => t * (2.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }

    /// Apply the Genie Effect distortion.
    ///
    /// The image shrinks from full-size to a tiny funnel at the anchor point (suck-in),
    /// or expands from a tiny funnel to full-size (pop-out).
    ///
    /// Core algorithm:
    ///   `scale` controls overall size: 1.0 = full-size, 0.0 = collapsed to a point.
    ///   The funnel distortion makes bottom rows narrower than top rows within the
    ///   already-scaled image, so as it shrinks the bottom pinches into a point first.
    fn apply_genie(
        &self,
        buffer: &mut RgbaImage,
        canvas_width: u32,
        canvas_height: u32,
        frame_time_offset: Duration,
        total_duration: Duration,
    ) -> Result<()> {
        // Ensure buffer is canvas-sized
        if buffer.width() != canvas_width || buffer.height() != canvas_height {
            let mut canvas = RgbaImage::new(canvas_width, canvas_height);
            let x = (canvas_width.saturating_sub(buffer.width())) / 2;
            let y = (canvas_height.saturating_sub(buffer.height())) / 2;
            image::imageops::overlay(&mut canvas, buffer, x as i64, y as i64);
            *buffer = canvas;
        }

        // Calculate animation ratio
        // Convention: ratio always goes 0→1 (0 = start of effect, 1 = end of effect)
        let ratio = if self.position == EffectPosition::Start {
            // Pop OUT: tiny funnel expands into full rectangle
            if frame_time_offset > self.duration {
                return Ok(()); // Animation complete
            } else {
                progress_ratio_from_offset(frame_time_offset, self.duration)
            }
        } else {
            // Suck IN: full rectangle shrinks into tiny funnel
            let time_from_end = total_duration.saturating_sub(frame_time_offset);
            if time_from_end > self.duration {
                return Ok(()); // Not yet in animation zone
            } else {
                // time_from_end goes from duration→0, so elapsed = duration - time_from_end goes 0→duration
                progress_ratio_from_offset(self.duration - time_from_end, self.duration)
            }
        };

        let eased = self.apply_easing(ratio);

        // scale: 1.0 = full-size image, 0.0 = collapsed to anchor point
        // Suck-in: starts at 1.0, goes to 0.0 (ratio 0→1 → scale 1→0)
        // Pop-out: starts at 0.0, goes to 1.0 (ratio 0→1 → scale 0→1)
        let mut scale = if self.position == EffectPosition::Start {
            eased // 0→1
        } else {
            1.0 - eased // 1→0
        };

        scale = scale.clamp(0.0, 1.0);

        // Skip if fully expanded (no effect)
        if scale >= 0.99 {
            return Ok(());
        }

        // Fully collapsed: clear to transparent
        if scale < 0.01 {
            *buffer = RgbaImage::from_pixel(canvas_width, canvas_height, Rgba([0, 0, 0, 0]));
            return Ok(());
        }

        let w = canvas_width as f32;
        let h = canvas_height as f32;
        let power = self.funnel_power;
        let shadow_intensity = self.shadow;

        // Anchor point (bottom of canvas)
        let anchor_x = self.anchor.anchor_x() * w;
        let anchor_y = h; // bottom edge

        let source = buffer.clone();
        let mut result = RgbaImage::new(canvas_width, canvas_height);

        result
            .par_enumerate_pixels_mut()
            .for_each(|(dst_x, dst_y, pixel)| {
                let px = dst_x as f32;
                let py = dst_y as f32;

                // For each destination pixel, compute its position relative to the anchor,
                // then determine if it falls within the scaled+funneled image region,
                // and if so, map back to the source image.

                // Relative position from anchor (anchor is at bottom)
                let rel_y = py - anchor_y; // negative (above anchor)

                // Distance from anchor as fraction of image size
                // rel_y ranges from -h (top) to 0 (bottom/anchor)
                // normalized_y: 0.0 at anchor (bottom), 1.0 at top
                let dist_from_anchor = -rel_y; // positive distance upward
                let normalized_y = dist_from_anchor / h;

                if normalized_y < 0.0 || normalized_y > scale {
                    // This pixel is below the anchor or above the top of the scaled image
                    *pixel = Rgba([0, 0, 0, 0]);
                    return;
                }

                // row_in_image: 0 at anchor, 1 at top of visible image
                let row_in_image = if scale > 0.001 {
                    normalized_y / scale
                } else {
                    1.0 // everything collapsed to a point
                };

                // Funnel factor: at top (row_in_image=1) the image is full width,
                // at bottom (row_in_image=0) the image narrows to a point.
                // The narrowing follows a power curve for the Genie shape.
                let funnel_factor = 1.0 - row_in_image.powf(power);

                // === Horizontal bounds at this row ===
                // The funnel narrows toward the anchor point.
                // At top (row_in_image=1): full-width edges
                // At bottom/anchor (row_in_image=0): edges converge to anchor_x
                //
                // left_edge:  lerp(0, anchor_x, funnel_factor) = anchor_x * funnel_factor
                // right_edge: lerp(w, anchor_x, funnel_factor) = w - (w - anchor_x) * funnel_factor
                //
                // This naturally handles all anchor positions:
                //   BottomLeft  (anchor=0):   left=0, right=w*(1-funnel)  → narrows rightward to x=0
                //   BottomCenter(anchor=w/2):  left=w/2*f, right=w-w/2*f  → narrows symmetrically
                //   BottomRight (anchor=w):    left=w*f, right=w           → narrows leftward to x=w
                let left_dist_from_anchor = anchor_x; // distance from anchor to left edge (x=0)
                let right_dist_from_anchor = w - anchor_x; // distance from anchor to right edge (x=w)

                // Apply scale + funnel: edges move toward anchor as scale shrinks
                // The funnel_factor makes rows near anchor narrower; the 0.95 factor
                // prevents complete collapse to zero width until scale→0.
                let funnel_narrow = funnel_factor * 0.95;
                let left_edge = anchor_x - left_dist_from_anchor * scale * (1.0 - funnel_narrow);
                let right_edge = anchor_x + right_dist_from_anchor * scale * (1.0 - funnel_narrow);

                // Check if this pixel is within the horizontal bounds
                if px < left_edge || px > right_edge {
                    *pixel = Rgba([0, 0, 0, 0]);
                    return;
                }

                // === Map destination → source ===
                // Source Y: linear map from the scaled region to the full image height.
                let src_y = (1.0 - row_in_image) * h;

                // Source X: map from [left_edge, right_edge] to [0, w).
                let src_x = if right_edge - left_edge > 1.0 {
                    (px - left_edge) / (right_edge - left_edge) * w
                } else {
                    anchor_x // collapsed to a point
                };

                // Bounds check
                if src_x < 0.0 || src_x >= w || src_y < 0.0 || src_y >= h {
                    *pixel = Rgba([0, 0, 0, 0]);
                    return;
                }

                let mut sampled = Self::sample_bilinear(&source, src_x, src_y);

                // Shadow along funnel edges
                if shadow_intensity > 0.0 && scale < 0.98 {
                    let dist_to_left = px - left_edge;
                    let dist_to_right = right_edge - px;
                    let edge_dist = dist_to_left.min(dist_to_right);
                    let shadow_width = funnel_factor * scale * 25.0;
                    if shadow_width > 0.5 && edge_dist < shadow_width {
                        let shadow_amt = (1.0 - edge_dist / shadow_width)
                            * shadow_intensity
                            * 0.5
                            * funnel_factor;
                        let darken = 1.0 - shadow_amt.min(1.0);
                        sampled.0[0] = (sampled.0[0] as f32 * darken).clamp(0.0, 255.0) as u8;
                        sampled.0[1] = (sampled.0[1] as f32 * darken).clamp(0.0, 255.0) as u8;
                        sampled.0[2] = (sampled.0[2] as f32 * darken).clamp(0.0, 255.0) as u8;
                    }
                }

                *pixel = sampled;
            });

        *buffer = result;
        Ok(())
    }
}

impl VideoFilter for GenieFilter {
    crate::impl_default_video_filter!(GenieFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let canvas_width = data.config.output_width;
        let canvas_height = data.config.output_height;
        let frame_time_offset = data.relative_timeline_offset;

        let total_duration = data.from_segment.duration;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_genie(
                    buffer,
                    canvas_width,
                    canvas_height,
                    frame_time_offset,
                    total_duration,
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_positions() {
        assert_eq!(GenieAnchor::BottomLeft.anchor_x(), 0.0);
        assert_eq!(GenieAnchor::BottomCenter.anchor_x(), 0.5);
        assert_eq!(GenieAnchor::BottomRight.anchor_x(), 1.0);
    }

    #[test]
    fn test_filter_name() {
        assert_eq!(GenieFilter::NAME, "genie");
    }

    #[test]
    fn test_default_values() {
        let filter = GenieFilter::default();
        assert_eq!(filter.position, EffectPosition::Start);
        assert_eq!(filter.anchor, GenieAnchor::BottomCenter);
        assert!((filter.funnel_power - 2.0).abs() < 0.001);
        assert!((filter.shadow - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_suck_in_shrinks_image() {
        // Suck-in at mid-animation: image should be smaller than full canvas
        let filter = GenieFilter::new(
            EffectPosition::End,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomCenter,
            2.0,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // Segment is 3.0s, suck-in effect last 0.8s (from 2.2s to 3.0s).
        // At frame_time=2.6: time_from_end = 3.0 - 2.6 = 0.4 → ratio ≈ 0.5
        let total_duration = Duration::from_secs_f32(3.0);
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(2.6),
                total_duration,
            )
            .unwrap();
        // Top-left corner should be transparent (image has shrunk)
        let top_left = buffer.get_pixel(0, 0);
        assert_eq!(
            top_left[3], 0,
            "Top-left should be transparent — image shrank"
        );
        // Pixels near anchor (bottom-center) should still be visible
        let near_anchor = buffer.get_pixel(50, 95);
        assert_eq!(near_anchor[3], 255, "Near anchor should still be visible");
    }

    #[test]
    fn test_suck_in_fully_collapsed() {
        // At end of suck-in, almost everything should be transparent
        let filter = GenieFilter::new(
            EffectPosition::End,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomCenter,
            2.0,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // Segment is 3.0s. Near end: frame_time=2.98 → time_from_end=0.02 → ratio≈0.975
        let total_duration = Duration::from_secs_f32(3.0);
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(2.98),
                total_duration,
            )
            .unwrap();
        // Top-left should definitely be transparent
        let top_left = buffer.get_pixel(0, 0);
        assert_eq!(
            top_left[3], 0,
            "Top-left should be transparent when collapsed"
        );
        // Far corners should be transparent
        let bottom_left = buffer.get_pixel(0, 99);
        assert_eq!(
            bottom_left[3], 0,
            "Bottom-left should be transparent when collapsed"
        );
    }

    #[test]
    fn test_pop_out_starts_small() {
        // Pop-out at beginning: image should start tiny
        let filter = GenieFilter::new(
            EffectPosition::Start,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomCenter,
            2.0,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_millis(10),
                Duration::from_secs_f32(3.0),
            )
            .unwrap();
        // Top-left should be transparent (image is tiny near anchor)
        let top_left = buffer.get_pixel(0, 0);
        assert_eq!(
            top_left[3], 0,
            "Top-left should be transparent at pop-out start"
        );
    }

    #[test]
    fn test_no_effect_after_duration() {
        let filter = GenieFilter::new(
            EffectPosition::Start,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomCenter,
            2.0,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(2.0),
                Duration::from_secs_f32(3.0),
            )
            .unwrap();
        for y in 0..100 {
            for x in 0..100 {
                let p = buffer.get_pixel(x, y);
                assert_eq!(p[0], 128, "Pixel should be unchanged after animation");
            }
        }
    }

    #[test]
    fn test_anchor_left() {
        let filter = GenieFilter::new(
            EffectPosition::End,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomLeft,
            2.0,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // Segment is 3.0s. At frame_time=2.6 → mid-animation (time_from_end=0.4)
        let total_duration = Duration::from_secs_f32(3.0);
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(2.6),
                total_duration,
            )
            .unwrap();
        // The funnel narrows toward bottom-left (x=0). Top-right should be transparent
        let top_right = buffer.get_pixel(99, 0);
        assert_eq!(
            top_right[3], 0,
            "Top-right should be transparent when anchor is bottom-left"
        );
    }

    #[test]
    fn test_anchor_left_maps_full_width() {
        // Verify that BottomLeft maps the full source width, not just half.
        // Create a source image where left half is red, right half is blue.
        let mut img = RgbaImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                if x < 50 {
                    img.put_pixel(x, y, Rgba([255, 0, 0, 255])); // red
                } else {
                    img.put_pixel(x, y, Rgba([0, 0, 255, 255])); // blue
                }
            }
        }
        let filter = GenieFilter::new(
            EffectPosition::Start,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomLeft,
            2.0,
        );
        let mut buffer = img.clone();
        // Pop-out at mid-animation: scale≈0.5
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(0.4),
                Duration::from_secs_f32(3.0),
            )
            .unwrap();
        // In the visible funnel region (near bottom-left), both red and blue
        // pixels should appear — meaning the full source width is mapped.
        // Check near bottom where funnel is narrow but still visible.
        // At row near bottom-center, the funnel should contain pixels from
        // both halves of the source image.
        let mut has_red = false;
        let mut has_blue = false;
        for x in 0..50 {
            for y in 80..100 {
                let p = buffer.get_pixel(x, y);
                if p[0] > 200 && p[2] < 50 {
                    has_red = true;
                }
                if p[2] > 200 && p[0] < 50 {
                    has_blue = true;
                }
            }
        }
        assert!(has_red, "Should have red pixels near bottom-left anchor");
        assert!(
            has_blue,
            "Should have blue pixels near bottom-left anchor (full width mapped)"
        );
    }

    #[test]
    fn test_anchor_right_maps_full_width() {
        // Verify that BottomRight maps the full source width, not just half.
        let mut img = RgbaImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                if x < 50 {
                    img.put_pixel(x, y, Rgba([255, 0, 0, 255])); // red
                } else {
                    img.put_pixel(x, y, Rgba([0, 0, 255, 255])); // blue
                }
            }
        }
        let filter = GenieFilter::new(
            EffectPosition::Start,
            Duration::from_secs_f32(0.8),
            GenieAnchor::BottomRight,
            2.0,
        );
        let mut buffer = img.clone();
        // Pop-out at mid-animation: scale≈0.5
        filter
            .apply_genie(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(0.4),
                Duration::from_secs_f32(3.0),
            )
            .unwrap();
        // In the visible funnel region (near bottom-right), both red and blue
        let mut has_red = false;
        let mut has_blue = false;
        for x in 50..100 {
            for y in 80..100 {
                let p = buffer.get_pixel(x, y);
                if p[0] > 200 && p[2] < 50 {
                    has_red = true;
                }
                if p[2] > 200 && p[0] < 50 {
                    has_blue = true;
                }
            }
        }
        assert!(has_red, "Should have red pixels near bottom-right anchor");
        assert!(
            has_blue,
            "Should have blue pixels near bottom-right anchor (full width mapped)"
        );
    }
}
