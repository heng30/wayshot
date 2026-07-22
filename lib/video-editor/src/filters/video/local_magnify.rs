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
pub struct LocalMagnifyFilter {
    // Center X position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "0.5"))]
    pub center_x: f32,

    // Center Y position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "0.5"))]
    pub center_y: f32,

    // Selection radius in pixels (based on 1080p)
    #[derivative(Default(value = "100"))]
    pub selection_radius: u32,

    // Scale factor (1.0 = no magnification, 2.0 = 2x larger output)
    #[derivative(Default(value = "2.0"))]
    pub scale: f32,

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

pub struct MagnifyValues {
    pub center_x: f32,
    pub center_y: f32,
    pub selection_radius: u32,
    pub scale: f32,
    pub border_width: u32,
    pub border_color: Option<(u8, u8, u8, u8)>,
}

impl LocalMagnifyFilter {
    pub const NAME: &'static str = "local magnify";

    pub fn new(center_x: f32, center_y: f32, selection_radius: u32, scale: f32) -> Self {
        Self {
            center_x,
            center_y,
            selection_radius,
            scale,
            border_color: Some((255, 255, 255, 255)),
            border_width: 2,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float2("center", "Position", 0.0, 1.0, 0.5, 0.5),
            AnimatableProperty::float("selection_radius", "Selection Radius", 0.0, 5000.0, 100.0),
            AnimatableProperty::float("scale", "Scale", 1.0, 10.0, 2.0),
            AnimatableProperty::float("border_width", "Border Width", 0.0, 500.0, 2.0),
            AnimatableProperty::color("border_color", "Border Color", 255, 255, 255, 255),
        ]
    }

    pub fn get_values_at_time(&self, time_ms: i64) -> MagnifyValues {
        let (center_x, center_y) = self
            .keyframe_tracks
            .get_track("center")
            .map(|track| get_float2_at_time(track, time_ms, self.center_x, self.center_y))
            .unwrap_or((self.center_x, self.center_y));

        let selection_radius = self
            .keyframe_tracks
            .get_track("selection_radius")
            .map(|track| get_float_at_time(track, time_ms, self.selection_radius as f32) as u32)
            .unwrap_or(self.selection_radius);

        let scale = self
            .keyframe_tracks
            .get_track("scale")
            .map(|track| get_float_at_time(track, time_ms, self.scale))
            .unwrap_or(self.scale);

        let border_width = self
            .keyframe_tracks
            .get_track("border_width")
            .map(|track| get_float_at_time(track, time_ms, self.border_width as f32) as u32)
            .unwrap_or(self.border_width);

        let border_color = self
            .keyframe_tracks
            .get_track("border_color")
            .filter(|track| track.has_keyframes())
            .map(|track| get_color_at_time(track, time_ms, (255, 255, 255, 255)))
            .or(self.border_color);

        MagnifyValues {
            center_x,
            center_y,
            selection_radius,
            scale,
            border_width,
            border_color,
        }
    }

    #[inline]
    fn is_inside_circle(x: f64, y: f64, cx: f64, cy: f64, radius: f64) -> f64 {
        let dx = x - cx;
        let dy = y - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        let aa_range = 1.0;

        if dist <= radius - aa_range {
            1.0
        } else if dist <= radius + aa_range {
            (radius + aa_range - dist) / (2.0 * aa_range)
        } else {
            0.0
        }
    }

