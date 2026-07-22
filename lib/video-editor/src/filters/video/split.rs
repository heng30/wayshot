// Split transition filter — the image splits into two halves that slide apart
// (End position) or come together (Start position).
//
// Visual model:
//   End position (split out):
//     ratio=0:   The image is fully visible, both halves together.
//     ratio→1:   A gap opens at the split line, and the two halves slide apart.
//                Each half moves away from the split line toward the nearest edge
//                and beyond, until both halves are completely off-screen.
//
//   Start position (split in):
//     ratio=0:   Both halves are off-screen, separated by a gap.
//     ratio→1:   The two halves slide toward each other and merge at the split line,
//                forming the complete image.
//
// Supports:
//   - Horizontal split (left/right halves move apart)
//   - Vertical split (top/bottom halves move apart)
//   - Configurable split position (not just center)
//   - Configurable duration and easing
//   - Shadow along the split edge for depth

use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{EasingFunction, EffectPosition, VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage, imageops};
use rayon::prelude::*;
use std::time::Duration;

/// Direction of the split animation.
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
pub enum SplitDirection {
    #[default]
    Horizontal = 0, // 左右分开 (left/right halves move apart)
    Vertical = 1, // 上下分开 (top/bottom halves move apart)
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
pub struct SplitFilter {
    /// Whether this filter acts on the start or end of the clip.
    /// Start = "split in" (halves come together from off-screen)
    /// End   = "split out" (halves slide apart off-screen)
    #[derivative(Default(value = "EffectPosition::Start"))]
    pub position: EffectPosition,

    /// Duration of the split animation.
    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub duration: Duration,

    /// Direction of the split: Horizontal (left/right) or Vertical (top/bottom).
    #[derivative(Default(value = "SplitDirection::Horizontal"))]
    pub direction: SplitDirection,

    /// Position of the split line as a fraction (0.0–1.0).
    /// 0.5 = center, 0.3 = off-center, etc.
    #[derivative(Default(value = "0.5"))]
    pub split_position: f32,

    /// Shadow intensity along the split edge (0.0–1.0).
    #[derivative(Default(value = "0.4"))]
    pub shadow: f32,

    /// Shadow width in pixels.
    #[derivative(Default(value = "20.0"))]
    pub shadow_width: f32,

    /// Easing function for the split animation.
    #[derivative(Default(value = "EasingFunction::EaseInOut"))]
    pub easing: EasingFunction,
}

impl SplitFilter {
    pub const NAME: &'static str = "split";

