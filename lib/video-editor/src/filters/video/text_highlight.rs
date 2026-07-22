use crate::{
    Result,
    filters::{
        subtitle::style::scale_pixel_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::Rgba;
use serde::{Deserialize, Serialize};

/// Highlight region with normalized coordinates (0-1)
#[derive(Debug, Clone, Copy, derivative::Derivative, Serialize, Deserialize, PartialEq)]
#[derivative(Default)]
pub struct HighlightRegion {
    /// X position (0-1, normalized to frame width)
    #[derivative(Default(value = "0.2"))]
    pub x: f32,
    /// Y position (0-1, normalized to frame height)
    #[derivative(Default(value = "0.2"))]
    pub y: f32,
    /// Width (0-1, normalized to frame width)
    #[derivative(Default(value = "0.5"))]
    pub width: f32,
    /// Height (0-1, normalized to frame height)
    #[derivative(Default(value = "0.5"))]
    pub height: f32,
}

/// Text highlight filter for highlighting regions with animation
///
/// This filter highlights specific regions on video frames by detecting
/// pixels that match the background color.
///
/// # Parameters
/// - `regions`: List of regions to highlight (normalized 0-1 coordinates)
/// - `highlight_color`: RGBA color to apply to highlighted pixels
/// - `background_color_to_detect`: RGB color for background detection
/// - `pixel_per_second`: Pixels to draw per second (animation speed)
#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[serde(default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct TextHighlightFilter {
    /// Regions to highlight (normalized 0-1 coordinates)
    #[derivative(Default(value = "Vec::new()"))]
    pub regions: Vec<HighlightRegion>,

    /// Highlight color (RGBA). The color to apply to detected pixels.
    #[derivative(Default(value = "[255, 255, 0, 180]"))]
    pub highlight_color: [u8; 4],

    /// Background color to detect (RGB). Pixels matching this color will be highlighted.
    #[derivative(Default(value = "[255, 255, 255]"))]
    pub background_color_to_detect: [u8; 3],

    /// Pixels to draw per second for animation speed. Default: 2000 pixels/s
    #[derivative(Default(value = "2000"))]
    pub pixel_per_second: u32,

    /// When false (default), highlight pixels that match the background color.
    /// When true, highlight pixels that do NOT match the background color.
    #[derivative(Default(value = "false"))]
    pub invert: bool,

    /// Similarity threshold for color matching (0.0-1.0). Default: 0.8
    /// Higher values require closer color match, lower values are more lenient.
    #[derivative(Default(value = "0.8"))]
    pub similarity_threshold: f32,
}

impl TextHighlightFilter {
    pub const NAME: &'static str = "text highlight";

    pub fn new(regions: Vec<HighlightRegion>) -> Self {
        Self {
            regions,
            ..Default::default()
        }
    }

    /// Calculate which region to highlight and the progress within that region
    /// Returns (region_index, progress) where progress is 0.0-1.0
    fn get_highlight_progress(
        &self,
        time_ms: i64,
        frame_width: u32,
        frame_height: u32,
    ) -> (usize, f32) {
        if self.regions.is_empty() {
            return (0, 1.0);
        }

        // Scale pixel_per_second based on frame height (1080P standard)
        // This ensures consistent animation speed across different resolutions
        let scaled_pixel_per_second =
            scale_pixel_for_height(self.pixel_per_second, frame_height) as f64;

        // Use actual time in milliseconds directly
        // pixel_per_second determines how many pixels are drawn per second
        // For a region of width W pixels, it takes W / pixel_per_second seconds to fully draw
        let scaled_progress = time_ms as f64;

        // Calculate total animation time for all regions
        let total_width_pixels: f64 = self
            .regions
            .iter()
            .map(|r| (r.width * frame_width as f32) as u32 as f64)
            .sum();
        let total_animation_ms = total_width_pixels / scaled_pixel_per_second * 1000.0;

        // Calculate which region and progress within that region
        let mut accumulated_width = 0.0;
        let mut region_index = 0;
        let mut within_region_progress = 0.0;

        for (idx, region) in self.regions.iter().enumerate() {
            let region_width_pixels = (region.width * frame_width as f32) as u32 as f64;
            let region_duration_ms = region_width_pixels / scaled_pixel_per_second * 1000.0;
            let region_end_ms =
                accumulated_width / scaled_pixel_per_second * 1000.0 + region_duration_ms;
            let region_start_ms = accumulated_width / scaled_pixel_per_second * 1000.0;

            if scaled_progress >= region_end_ms {
                // This region is complete
                accumulated_width += region_width_pixels;
            } else if scaled_progress >= region_start_ms {
                // We're in this region
                region_index = idx;
                within_region_progress = (scaled_progress - region_start_ms) / region_duration_ms;
                break;
            } else {
                // We haven't started yet
                region_index = idx;
                within_region_progress = 0.0;
                break;
            }
        }

        // If all regions are complete
        if scaled_progress >= total_animation_ms {
            return (self.regions.len(), 1.0);
        }

        (region_index, within_region_progress.min(1.0) as f32)
    }

    /// Calculate similarity between two colors (0.0 to 1.0)
    #[inline]
    fn color_similarity(c1: [u8; 3], c2: [u8; 3]) -> f32 {
        let dr = (c1[0] as i32 - c2[0] as i32).abs() as f32;
        let dg = (c1[1] as i32 - c2[1] as i32).abs() as f32;
        let db = (c1[2] as i32 - c2[2] as i32).abs() as f32;

        // Normalize to 0-1 range (max difference is 255 * 3)
        let max_diff = 255.0 * 3.0;
        let total_diff = dr + dg + db;

        1.0 - (total_diff / max_diff)
    }

    /// Draw highlight based on background color detection with partial progress
    /// progress: 0.0 = none, 1.0 = fully drawn (from left to right)
    fn draw_highlight(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        region: HighlightRegion,
        progress: f32,
        highlight_color: [u8; 4],
        background_color_to_detect: [u8; 3],
        invert: bool,
        similarity_threshold: f32,
    ) {
        let frame_width = buffer.width();
        let frame_height = buffer.height();

        // Convert normalized coordinates to pixel coordinates
        let x = (region.x * frame_width as f32) as u32;
        let y = (region.y * frame_height as f32) as u32;
        let width = (region.width * frame_width as f32) as u32;
        let height = (region.height * frame_height as f32) as u32;

        let drawn_width = (width as f32 * progress).ceil() as u32;

        if drawn_width == 0 {
            return;
        }

        // Create partial bounds for the drawn portion
        let drawn_x_end = (x + drawn_width).min(frame_width);
        let drawn_y_end = (y + height).min(frame_height);

        Self::draw_highlight_background_color_mode(
            buffer,
            x,
            y,
            drawn_x_end,
            drawn_y_end,
            highlight_color,
            background_color_to_detect,
            invert,
            similarity_threshold,
        );
    }

    /// BackgroundColor mode: highlight pixels that match (or don't match if invert=true) background_color_to_detect
    fn draw_highlight_background_color_mode(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        x_end: u32,
        y_end: u32,
        highlight_color: [u8; 4],
        background_color_to_detect: [u8; 3],
        invert: bool,
        similarity_threshold: f32,
    ) {
        for py in y..y_end {
            for px in x..x_end {
                let existing: [u8; 4] = buffer.get_pixel(px, py).0;

                // Check if pixel color is similar to background color
                let pixel_color = [existing[0], existing[1], existing[2]];
                let similarity = Self::color_similarity(pixel_color, background_color_to_detect);

                // Determine if we should highlight this pixel based on invert mode:
                // - invert=false: highlight pixels that ARE background (similarity > threshold)
                // - invert=true: highlight pixels that are NOT background (similarity <= threshold)
                let should_highlight = (similarity > similarity_threshold) != invert;
                if should_highlight {
                    Self::apply_highlight_color(buffer, px, py, &Rgba(existing), highlight_color);
                }
            }
        }
    }

    /// Apply highlight color to a pixel using alpha blending
    #[inline]
    fn apply_highlight_color(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        px: u32,
        py: u32,
        existing: &Rgba<u8>,
        highlight_color: [u8; 4],
    ) {
        let alpha = highlight_color[3] as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        let base_r = existing[0] as f32;
        let base_g = existing[1] as f32;
        let base_b = existing[2] as f32;

        let hl_r = highlight_color[0] as f32;
        let hl_g = highlight_color[1] as f32;
        let hl_b = highlight_color[2] as f32;

        let new_r = (hl_r * alpha + base_r * inv_alpha) as u8;
        let new_g = (hl_g * alpha + base_g * inv_alpha) as u8;
        let new_b = (hl_b * alpha + base_b * inv_alpha) as u8;

        buffer.put_pixel(px, py, Rgba([new_r, new_g, new_b, 255]));
    }
}

impl VideoFilter for TextHighlightFilter {
    crate::impl_default_video_filter!(TextHighlightFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        if self.regions.is_empty() {
            return Ok(());
        }

        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get frame dimensions for progress calculation
        let (frame_width, frame_height) = data
            .frames
            .first()
            .and_then(|f| {
                if let VideoImage::Image { buffer, .. } = f {
                    Some((buffer.width(), buffer.height()))
                } else {
                    None
                }
            })
            .unwrap_or((1920, 1080));

        // Get highlight progress for current time
        let (current_region_idx, within_region_progress) =
            self.get_highlight_progress(time_ms, frame_width, frame_height);

        let highlight_color = self.highlight_color;
        let background_color_to_detect = self.background_color_to_detect;
        let invert = self.invert;
        let similarity_threshold = self.similarity_threshold;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // Highlight all completed regions (cumulative, fully drawn)
                for (idx, region) in self.regions.iter().enumerate() {
                    if idx < current_region_idx {
                        // Fully highlight this region
                        Self::draw_highlight(
                            buffer,
                            *region,
                            1.0,
                            highlight_color,
                            background_color_to_detect,
                            invert,
                            similarity_threshold,
                        );
                    } else if idx == current_region_idx {
                        // Partially highlight current region
                        Self::draw_highlight(
                            buffer,
                            *region,
                            within_region_progress,
                            highlight_color,
                            background_color_to_detect,
                            invert,
                            similarity_threshold,
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
