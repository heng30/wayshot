// Page flip filter — realistic book-style page turn using the turn-rs library.
//
// Uses the perpendicular bisector fold model from turn.js, ported to Rust.
// Supports 4 corners, horizontal/vertical axis, and shadow rendering.

use crate::{
    Result,
    filters::traits::{VideoData, VideoFilter},
    tracks::video_frame_cache::VideoImage,
};
use image::RgbaImage;
use std::time::Duration;
use turn_rs::{Corner, FlipAxis, FlipConfig, FlipDirection, render_flip};

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
/// When the page flip animation occurs within the segment.
pub enum PageFlipPosition {
    #[default]
    Start = 0,
    End = 1,
}

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
pub enum PageFlipDirection {
    #[default]
    Forward = 0,
    Backward = 1,
    RoundTrip = 2,
}

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
pub enum PageFlipCorner {
    #[default]
    BottomRight = 0,
    BottomLeft = 1,
    TopRight = 2,
    TopLeft = 3,
}

impl From<PageFlipCorner> for Corner {
    fn from(value: PageFlipCorner) -> Self {
        match value {
            PageFlipCorner::BottomRight => Corner::BottomRight,
            PageFlipCorner::BottomLeft => Corner::BottomLeft,
            PageFlipCorner::TopRight => Corner::TopRight,
            PageFlipCorner::TopLeft => Corner::TopLeft,
        }
    }
}

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
pub enum PageFlipAxis {
    #[default]
    Horizontal = 0,
    Vertical = 1,
}

impl From<PageFlipAxis> for FlipAxis {
    fn from(value: PageFlipAxis) -> Self {
        match value {
            PageFlipAxis::Horizontal => FlipAxis::Horizontal,
            PageFlipAxis::Vertical => FlipAxis::Vertical,
        }
    }
}