    fn apply_magnify(&self, values: &MagnifyValues, buffer: &mut RgbaImage) -> Result<()> {
        let width = buffer.width();
        let height = buffer.height();

        let output_radius = (values.selection_radius as f32 * values.scale) as u32;

        let cx = (values.center_x * width as f32).round() as i32;
        let cy = (values.center_y * height as f32).round() as i32;

        if output_radius == 0 || values.scale <= 1.0 {
            return Ok(());
        }

        let selection_radius = values.selection_radius;
        let sel_left = cx - selection_radius as i32;
        let sel_top = cy - selection_radius as i32;
        let out_left = cx - output_radius as i32;
        let out_top = cy - output_radius as i32;

        // Create a temporary buffer for the selection content
        // We'll extract circular content from selection radius and render it at output radius
        let sel_size = selection_radius * 2;
        let out_size = output_radius * 2;

        if sel_size == 0 || out_size == 0 {
            return Ok(());
        }

        // Create selection buffer - extract content from source in a circle
        let mut selection_buffer = RgbaImage::new(sel_size, sel_size);
        let sel_cx = selection_radius as f64;
        let sel_cy = selection_radius as f64;

        for y in 0..sel_size {
            for x in 0..sel_size {
                // Check if inside selection circle
                let alpha = Self::is_inside_circle(
                    x as f64,
                    y as f64,
                    sel_cx,
                    sel_cy,
                    selection_radius as f64,
                );
                if alpha > 0.0 {
                    // Map to source image coordinates
                    let src_x = sel_left + x as i32;
                    let src_y = sel_top + y as i32;

                    // Get pixel from source (or transparent if out of bounds)
                    let src_pixel = if src_x >= 0
                        && src_x < width as i32
                        && src_y >= 0
                        && src_y < height as i32
                    {
                        buffer.get_pixel(src_x as u32, src_y as u32).clone()
                    } else {
                        Rgba([0, 0, 0, 0])
                    };

                    // Apply alpha based on circle mask
                    let final_alpha = (src_pixel[3] as f64 * alpha).min(255.0) as u8;
                    selection_buffer.put_pixel(
                        x,
                        y,
                        Rgba([src_pixel[0], src_pixel[1], src_pixel[2], final_alpha]),
                    );
                }
            }
        }

        // Scale up the selection buffer to output size using bilinear interpolation
        let mut magnified_buffer = RgbaImage::new(out_size, out_size);
        let scale_factor = values.scale as f64;

        for out_y in 0..out_size {
            for out_x in 0..out_size {
                // Check if inside output circle
                let out_cx = output_radius as f64;
                let out_cy = output_radius as f64;
                let alpha = Self::is_inside_circle(
                    out_x as f64,
                    out_y as f64,
                    out_cx,
                    out_cy,
                    output_radius as f64,
                );

                if alpha > 0.0 {
                    // Map output coordinates back to selection coordinates
                    let sel_x = (out_x as f64 / scale_factor).clamp(0.0, (sel_size - 1) as f64);
                    let sel_y = (out_y as f64 / scale_factor).clamp(0.0, (sel_size - 1) as f64);

                    // Bilinear interpolation
                    let x0 = sel_x.floor() as u32;
                    let y0 = sel_y.floor() as u32;
                    let x1 = (x0 + 1).min(sel_size - 1);
                    let y1 = (y0 + 1).min(sel_size - 1);

                    let dx = sel_x - x0 as f64;
                    let dy = sel_y - y0 as f64;

                    let p00 = selection_buffer.get_pixel(x0, y0);
                    let p01 = selection_buffer.get_pixel(x1, y0);
                    let p10 = selection_buffer.get_pixel(x0, y1);
                    let p11 = selection_buffer.get_pixel(x1, y1);

                    // Interpolate each channel
                    let interpolate =
                        |v00: u8, v01: u8, v10: u8, v11: u8, dx: f64, dy: f64| -> u8 {
                            let v0 = v00 as f64 * (1.0 - dx) + v01 as f64 * dx;
                            let v1 = v10 as f64 * (1.0 - dx) + v11 as f64 * dx;
                            ((v0 * (1.0 - dy) + v1 * dy).clamp(0.0, 255.0)) as u8
                        };

                    let pixel = Rgba([
                        interpolate(p00[0], p01[0], p10[0], p11[0], dx, dy),
                        interpolate(p00[1], p01[1], p10[1], p11[1], dx, dy),
                        interpolate(p00[2], p01[2], p10[2], p11[2], dx, dy),
                        interpolate(p00[3], p01[3], p10[3], p11[3], dx, dy),
                    ]);

                    // Apply circle mask alpha
                    let final_alpha = (pixel[3] as f64 * alpha).min(255.0) as u8;
                    magnified_buffer.put_pixel(
                        out_x,
                        out_y,
                        Rgba([pixel[0], pixel[1], pixel[2], final_alpha]),
                    );
                }
            }
        }

        // Draw the magnified content onto the original buffer
        // We need to blend the magnified circular region with the original image
        let out_cx_f = output_radius as f64;
        let out_cy_f = output_radius as f64;

        // Draw output circle onto buffer
        for y in 0..out_size {
            for x in 0..out_size {
                let alpha = Self::is_inside_circle(
                    x as f64,
                    y as f64,
                    out_cx_f,
                    out_cy_f,
                    output_radius as f64,
                );
                if alpha > 0.01 {
                    // Target position in original buffer
                    let tgt_x = out_left + x as i32;
                    let tgt_y = out_top + y as i32;

                    if tgt_x >= 0 && tgt_x < width as i32 && tgt_y >= 0 && tgt_y < height as i32 {
                        let magnified_pixel = magnified_buffer.get_pixel(x, y);
                        let src_pixel = buffer.get_pixel(tgt_x as u32, tgt_y as u32);

                        // Blend magnified content with source (magnified over source)
                        let src_alpha = src_pixel[3] as f64 / 255.0;
                        let mag_alpha = magnified_pixel[3] as f64 / 255.0;

                        // Only draw if magnified content has significant alpha
                        if mag_alpha > 0.01 {
                            let out_alpha = mag_alpha + src_alpha * (1.0 - mag_alpha);
                            if out_alpha > 0.0 {
                                let blend =
                                    |src: u8, mag: u8, src_a: f64, mag_a: f64, out_a: f64| -> u8 {
                                        ((mag as f64 * mag_a + src as f64 * src_a * (1.0 - mag_a))
                                            / out_a)
                                            .clamp(0.0, 255.0)
                                            as u8
                                    };

                                let final_pixel = Rgba([
                                    blend(
                                        src_pixel[0],
                                        magnified_pixel[0],
                                        src_alpha,
                                        mag_alpha,
                                        out_alpha,
                                    ),
                                    blend(
                                        src_pixel[1],
                                        magnified_pixel[1],
                                        src_alpha,
                                        mag_alpha,
                                        out_alpha,
                                    ),
                                    blend(
                                        src_pixel[2],
                                        magnified_pixel[2],
                                        src_alpha,
                                        mag_alpha,
                                        out_alpha,
                                    ),
                                    (out_alpha * 255.0).clamp(0.0, 255.0) as u8,
                                ]);

                                buffer.put_pixel(tgt_x as u32, tgt_y as u32, final_pixel);
                            }
                        }
                    }
                }
            }
        }

        // Draw border on the output circle
        if let Some(border) = values.border_color {
            if values.border_width > 0 {
                self.draw_border(buffer, cx, cy, output_radius, values.border_width, border)?;
            }
        }

        Ok(())
    }

