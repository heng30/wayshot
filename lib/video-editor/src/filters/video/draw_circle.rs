use crate::{
    Result,
    filters::{
        interpolation::{get_color_at_time, get_float_at_time, get_float2_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage};

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
#[non_exhaustive]
pub struct DrawCircleFilter {
    // Center X position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    // Center Y position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    #[derivative(Default(value = "50"))]
    pub radius: u32,

    // Fill color (R, G, B, A) - None for transparent
    pub fill_color: Option<(u8, u8, u8, u8)>,

    // Border color (R, G, B, A) - None for no border
    #[derivative(Default(value = "Some((255, 255, 255, 255))"))]
    pub border_color: Option<(u8, u8, u8, u8)>,

    // Border width in pixels
    #[derivative(Default(value = "2"))]
    pub border_width: u32,

    // Keyframe tracks for animatable properties
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

/// Interpolated circle values at a specific time
pub struct CircleValues {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: u32,
    pub border_width: u32,
    pub fill_color: Option<(u8, u8, u8, u8)>,
    pub border_color: Option<(u8, u8, u8, u8)>,
}

impl DrawCircleFilter {
    pub const NAME: &'static str = "draw circle";

    pub fn new(center_x: f32, center_y: f32, radius: u32) -> Self {
        Self {
            center_x,
            center_y,
            radius,
            fill_color: None,
            border_color: Some((255, 255, 255, 255)),
            border_width: 2,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float2("center", "Position", 0.0, 1.0, 0.5, 0.5),
            AnimatableProperty::float("radius", "Radius", 0.0, 5000.0, 50.0),
            AnimatableProperty::float("border_width", "Border Width", 0.0, 500.0, 2.0),
            AnimatableProperty::color("fill_color", "Fill Color", 0, 0, 0, 255),
            AnimatableProperty::color("border_color", "Border Color", 255, 255, 255, 255),
        ]
    }

    /// Get interpolated values at a specific time
    pub fn get_values_at_time(&self, time_ms: i64) -> CircleValues {
        // Get center position from keyframes or use static values
        let (center_x, center_y) = self
            .keyframe_tracks
            .get_track("center")
            .map(|track| get_float2_at_time(track, time_ms, self.center_x, self.center_y))
            .unwrap_or((self.center_x, self.center_y));

        // Get radius from keyframes or use static value
        let radius = self
            .keyframe_tracks
            .get_track("radius")
            .map(|track| get_float_at_time(track, time_ms, self.radius as f32) as u32)
            .unwrap_or(self.radius);

        // Get border width from keyframes or use static value
        let border_width = self
            .keyframe_tracks
            .get_track("border_width")
            .map(|track| get_float_at_time(track, time_ms, self.border_width as f32) as u32)
            .unwrap_or(self.border_width);

        // Get fill color from keyframes or use static value
        let fill_color = self
            .keyframe_tracks
            .get_track("fill_color")
            .filter(|track| track.has_keyframes())
            .map(|track| get_color_at_time(track, time_ms, (0, 0, 0, 255)))
            .or(self.fill_color);

        // Get border color from keyframes or use static value
        let border_color = self
            .keyframe_tracks
            .get_track("border_color")
            .filter(|track| track.has_keyframes())
            .map(|track| get_color_at_time(track, time_ms, (255, 255, 255, 255)))
            .or(self.border_color);

        CircleValues {
            center_x,
            center_y,
            radius,
            border_width,
            fill_color,
            border_color,
        }
    }

    #[inline]
    fn draw_pixel_aa(buffer: &mut RgbaImage, x: u32, y: u32, color: &Rgba<u8>, opacity: f32) {
        if x >= buffer.width() || y >= buffer.height() {
            return;
        }

        // Compute source alpha: opacity * color's alpha channel
        let src_alpha = opacity * (color[3] as f32 / 255.0);
        if src_alpha <= 0.0 {
            return;
        }

        let pixel = buffer.get_pixel_mut(x, y);
        let dst_alpha = pixel[3] as f32 / 255.0;
        let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

        if out_alpha > 0.0 {
            // Proper "over" compositing with premultiplied alpha
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

    /// Blend two colors with interpolation factor t (0.0 = c1, 1.0 = c2)
    #[inline]
    fn blend_colors(c1: Rgba<u8>, c2: Rgba<u8>, t: f64) -> Rgba<u8> {
        let t = t.clamp(0.0, 1.0);
        Rgba([
            (c1[0] as f64 * (1.0 - t) + c2[0] as f64 * t).round() as u8,
            (c1[1] as f64 * (1.0 - t) + c2[1] as f64 * t).round() as u8,
            (c1[2] as f64 * (1.0 - t) + c2[2] as f64 * t).round() as u8,
            (c1[3] as f64 * (1.0 - t) + c2[3] as f64 * t).round() as u8,
        ])
    }

    /// Draw a circle with both fill and border in a single pass (unified anti-aliasing)
    fn draw_circle_with_border(
        buffer: &mut RgbaImage,
        cx: i32,
        cy: i32,
        radius: u32,
        fill_color: &Rgba<u8>,
        border_width: u32,
        border_color: &Rgba<u8>,
    ) {
        let cx_f = cx as f64;
        let cy_f = cy as f64;
        let radius_f = radius as f64;
        let aa_range: f64 = 1.5;

        // Inner radius for fill area
        let inner_radius = (radius_f - border_width as f64).max(0.0);

        // Bounding box with margin for anti-aliasing
        let margin = aa_range.ceil() as i32 + 1;
        let x_start = (cx - radius as i32 - margin).max(0) as u32;
        let x_end = (cx + radius as i32 + margin + 1).min(buffer.width() as i32) as u32;
        let y_start = (cy - radius as i32 - margin).max(0) as u32;
        let y_end = (cy + radius as i32 + margin + 1).min(buffer.height() as i32) as u32;

        for y in y_start..y_end {
            for x in x_start..x_end {
                // Use pixel center for accurate distance calculation
                let dx = x as f64 + 0.5 - cx_f;
                let dy = y as f64 + 0.5 - cy_f;
                let dist_sqrt = (dx * dx + dy * dy).sqrt();

                // Check if outside the outer circle
                if dist_sqrt > radius_f + aa_range {
                    continue;
                }

                // Compute final color based on region
                let final_color = if dist_sqrt < inner_radius - aa_range {
                    // Inside fill area (fully inside, no edge AA needed)
                    *fill_color
                } else if dist_sqrt < inner_radius + aa_range && inner_radius > 0.0 {
                    // Near inner edge (transition between fill and border)
                    let dist_from_inner = inner_radius - dist_sqrt;
                    let alpha = (dist_from_inner / aa_range).clamp(0.0, 1.0);
                    // alpha: 1 = fill, 0 = border
                    Self::blend_colors(*border_color, *fill_color, alpha)
                } else if dist_sqrt < radius_f - aa_range {
                    // Inside border area (fully inside, no edge AA needed)
                    *border_color
                } else if dist_sqrt <= radius_f + aa_range {
                    // Near outer edge (transition between border and background)
                    let dist_from_outer = radius_f - dist_sqrt;
                    let alpha = (dist_from_outer / aa_range).clamp(0.0, 1.0);
                    // alpha: 1 = border, 0 = transparent
                    Self::blend_colors(Rgba([0, 0, 0, 0]), *border_color, alpha)
                } else {
                    // Outside
                    continue;
                };

                Self::draw_pixel_aa(buffer, x, y, &final_color, 1.0);
            }
        }
    }

    fn draw_circle_aa(
        buffer: &mut RgbaImage,
        cx: i32,
        cy: i32,
        radius: u32,
        color: &Rgba<u8>,
        fill: bool,
        border_width: u32,
    ) {
        let cx_f = cx as f64;
        let cy_f = cy as f64;
        let radius_f = radius as f64;
        let r_squared = radius_f * radius_f;

        // Bounding box with margin for anti-aliasing and border
        let aa_range: f64 = 1.5;
        let margin = (border_width as f64 / 2.0).ceil() as i32 + aa_range.ceil() as i32 + 1;
        let x_start = (cx - radius as i32 - margin).max(0) as u32;
        let x_end = (cx + radius as i32 + margin + 1).min(buffer.width() as i32) as u32;
        let y_start = (cy - radius as i32 - margin).max(0) as u32;
        let y_end = (cy + radius as i32 + margin + 1).min(buffer.height() as i32) as u32;

        for y in y_start..y_end {
            for x in x_start..x_end {
                // Use pixel center for accurate distance calculation
                let dx = x as f64 + 0.5 - cx_f;
                let dy = y as f64 + 0.5 - cy_f;
                let dist_sqrt = (dx * dx + dy * dy).sqrt();

                if fill {
                    // Filled circle with smooth edges
                    if border_width == 0 {
                        // No border, just fill with proper anti-aliasing
                        if dist_sqrt <= radius_f {
                            let dist_from_edge = radius_f - dist_sqrt;

                            let alpha = if dist_from_edge >= aa_range {
                                1.0
                            } else if dist_from_edge > 0.0 {
                                // Smooth transition: 0 at edge to 1 at aa_range inside
                                dist_from_edge / aa_range
                            } else {
                                0.0
                            };
                            if alpha > 0.05 {
                                Self::draw_pixel_aa(buffer, x, y, color, alpha as f32);
                            }
                        }
                    } else {
                        // Fill with border (exclude border area)
                        let inner_radius = radius_f - border_width as f64;
                        let inner_r_squared = inner_radius * inner_radius;
                        let distance = dist_sqrt * dist_sqrt;

                        if distance <= r_squared && distance >= inner_r_squared.max(0.0) {
                            // Border area with improved anti-aliasing
                            let dist_from_outer = radius_f - dist_sqrt;
                            let dist_from_inner = dist_sqrt - inner_radius;

                            let alpha = if dist_from_outer <= aa_range {
                                dist_from_outer / aa_range
                            } else if dist_from_inner <= aa_range {
                                dist_from_inner / aa_range
                            } else {
                                1.0
                            };
                            if alpha > 0.05 {
                                Self::draw_pixel_aa(buffer, x, y, color, alpha as f32);
                            }
                        } else if distance < inner_r_squared.max(0.0) {
                            // Inner fill area
                            Self::draw_pixel_aa(buffer, x, y, color, 1.0);
                        }
                    }
                } else {
                    // Outline only (border) with improved anti-aliasing
                    let half_border = border_width as f64 / 2.0;
                    let outer_radius = radius_f + half_border;
                    let inner_radius = (radius_f - half_border).max(0.0);

                    if dist_sqrt <= outer_radius && dist_sqrt >= inner_radius {
                        let dist_from_outer = outer_radius - dist_sqrt;
                        let dist_from_inner = dist_sqrt - inner_radius;

                        let alpha = if dist_from_outer <= aa_range {
                            // Near outer edge: smooth transition from 0 at edge to 1 at aa_range
                            dist_from_outer / aa_range
                        } else if dist_from_inner <= aa_range {
                            // Near inner edge: smooth transition from 0 at edge to 1 at aa_range
                            dist_from_inner / aa_range
                        } else {
                            // Center of border: full opacity
                            1.0
                        };

                        if alpha > 0.05 {
                            Self::draw_pixel_aa(buffer, x, y, color, alpha as f32);
                        }
                    }
                }
            }
        }
    }

    fn apply_circle(
        &self,
        values: &CircleValues,
        buffer: &mut RgbaImage,
        opacity: f32,
    ) -> Result<()> {
        // Convert normalized coordinates to pixel coordinates
        let cx = (values.center_x * buffer.width() as f32).round() as i32;
        let cy = (values.center_y * buffer.height() as f32).round() as i32;

        // Apply opacity to colors
        let apply_opacity = |color: (u8, u8, u8, u8)| -> [u8; 4] {
            [
                color.0,
                color.1,
                color.2,
                (color.3 as f32 * opacity).clamp(0.0, 255.0) as u8,
            ]
        };

        // Use unified single-pass rendering when both fill and border exist
        if let (Some(fill), Some(border)) = (values.fill_color, values.border_color) {
            let fill_color = Rgba(apply_opacity(fill));
            let border_color = Rgba(apply_opacity(border));
            Self::draw_circle_with_border(
                buffer,
                cx,
                cy,
                values.radius,
                &fill_color,
                values.border_width,
                &border_color,
            );
        } else if let Some(fill) = values.fill_color {
            // Fill only (no border)
            let fill_color = Rgba(apply_opacity(fill));
            Self::draw_circle_aa(buffer, cx, cy, values.radius, &fill_color, true, 0);
        } else if let Some(border) = values.border_color {
            // Border only (no fill)
            let border_color = Rgba(apply_opacity(border));
            Self::draw_circle_aa(
                buffer,
                cx,
                cy,
                values.radius,
                &border_color,
                false,
                values.border_width,
            );
        }

        Ok(())
    }
}

impl VideoFilter for DrawCircleFilter {
    crate::impl_default_video_filter!(DrawCircleFilter);

    fn take_effect_in_layer_frame(&self) -> bool {
        false
    }

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        // Calculate current time in milliseconds relative to segment start
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time (or static values if no keyframes)
        let values = self.get_values_at_time(time_ms);

        // Get output height for resolution-relative scaling
        let output_height = data.config.output_height;

        // Scale pixel values from base 1080p to target resolution
        let scaled_values = CircleValues {
            center_x: values.center_x,
            center_y: values.center_y,
            radius: scale_pixel_for_height(values.radius, output_height),
            border_width: scale_pixel_for_height(values.border_width, output_height),
            fill_color: values.fill_color,
            border_color: values.border_color,
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // Render with opacity 1.0 (keyframes or static values)
                self.apply_circle(&scaled_values, buffer, 1.0)?;
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

        if let Some(track) = tracks.get_track("center")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "center",
                time_ms,
                KeyframeValue::Float2(self.center_x, self.center_y),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("radius")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "radius",
                time_ms,
                KeyframeValue::Float(self.radius as f32),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("border_width")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "border_width",
                time_ms,
                KeyframeValue::Float(self.border_width as f32),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("fill_color")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            if let Some(color) = self.fill_color {
                tracks.update_keyframe_value(
                    "fill_color",
                    time_ms,
                    KeyframeValue::Color(color.0, color.1, color.2, color.3),
                );
                updated = true;
            }
        }

        if let Some(track) = tracks.get_track("border_color")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            if let Some(color) = self.border_color {
                tracks.update_keyframe_value(
                    "border_color",
                    time_ms,
                    KeyframeValue::Color(color.0, color.1, color.2, color.3),
                );
                updated = true;
            }
        }

        updated
    }
}
