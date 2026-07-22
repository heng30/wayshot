use crate::{
    Result,
    filters::{
        interpolation::{get_color_at_time, get_float_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::Rgba;
use rayon::prelude::*;

/// Calculate distance from pixel to the edge of a rounded rectangle.
/// Positive value means inside the shape, negative means outside.
/// For corner regions, returns radial distance to the arc.
/// For straight edge regions, returns horizontal/vertical distance to edge.
///
/// Parameters:
/// - px, py: pixel coordinates relative to image origin (0,0)
/// - x, y: top-left corner of the rounded rectangle
/// - width, height: dimensions of the rounded rectangle
/// - radius: corner radius
pub fn distance_to_rounded_edge(
    px: u32,
    py: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    radius: u32,
) -> f32 {
    let near_left = px < x + radius;
    let near_right = px >= x + width.saturating_sub(radius);
    let near_top = py < y + radius;
    let near_bottom = py >= y + height.saturating_sub(radius);

    let in_corner = (near_left || near_right) && (near_top || near_bottom);

    if in_corner {
        // Corner region: return radial distance to the arc
        // For right/bottom corners, center is at the edge of the rounded rect
        let cx = if near_left {
            x + radius // left corner: center at x + radius
        } else {
            x + width - radius // right corner: center at x + width - radius (not width - radius - 1!)
        };
        let cy = if near_top {
            y + radius // top corner: center at y + radius
        } else {
            y + height - radius // bottom corner: center at y + height - radius
        };
        let dx = px as i32 - cx as i32;
        let dy = py as i32 - cy as i32;
        let dist = ((dx * dx + dy * dy) as f32).sqrt();
        radius as f32 - dist // positive = inside, negative = outside
    } else if near_left {
        (px - x) as f32
    } else if near_right {
        (x + width - px) as f32
    } else if near_top {
        (py - y) as f32
    } else if near_bottom {
        (y + height - py) as f32
    } else {
        // Interior region: return distance to nearest edge
        (px - x)
            .min(x + width - px)
            .min(py - y)
            .min(y + height - py) as f32
    }
}

/// Blend a source color onto a destination pixel with given alpha.
pub fn blend_pixel(pixel: &mut image::Rgba<u8>, color: Rgba<u8>, src_alpha: f32) {
    if src_alpha <= 0.0 {
        return;
    }

    let dst_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha > 0.0 {
        pixel[0] = ((color[0] as f32 * src_alpha + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[1] = ((color[1] as f32 * src_alpha + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[2] = ((color[2] as f32 * src_alpha + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
            / out_alpha) as u8;
        pixel[3] = (out_alpha * 255.0) as u8;
    }
}

/// Draw a rounded border with anti-aliasing onto an image.
/// The border is drawn as a stroke (hollow), not a fill.
///
/// Parameters:
/// - image: target image buffer
/// - x, y: top-left corner of the rounded rectangle area
/// - width, height: dimensions of the rounded rectangle area
/// - border_size: width of the border stroke in pixels
/// - radius: corner radius of the rounded rectangle
/// - border_color: RGBA color of the border
pub fn draw_rounded_border_aa(
    image: &mut image::RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    border_size: u32,
    radius: u32,
    border_color: Rgba<u8>,
) {
    if border_size == 0 || width == 0 || height == 0 {
        return;
    }

    let img_w = image.width();
    let img_h = image.height();
    if x >= img_w || y >= img_h {
        return;
    }

    let radius = radius.min(width / 2).min(height / 2);
    let base_alpha = border_color[3] as f32 / 255.0;
    let border_size_f = border_size as f32;

    let x_end = (x + width).min(img_w);
    let y_end = (y + height).min(img_h);

    for py in y..y_end {
        for px in x..x_end {
            let dist = distance_to_rounded_edge(px, py, x, y, width, height, radius);

            // Make pixels outside the rounded rectangle transparent
            // (beyond the anti-aliasing zone)
            if dist < -1.0 {
                // Outside the shape and beyond AA zone - make transparent
                image.get_pixel_mut(px, py)[3] = 0;
                continue;
            }

            // Check if pixel is in the border region or its anti-aliasing zone
            let (should_draw, aa_factor) = if dist < 0.0 {
                // Outside the shape but within AA zone at outer edge
                (true, dist + 1.0) // fades from 0 at dist=-1 to 1 at dist=0
            } else if dist < border_size_f {
                // Inside the border region
                (true, 1.0)
            } else if dist < border_size_f + 1.0 {
                // AA zone at inner edge
                (true, border_size_f + 1.0 - dist) // fades from 1 to 0
            } else {
                (false, 0.0)
            };

            if should_draw && aa_factor > 0.0 {
                // In the outer anti-aliasing zone (dist < 0), first attenuate
                // the original pixel alpha to prevent residual content showing through
                if dist < 0.0 {
                    let fade_factor = dist + 1.0; // 0.0 at dist=-1.0, 1.0 at dist=0.0
                    let pixel = image.get_pixel_mut(px, py);
                    pixel[3] = (pixel[3] as f32 * fade_factor) as u8;
                }
                blend_pixel(
                    image.get_pixel_mut(px, py),
                    border_color,
                    base_alpha * aa_factor,
                );
            }
        }
    }
}

/// Fill a rounded rectangle with anti-aliasing.
/// Uses signed distance field for smooth edge transitions.
///
/// Parameters:
/// - image: target image buffer
/// - x, y: top-left corner of the rounded rectangle
/// - width, height: dimensions of the rounded rectangle
/// - radius: corner radius of the rounded rectangle
/// - color: RGBA fill color
pub fn draw_rounded_rect_fill_aa(
    image: &mut image::RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    radius: u32,
    color: Rgba<u8>,
) {
    if width == 0 || height == 0 {
        return;
    }

    let img_w = image.width();
    let img_h = image.height();
    if x >= img_w || y >= img_h {
        return;
    }

    let radius = radius.min(width / 2).min(height / 2);
    let base_alpha = color[3] as f32 / 255.0;

    let x_end = (x + width).min(img_w);
    let y_end = (y + height).min(img_h);

    for py in y..y_end {
        for px in x..x_end {
            let dist = distance_to_rounded_edge(px, py, x, y, width, height, radius);

            // Calculate alpha based on distance
            // dist >= 0.0: fully inside, draw with full alpha
            // dist in -1.0..0.0: anti-aliasing zone, interpolate alpha
            // dist < -1.0: outside, don't draw
            let aa_factor = if dist >= 0.0 {
                1.0 // Fully inside
            } else if dist >= -1.0 {
                dist + 1.0 // Anti-aliasing: fades from 0 at dist=-1 to 1 at dist=0
            } else {
                0.0 // Outside, skip
            };

            if aa_factor > 0.0 {
                blend_pixel(image.get_pixel_mut(px, py), color, base_alpha * aa_factor);
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BorderFilter {
    pub size: u32,          // border width in pixels
    pub color: [u8; 4],     // RGBA color
    pub corner_radius: u32, // rounded corner radius
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for BorderFilter {
    fn default() -> Self {
        Self {
            size: 4,
            color: [255, 255, 255, 255],
            corner_radius: 32,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl BorderFilter {
    pub const NAME: &'static str = "border";

    pub fn new(size: u32, color: [u8; 4], corner_radius: u32) -> Self {
        Self {
            size,
            color,
            corner_radius,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("size", "Size", 0.0, 1000.0, 0.0),
            AnimatableProperty::float("corner_radius", "Corner Radius", 0.0, 5000.0, 0.0),
            AnimatableProperty::color("color", "Color", 0, 0, 0, 255),
        ]
    }

    fn draw_border(
        &self,
        image: &mut image::RgbaImage,
        scaled_size: u32,
        scaled_radius: u32,
        color: [u8; 4],
    ) {
        let width = image.width();
        let height = image.height();
        let border_color = Rgba(color);
        let size = scaled_size.min(width / 2).min(height / 2);
        let radius = scaled_radius.min(width / 2).min(height / 2);

        if size == 0 && radius == 0 {
            return;
        }

        if size == 0 {
            // Only apply rounded corner mask, no border
            self.apply_rounded_corner_mask(image, width, height, radius);
            return;
        }

        // size > 0: draw border normally
        if radius == 0 {
            // No rounded corners - draw simple rectangular border with transparent corners
            let inner_left = size;
            let inner_right = width.saturating_sub(size);
            let inner_top = size;
            let inner_bottom = height.saturating_sub(size);

            image
                .par_enumerate_pixels_mut()
                .for_each(|(px, py, pixel)| {
                    // Check if pixel is inside the inner rectangle (not in border area)
                    let inside_inner = px >= inner_left
                        && px < inner_right
                        && py >= inner_top
                        && py < inner_bottom;

                    if !inside_inner {
                        // Outside inner rectangle - either border or transparent corner
                        let in_border_h = px < size || px >= width.saturating_sub(size);
                        let in_border_v = py < size || py >= height.saturating_sub(size);

                        if in_border_h || in_border_v {
                            // In border area
                            *pixel = border_color;
                        } else {
                            // Corner area - make transparent
                            pixel[3] = 0;
                        }
                    }
                    // Inside inner rectangle - keep original pixel unchanged
                });
        } else {
            // Rounded corners - use the public anti-aliased border function
            draw_rounded_border_aa(image, 0, 0, width, height, size, radius, border_color);
        }
    }

    /// Apply rounded corner mask to make corners transparent.
    /// Used when border size is 0 but corner_radius > 0.
    fn apply_rounded_corner_mask(
        &self,
        image: &mut image::RgbaImage,
        width: u32,
        height: u32,
        radius: u32,
    ) {
        let radius = radius.min(width / 2).min(height / 2);

        image
            .par_enumerate_pixels_mut()
            .for_each(|(px, py, pixel)| {
                let dist = distance_to_rounded_edge(px, py, 0, 0, width, height, radius);

                // Make pixels outside the rounded rectangle transparent
                if dist < 0.0 {
                    let alpha = pixel[3] as f32 / 255.0;
                    let new_alpha = if dist < -1.0 {
                        0.0
                    } else {
                        // Anti-aliasing at edge: dist from -1.0 to 0.0
                        alpha * (dist + 1.0)
                    };
                    pixel[3] = (new_alpha * 255.0) as u8;
                }
            });
    }
}

impl VideoFilter for BorderFilter {
    crate::impl_default_video_filter!(BorderFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        // Get frame height for scaling (1080P standard)
        let frame_height = data
            .frames
            .first()
            .and_then(|f| {
                if let VideoImage::Image { buffer, .. } = f {
                    Some(buffer.height())
                } else {
                    None
                }
            })
            .unwrap_or(1080);

        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let size = self
            .keyframe_tracks
            .get_track("size")
            .map(|track| get_float_at_time(track, time_ms, self.size as f32) as u32)
            .unwrap_or(self.size);
        let corner_radius = self
            .keyframe_tracks
            .get_track("corner_radius")
            .map(|track| get_float_at_time(track, time_ms, self.corner_radius as f32) as u32)
            .unwrap_or(self.corner_radius);
        let color = self
            .keyframe_tracks
            .get_track("color")
            .map(|track| {
                get_color_at_time(
                    track,
                    time_ms,
                    (self.color[0], self.color[1], self.color[2], self.color[3]),
                )
            })
            .map(|c| [c.0, c.1, c.2, c.3])
            .unwrap_or(self.color);

        // Scale pixel values based on frame height
        let scaled_size = scale_pixel_for_height(size, frame_height);
        let scaled_radius = scale_pixel_for_height(corner_radius, frame_height);

        for frame in &mut data.frames {
            if let VideoImage::Image { buffer, .. } = frame {
                self.draw_border(buffer, scaled_size, scaled_radius, color);
            }
        }
        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        let mut updated = false;

        if let Some(track) = tracks.get_track("size")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value("size", time_ms, KeyframeValue::Float(self.size as f32));
            updated = true;
        }

        if let Some(track) = tracks.get_track("corner_radius")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "corner_radius",
                time_ms,
                KeyframeValue::Float(self.corner_radius as f32),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("color")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "color",
                time_ms,
                KeyframeValue::Color(self.color[0], self.color[1], self.color[2], self.color[3]),
            );
            updated = true;
        }

        updated
    }
}