    fn draw_border(
        &self,
        buffer: &mut RgbaImage,
        cx: i32,
        cy: i32,
        radius: u32,
        border_width: u32,
        border_color: (u8, u8, u8, u8),
    ) -> Result<()> {
        let cx_f = cx as f64;
        let cy_f = cy as f64;
        let radius_f = radius as f64;
        let half_border = border_width as f64 / 2.0;
        let outer_radius = radius_f + half_border;
        let inner_radius = (radius_f - half_border).max(0.0);
        let aa_range = 1.5;

        let border_rgba = Rgba([
            border_color.0,
            border_color.1,
            border_color.2,
            border_color.3,
        ]);

        // Bounding box
        let margin = (half_border + aa_range).ceil() as i32 + 1;
        let x_start = (cx - radius as i32 - margin).max(0) as u32;
        let x_end = (cx + radius as i32 + margin + 1).min(buffer.width() as i32) as u32;
        let y_start = (cy - radius as i32 - margin).max(0) as u32;
        let y_end = (cy + radius as i32 + margin + 1).min(buffer.height() as i32) as u32;

        for y in y_start..y_end {
            for x in x_start..x_end {
                // Use pixel center
                let dx = x as f64 + 0.5 - cx_f;
                let dy = y as f64 + 0.5 - cy_f;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist >= inner_radius - aa_range && dist <= outer_radius + aa_range {
                    // Calculate alpha based on position relative to border edges
                    let alpha = if dist <= inner_radius + aa_range
                        && dist >= inner_radius - aa_range
                    {
                        // Near inner edge
                        (dist - (inner_radius - aa_range)) / (2.0 * aa_range)
                    } else if dist >= outer_radius - aa_range && dist <= outer_radius + aa_range {
                        // Near outer edge
                        (outer_radius + aa_range - dist) / (2.0 * aa_range)
                    } else if dist > inner_radius + aa_range && dist < outer_radius - aa_range {
                        // Inside border (fully opaque)
                        1.0
                    } else {
                        continue;
                    };

                    if alpha > 0.05 {
                        let pixel = buffer.get_pixel_mut(x, y);
                        let dst_alpha = pixel[3] as f64 / 255.0;
                        let src_alpha = border_rgba[3] as f64 / 255.0 * alpha;
                        let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

                        if out_alpha > 0.0 {
                            pixel[0] = ((border_rgba[0] as f64 * src_alpha
                                + pixel[0] as f64 * dst_alpha * (1.0 - src_alpha))
                                / out_alpha) as u8;
                            pixel[1] = ((border_rgba[1] as f64 * src_alpha
                                + pixel[1] as f64 * dst_alpha * (1.0 - src_alpha))
                                / out_alpha) as u8;
                            pixel[2] = ((border_rgba[2] as f64 * src_alpha
                                + pixel[2] as f64 * dst_alpha * (1.0 - src_alpha))
                                / out_alpha) as u8;
                            pixel[3] = (out_alpha * 255.0) as u8;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl VideoFilter for LocalMagnifyFilter {
    crate::impl_default_video_filter!(LocalMagnifyFilter);

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
        let scaled_values = MagnifyValues {
            center_x: values.center_x,
            center_y: values.center_y,
            selection_radius: scale_pixel_for_height(values.selection_radius, output_height),
            scale: values.scale,
            border_width: scale_pixel_for_height(values.border_width, output_height),
            border_color: values.border_color,
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                self.apply_magnify(&scaled_values, buffer)?;
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

        if let Some(track) = tracks.get_track("selection_radius")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "selection_radius",
                time_ms,
                KeyframeValue::Float(self.selection_radius as f32),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("scale")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value("scale", time_ms, KeyframeValue::Float(self.scale));
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