    pub fn new(position: EffectPosition, duration: Duration, direction: SplitDirection) -> Self {
        Self {
            position,
            duration,
            direction,
            split_position: 0.5,
            shadow: 0.4,
            shadow_width: 20.0,
            easing: EasingFunction::EaseInOut,
        }
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

    fn apply_split(
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
            imageops::overlay(&mut canvas, buffer, x as i64, y as i64);
            *buffer = canvas;
        }

        // Calculate animation ratio based on position
        let ratio = if self.position == EffectPosition::End {
            // Split OUT: at segment end, halves slide apart
            let time_until_end = total_duration.saturating_sub(frame_time_offset);
            if time_until_end > self.duration {
                return Ok(()); // Not yet in animation zone
            }
            // ratio: 0 at start of effect (image intact), 1 at end (fully split apart)
            progress_ratio_from_offset(self.duration - time_until_end, self.duration)
        } else {
            // Split IN: at segment start, halves come together
            if frame_time_offset > self.duration {
                return Ok(()); // Animation complete
            }
            // ratio: 0 at start (fully split apart), 1 at end (image intact)
            progress_ratio_from_offset(frame_time_offset, self.duration)
        };

        let eased = self.apply_easing(ratio);

        // progress: 0 = fully split apart, 1 = fully together
        // For End position: eased goes 0→1, so progress = eased (starts together, ends apart)
        //   but we want: at ratio=0 image is together, at ratio=1 image is apart
        //   so offset_progress = eased (0→1 means increasing separation)
        // For Start position: eased goes 0→1, meaning halves come together
        //   so offset_progress = 1.0 - eased (1→0 means decreasing separation)
        let offset_progress = if self.position == EffectPosition::End {
            eased // 0→1: increasing separation
        } else {
            1.0 - eased // 1→0: decreasing separation (halves coming together)
        };

        let offset_progress = offset_progress.clamp(0.0, 1.0);

        // Skip if no effect
        if offset_progress < 0.001 {
            // Fully together — no split visible
            return Ok(());
        }

        // Fully split apart: both halves are off-screen, clear to transparent
        if offset_progress > 0.995 {
            *buffer = RgbaImage::from_pixel(canvas_width, canvas_height, Rgba([0, 0, 0, 0]));
            return Ok(());
        }

        let w = canvas_width as f32;
        let h = canvas_height as f32;
        let split_pos = self.split_position.clamp(0.05, 0.95);
        let shadow_intensity = self.shadow;
        let shadow_w = self.shadow_width;

        let source = buffer.clone();
        let mut result = RgbaImage::new(canvas_width, canvas_height);

        match self.direction {
            SplitDirection::Horizontal => {
                // Split left/right: left half moves left, right half moves right
                let split_x = split_pos * w; // split line position
                let left_width = split_x; // width of left half
                let right_width = w - split_x; // width of right half

                // Each half needs to move beyond the canvas edge to be fully off-screen.
                // Left half moves left by at least `left_width` pixels (its own width),
                // right half moves right by at least `right_width` pixels.
                // We add a small extra so that at offset_progress=1.0 the half is
                // guaranteed fully beyond the canvas boundary (avoiding sub-pixel residue).
                let left_offset = offset_progress * (left_width + 1.0);
                let right_offset = offset_progress * (right_width + 1.0);

                // After moving:
                //   Left half occupies:  [-left_offset, split_x - left_offset]
                //   Right half occupies: [split_x + right_offset, w + right_offset]
                //   Gap:                 [split_x - left_offset, split_x + right_offset]

                let left_end = split_x - left_offset; // right edge of moved left half
                let right_start = split_x + right_offset; // left edge of moved right half

                result
                    .par_enumerate_pixels_mut()
                    .for_each(|(dst_x, dst_y, pixel)| {
                        let px = dst_x as f32;

                        if px < left_end {
                            // In the moved left half region
                            let src_x = px + left_offset;
                            if src_x >= 0.0 && src_x < w {
                                let orig = source.get_pixel(src_x as u32, dst_y);
                                // Shadow near the split edge (right edge of left half)
                                let dist_from_edge = left_end - px;
                                if dist_from_edge < shadow_w && shadow_intensity > 0.0 {
                                    let shadow_amt = (1.0 - dist_from_edge / shadow_w).powi(2)
                                        * shadow_intensity
                                        * 0.6;
                                    pixel.0 = [
                                        (orig.0[0] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[1] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[2] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        orig.0[3],
                                    ];
                                } else {
                                    *pixel = *orig;
                                }
                            }
                            // else: transparent (left half has moved off-screen to the left)
                        } else if px >= right_start {
                            // In the moved right half region
                            let src_x = px - right_offset;
                            if src_x >= 0.0 && src_x < w {
                                let orig = source.get_pixel(src_x as u32, dst_y);
                                // Shadow near the split edge (left edge of right half)
                                let dist_from_edge = px - right_start;
                                if dist_from_edge < shadow_w && shadow_intensity > 0.0 {
                                    let shadow_amt = (1.0 - dist_from_edge / shadow_w).powi(2)
                                        * shadow_intensity
                                        * 0.6;
                                    pixel.0 = [
                                        (orig.0[0] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[1] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[2] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        orig.0[3],
                                    ];
                                } else {
                                    *pixel = *orig;
                                }
                            }
                            // else: transparent (right half has moved off-screen to the right)
                        }
                        // else: in the gap — transparent (default)
                    });
            }
            SplitDirection::Vertical => {
                // Split top/bottom: top half moves up, bottom half moves down
                let split_y = split_pos * h; // split line position
                let top_height = split_y; // height of top half
                let bottom_height = h - split_y; // height of bottom half

                // Each half needs to move beyond the canvas edge to be fully off-screen,
                // with a small extra to avoid sub-pixel residue.
                let top_offset = offset_progress * (top_height + 1.0);
                let bottom_offset = offset_progress * (bottom_height + 1.0);

                // After moving:
                //   Top half occupies:    [-top_offset, split_y - top_offset]
                //   Bottom half occupies: [split_y + bottom_offset, h + bottom_offset]
                //   Gap:                  [split_y - top_offset, split_y + bottom_offset]

                let top_end = split_y - top_offset; // bottom edge of moved top half
                let bottom_start = split_y + bottom_offset; // top edge of moved bottom half

                result
                    .par_enumerate_pixels_mut()
                    .for_each(|(dst_x, dst_y, pixel)| {
                        let py = dst_y as f32;

                        if py < top_end {
                            // In the moved top half region
                            let src_y = py + top_offset;
                            if src_y >= 0.0 && src_y < h {
                                let orig = source.get_pixel(dst_x, src_y as u32);
                                // Shadow near the split edge (bottom edge of top half)
                                let dist_from_edge = top_end - py;
                                if dist_from_edge < shadow_w && shadow_intensity > 0.0 {
                                    let shadow_amt = (1.0 - dist_from_edge / shadow_w).powi(2)
                                        * shadow_intensity
                                        * 0.6;
                                    pixel.0 = [
                                        (orig.0[0] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[1] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[2] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        orig.0[3],
                                    ];
                                } else {
                                    *pixel = *orig;
                                }
                            }
                            // else: transparent (top half has moved off-screen upward)
                        } else if py >= bottom_start {
                            // In the moved bottom half region
                            let src_y = py - bottom_offset;
                            if src_y >= 0.0 && src_y < h {
                                let orig = source.get_pixel(dst_x, src_y as u32);
                                // Shadow near the split edge (top edge of bottom half)
                                let dist_from_edge = py - bottom_start;
                                if dist_from_edge < shadow_w && shadow_intensity > 0.0 {
                                    let shadow_amt = (1.0 - dist_from_edge / shadow_w).powi(2)
                                        * shadow_intensity
                                        * 0.6;
                                    pixel.0 = [
                                        (orig.0[0] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[1] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        (orig.0[2] as f32 * (1.0 - shadow_amt)).clamp(0.0, 255.0)
                                            as u8,
                                        orig.0[3],
                                    ];
                                } else {
                                    *pixel = *orig;
                                }
                            }
                            // else: transparent (bottom half has moved off-screen downward)
                        }
                        // else: in the gap — transparent (default)
                    });
            }
        }

        *buffer = result;
        Ok(())
    }
}

impl VideoFilter for SplitFilter {
    crate::impl_default_video_filter!(SplitFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false // Operate on image_for_composite stream
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let canvas_width = data.config.output_width;
        let canvas_height = data.config.output_height;
        let frame_time_offset = data.relative_timeline_offset;
        let total_duration = data.from_segment.duration;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_split(
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
    use image::Rgba;

    #[test]
    fn test_filter_name() {
        assert_eq!(SplitFilter::NAME, "split");
    }

    #[test]
    fn test_default_values() {
        let filter = SplitFilter::default();
        assert_eq!(filter.position, EffectPosition::End);
        assert_eq!(filter.direction, SplitDirection::Horizontal);
        assert!((filter.split_position - 0.5).abs() < 0.001);
        assert!((filter.shadow - 0.4).abs() < 0.001);
        assert_eq!(filter.duration, Duration::from_secs(1));
    }

    #[test]
    fn test_setters() {
        let filter = SplitFilter::default()
            .with_direction(SplitDirection::Vertical)
            .with_split_position(0.3)
            .with_shadow(0.8)
            .with_duration(Duration::from_secs(2));

        assert_eq!(filter.direction, SplitDirection::Vertical);
        assert!((filter.split_position - 0.3).abs() < 0.001);
        assert!((filter.shadow - 0.8).abs() < 0.001);
        assert_eq!(filter.duration, Duration::from_secs(2));
    }

    #[test]
    fn test_easing() {
        let filter = SplitFilter::default();
        assert!((filter.apply_easing(0.0) - 0.0).abs() < 0.001);
        assert!((filter.apply_easing(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_no_effect_before_animation_zone() {
        // At the start of a long segment, End-position filter should have no effect
        let filter = SplitFilter::default();
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // total_duration=10s, frame_time=0s → time_until_end=10s > 1s duration → no effect
        filter
            .apply_split(
                &mut buffer,
                100,
                100,
                Duration::from_secs(0),
                Duration::from_secs(10),
            )
            .unwrap();
        // Image should be unchanged
        for y in 0..100 {
            for x in 0..100 {
                let p = buffer.get_pixel(x, y);
                assert_eq!(p[0], 128, "Pixel should be unchanged before animation zone");
            }
        }
    }

    #[test]
    fn test_horizontal_split_creates_gap() {
        // At mid-animation, there should be a transparent gap in the center
        let filter = SplitFilter::new(
            EffectPosition::End,
            Duration::from_secs(1),
            SplitDirection::Horizontal,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // total_duration=1s, frame_time=0.5s → time_until_end=0.5s → ratio=0.5
        filter
            .apply_split(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(0.5),
                Duration::from_secs(1),
            )
            .unwrap();
        // Center pixel should be transparent (in the gap)
        let center = buffer.get_pixel(50, 50);
        assert_eq!(
            center[3], 0,
            "Center pixel should be transparent in the gap"
        );
    }

    #[test]
    fn test_vertical_split_creates_gap() {
        // At mid-animation, there should be a transparent gap in the center
        let filter = SplitFilter::new(
            EffectPosition::End,
            Duration::from_secs(1),
            SplitDirection::Vertical,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // total_duration=1s, frame_time=0.5s → time_until_end=0.5s → ratio=0.5
        filter
            .apply_split(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(0.5),
                Duration::from_secs(1),
            )
            .unwrap();
        // Center pixel should be transparent (in the gap)
        let center = buffer.get_pixel(50, 50);
        assert_eq!(
            center[3], 0,
            "Center pixel should be transparent in vertical gap"
        );
    }

    #[test]
    fn test_split_in_brings_halves_together() {
        // Start position: halves come together. At end of animation, image should be intact.
        let filter = SplitFilter::new(
            EffectPosition::Start,
            Duration::from_secs(1),
            SplitDirection::Horizontal,
        );
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // After duration, image should be fully together
        filter
            .apply_split(
                &mut buffer,
                100,
                100,
                Duration::from_secs(2),
                Duration::from_secs(10),
            )
            .unwrap();
        // Image should be unchanged (animation complete)
        for y in 0..100 {
            for x in 0..100 {
                let p = buffer.get_pixel(x, y);
                assert_eq!(
                    p[0], 128,
                    "Pixel should be unchanged after split-in completes"
                );
            }
        }
    }

    #[test]
    fn test_off_center_split() {
        // Split at 30% from left: left half is 30px, right half is 70px
        let filter = SplitFilter::default()
            .with_split_position(0.3)
            .with_shadow(0.0);
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // Mid-animation
        filter
            .apply_split(
                &mut buffer,
                100,
                100,
                Duration::from_secs_f32(0.5),
                Duration::from_secs(1),
            )
            .unwrap();
        // The gap should be around x=30 (the split position)
        // At ratio=0.5 with EaseInOut, the gap is smaller than at ratio=1
        // Just verify there's some transparency near the split line
        let near_split = buffer.get_pixel(30, 50);
        assert_eq!(
            near_split[3], 0,
            "Pixel near split line should be transparent"
        );
    }

    #[test]
    fn test_fully_split_clears_buffer() {
        // At the very end of the animation, both halves should be fully off-screen
        let filter = SplitFilter::default();
        let img = RgbaImage::from_fn(100, 100, |_x, _y| Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        // total_duration=1s, frame_time=1s → time_until_end=0s → ratio=1.0 → fully split
        filter
            .apply_split(
                &mut buffer,
                100,
                100,
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .unwrap();
        // All pixels should be transparent
        for y in 0..100 {
            for x in 0..100 {
                let p = buffer.get_pixel(x, y);
                assert_eq!(
                    p[3], 0,
                    "Pixel ({}, {}) should be transparent when fully split",
                    x, y
                );
            }
        }
    }
}