impl From<PageFlipDirection> for FlipDirection {
    fn from(value: PageFlipDirection) -> Self {
        match value {
            PageFlipDirection::Forward => FlipDirection::Forward,
            PageFlipDirection::Backward => FlipDirection::Backward,
            PageFlipDirection::RoundTrip => FlipDirection::RoundTrip,
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
pub struct PageFlipFilter {
    /// Duration of the flip animation.
    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub duration: Duration,

    /// When the flip animation occurs: Start or End of the segment.
    #[derivative(Default(value = "PageFlipPosition::Start"))]
    pub position: PageFlipPosition,

    /// Which corner the flip originates from.
    #[derivative(Default(value = "PageFlipCorner::BottomRight"))]
    pub corner: PageFlipCorner,

    /// Flip direction: Forward, Backward, or RoundTrip.
    #[derivative(Default(value = "PageFlipDirection::Forward"))]
    pub direction: PageFlipDirection,

    /// Flip axis: Horizontal (book-style) or Vertical (calendar-style).
    #[derivative(Default(value = "PageFlipAxis::Horizontal"))]
    pub axis: PageFlipAxis,

    /// Whether to render shadow/highlight gradients.
    #[derivative(Default(value = "true"))]
    pub shadow: bool,

    /// Flip extent: how far the page flips (0.0–1.0).
    ///
    /// - `1.0` (default): full flip — the page turns all the way over.
    /// - `0.5`: half flip — the page only folds halfway (curl effect).
    /// - `0.0`: no flip — the page stays flat.
    #[derivative(Default(value = "1.0"))]
    pub flip_extent: f64,

    /// Whether to keep the original image as the base layer underneath the flip.
    ///
    /// - `false` (default): clean page-turn — the area revealed by the flip
    ///   is transparent (letting underlying layers show).
    /// - `true`: "peel/tear" effect — the original image remains fully visible
    ///   underneath the flip animation, as if the page is being peeled off
    ///   a surface that still shows the same image.
    #[derivative(Default(value = "false"))]
    pub keep_base: bool,
}

impl PageFlipFilter {
    pub const NAME: &'static str = "page flip";

    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            position: PageFlipPosition::Start,
            corner: PageFlipCorner::BottomRight,
            direction: PageFlipDirection::Forward,
            axis: PageFlipAxis::Horizontal,
            shadow: true,
            flip_extent: 1.0,
            keep_base: false,
        }
    }
}

impl VideoFilter for PageFlipFilter {
    crate::impl_default_video_filter!(PageFlipFilter);

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
                self.apply_flip(
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

impl PageFlipFilter {
    fn apply_flip(
        &self,
        buffer: &mut RgbaImage,
        canvas_width: u32,
        canvas_height: u32,
        frame_time_offset: Duration,
        total_duration: Duration,
    ) -> Result<()> {
        let duration_ms = self.duration.as_millis() as u32;

        if duration_ms == 0 {
            return Ok(());
        }

        // Calculate animation time based on position:
        //   Start: animation runs from segment start, time_ms = frame_time_offset
        //   End:   animation runs towards segment end, time_ms = duration - time_from_end
        let time_ms: u32 = match self.position {
            PageFlipPosition::Start => {
                let t = frame_time_offset.as_millis() as u32;
                if t >= duration_ms {
                    // Animation complete
                    if matches!(self.direction, PageFlipDirection::Forward)
                        && self.flip_extent >= 0.95
                    {
                        // Full flip complete → page is gone, show transparent
                        *buffer = RgbaImage::from_pixel(
                            canvas_width,
                            canvas_height,
                            image::Rgba([0, 0, 0, 0]),
                        );
                    }
                    // Partial flip (flip_extent < 1): page stays at the folded position
                    // Backward: flat, unchanged
                    // RoundTrip: flat, unchanged
                    return Ok(());
                }
                t
            }
            PageFlipPosition::End => {
                let time_from_end = total_duration.saturating_sub(frame_time_offset);
                let t = time_from_end.as_millis() as u32;
                if t >= duration_ms {
                    // Not yet in animation zone, image is flat
                    return Ok(());
                }
                // time_from_end goes duration→0, elapsed goes 0→duration
                duration_ms - t
            }
        };

        // For RoundTrip at Start position, if time_ms >= duration_ms, animation is complete
        if time_ms >= duration_ms {
            return Ok(());
        }

        // Determine the actual image content area.
        // The buffer may be smaller than the canvas (non-16:9 images are scaled
        // to fit, leaving transparent padding). We only apply the flip to the
        // image content itself, not the transparent padding — otherwise the
        // padding participates in the flip and becomes black when composited.
        let (offset_x, offset_y, img_w, img_h) = if buffer.width() != canvas_width
            || buffer.height() != canvas_height
        {
            // Buffer is smaller than canvas — compute center offset
            let x = (canvas_width.saturating_sub(buffer.width())) / 2;
            let y = (canvas_height.saturating_sub(buffer.height())) / 2;
            (x, y, buffer.width(), buffer.height())
        } else {
            // Buffer is canvas-sized — the image fills the canvas
            (0, 0, canvas_width, canvas_height)
        };

        // Build the turn-rs FlipConfig
        let config = FlipConfig {
            corner: self.corner.into(),
            duration_ms,
            time_ms,
            shadow: self.shadow,
            direction: self.direction.into(),
            axis: self.axis.into(),
            flip_extent: self.flip_extent.clamp(0.0, 1.0),
        };

        let front = buffer.clone();
        // When keep_base is false (default), back is transparent so the area
        // revealed by the flip becomes transparent (clean page-turn effect).
        // When keep_base is true, back is the same as front so the original
        // image remains visible underneath (peel/tear effect).
        let back = if self.keep_base {
            front.clone()
        } else {
            RgbaImage::new(img_w, img_h)
        };

        let mut result = render_flip(&front, &back, &config);

        if self.keep_base {
            // "Peel/tear" effect: overlay the original image underneath the flip
            // so the original image remains fully visible while the page peels away.
            let mut base = front.clone();
            image::imageops::overlay(&mut base, &result, 0, 0);
            result = base;
        }

        // Place the flipped image back onto a canvas-sized transparent buffer
        if img_w != canvas_width || img_h != canvas_height {
            let mut canvas = RgbaImage::new(canvas_width, canvas_height);
            image::imageops::overlay(&mut canvas, &result, offset_x as i64, offset_y as i64);
            *buffer = canvas;
        } else {
            *buffer = result;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_name() {
        assert_eq!(PageFlipFilter::NAME, "page flip");
    }

    #[test]
    fn test_default_values() {
        let filter = PageFlipFilter::default();
        assert_eq!(filter.position, PageFlipPosition::Start);
        assert_eq!(filter.corner, PageFlipCorner::BottomRight);
        assert_eq!(filter.direction, PageFlipDirection::Forward);
        assert_eq!(filter.axis, PageFlipAxis::Horizontal);
        assert!(filter.shadow);
        assert_eq!(filter.duration, Duration::from_secs(1));
        assert!((filter.flip_extent - 1.0).abs() < f64::EPSILON);
        assert!(!filter.keep_base);
    }

    #[test]
    fn test_setters() {
        let filter = PageFlipFilter::default()
            .with_position(PageFlipPosition::End)
            .with_corner(PageFlipCorner::TopLeft)
            .with_direction(PageFlipDirection::Backward)
            .with_axis(PageFlipAxis::Vertical)
            .with_shadow(false)
            .with_flip_extent(0.5)
            .with_keep_base(true);

        assert_eq!(filter.position, PageFlipPosition::End);
        assert_eq!(filter.corner, PageFlipCorner::TopLeft);
        assert_eq!(filter.direction, PageFlipDirection::Backward);
        assert_eq!(filter.axis, PageFlipAxis::Vertical);
        assert!(!filter.shadow);
        assert!((filter.flip_extent - 0.5).abs() < f64::EPSILON);
        assert!(filter.keep_base);
    }

    #[test]
    fn test_forward_flip_at_start_unchanged() {
        // At time_ms=0, forward flip hasn't started → image unchanged
        let filter = PageFlipFilter::default();
        let img = RgbaImage::from_fn(100, 100, |_x, _y| image::Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        filter
            .apply_flip(
                &mut buffer,
                100,
                100,
                Duration::from_millis(0),
                Duration::from_secs(10),
            )
            .unwrap();
        // Should be unchanged
        let p = buffer.get_pixel(50, 50);
        assert_eq!(p[0], 128);
    }

    #[test]
    fn test_forward_flip_mid_animation() {
        // At mid-point of forward flip, the image should be different from original
        let filter = PageFlipFilter::default();
        let img = RgbaImage::from_fn(100, 100, |_x, _y| image::Rgba([128, 128, 128, 255]));
        let mut buffer = img.clone();
        filter
            .apply_flip(
                &mut buffer,
                100,
                100,
                Duration::from_millis(500),
                Duration::from_secs(10),
            )
            .unwrap();
        // Some pixels should differ from the original gray
        let mut changed = false;
        for y in 0..100 {
            for x in 0..100 {
                let p = buffer.get_pixel(x, y);
                if p[0] != 128 || p[3] != 255 {
                    changed = true;
                    break;
                }
            }
            if changed {
                break;
            }
        }
        assert!(changed, "Image should change during flip animation");
    }

    #[test]
    fn test_keep_base_true_retains_original() {
        // With keep_base=true, the original image should remain visible
        let filter = PageFlipFilter::default().with_keep_base(true);
        let img = RgbaImage::from_fn(100, 100, |_x, _y| image::Rgba([200, 100, 50, 255]));
        let mut buffer = img.clone();
        filter
            .apply_flip(
                &mut buffer,
                100,
                100,
                Duration::from_millis(500),
                Duration::from_secs(10),
            )
            .unwrap();
        // With keep_base, the flat (non-flipped) side should still show original color
        // Check top-left area which should be on the flat side for BottomRight flip
        let p = buffer.get_pixel(5, 5);
        assert_eq!(p[0], 200, "keep_base should preserve original image color");
    }
}
