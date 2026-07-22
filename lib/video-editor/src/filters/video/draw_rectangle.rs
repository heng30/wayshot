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
pub struct DrawRectangleFilter {
    #[derivative(Default(value = "0.4"))]
    pub x: f32,

    #[derivative(Default(value = "0.4"))]
    pub y: f32,

    #[derivative(Default(value = "0.2"))]
    pub width: f32,

    #[derivative(Default(value = "0.2"))]
    pub height: f32,

    // Fill color (R, G, B, A) - None for transparent
    pub fill_color: Option<(u8, u8, u8, u8)>,

    // Border color (R, G, B, A) - None for no border
    #[derivative(Default(value = "Some((255, 255, 255, 255))"))]
    pub border_color: Option<(u8, u8, u8, u8)>,

    #[derivative(Default(value = "2"))]
    pub border_width: u32,

    #[derivative(Default(value = "0"))]
    pub corner_radius: u32,

    // Keyframe tracks for animatable properties
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

/// Interpolated rectangle values at a specific time
struct RectangleValues {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: u32,
    border_width: u32,
    fill_color: Option<(u8, u8, u8, u8)>,
    border_color: Option<(u8, u8, u8, u8)>,
}

impl DrawRectangleFilter {
    pub const NAME: &'static str = "draw rectangle";

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            fill_color: None,
            border_color: Some((255, 255, 255, 255)),
            border_width: 2,
            corner_radius: 0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float2("position", "Position", 0.0, 1.0, 0.1, 0.1),
            AnimatableProperty::float2("size", "Size", 0.0, 1.0, 0.1, 0.1),
            AnimatableProperty::float("corner_radius", "Corner Radius", 0.0, 200.0, 0.0),
            AnimatableProperty::float("border_width", "Border Width", 0.0, 50.0, 2.0),
            AnimatableProperty::color("fill_color", "Fill Color", 0, 0, 0, 255),
            AnimatableProperty::color("border_color", "Border Color", 255, 255, 255, 255),
        ]
    }

    /// Get interpolated values at a specific time
    fn get_values_at_time(&self, time_ms: i64) -> RectangleValues {
        // Get position from keyframes or use static values
        let (x, y) = self
            .keyframe_tracks
            .get_track("position")
            .map(|track| get_float2_at_time(track, time_ms, self.x, self.y))
            .unwrap_or((self.x, self.y));

        // Get size from keyframes or use static values (relative 0-1 values)
        let (width, height) = self
            .keyframe_tracks
            .get_track("size")
            .map(|track| get_float2_at_time(track, time_ms, self.width, self.height))
            .unwrap_or((self.width, self.height));

        // Get corner radius from keyframes or use static value
        let corner_radius = self
            .keyframe_tracks
            .get_track("corner_radius")
            .map(|track| get_float_at_time(track, time_ms, self.corner_radius as f32) as u32)
            .unwrap_or(self.corner_radius);

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

        RectangleValues {
            x,
            y,
            width,
            height,
            corner_radius,
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

    /// 判断点是否在圆角矩形内部
    fn inside_rounded_rect(x: f64, y: f64, w: f64, h: f64, r: f64) -> bool {
        // 检查是否在圆角区域
        let near_left = x < r;
        let near_right = x > w - r;
        let near_top = y < r;
        let near_bottom = y > h - r;

        let in_corner = (near_left || near_right) && (near_top || near_bottom);

        if in_corner {
            // 圆角区域：检查是否在圆内
            let cx = if near_left { r } else { w - r };
            let cy = if near_top { r } else { h - r };
            let dx = x - cx;
            let dy = y - cy;
            dx * dx + dy * dy <= r * r
        } else {
            // 直线区域
            x >= 0.0 && x <= w && y >= 0.0 && y <= h
        }
    }

    /// 计算点到圆角矩形边缘的距离（正值表示在内部）
    fn distance_to_edge(x: f64, y: f64, w: f64, h: f64, r: f64) -> f64 {
        let near_left = x < r;
        let near_right = x > w - r;
        let near_top = y < r;
        let near_bottom = y > h - r;

        let in_corner = (near_left || near_right) && (near_top || near_bottom);

        if in_corner {
            // 圆角区域：到圆弧的距离
            let cx = if near_left { r } else { w - r };
            let cy = if near_top { r } else { h - r };
            let dx = x - cx;
            let dy = y - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            r - dist
        } else if near_left {
            x
        } else if near_right {
            w - x
        } else if near_top {
            y
        } else if near_bottom {
            h - y
        } else {
            // 内部区域：到最近边的距离
            x.min(w - x).min(y).min(h - y)
        }
    }

    /// 颜色混合
    fn blend_colors(c1: Rgba<u8>, c2: Rgba<u8>, t: f64) -> Rgba<u8> {
        let t = t.clamp(0.0, 1.0);
        Rgba([
            (c1[0] as f64 * (1.0 - t) + c2[0] as f64 * t).round() as u8,
            (c1[1] as f64 * (1.0 - t) + c2[1] as f64 * t).round() as u8,
            (c1[2] as f64 * (1.0 - t) + c2[2] as f64 * t).round() as u8,
            (c1[3] as f64 * (1.0 - t) + c2[3] as f64 * t).round() as u8,
        ])
    }

    fn draw_filled_rounded_rect(
        buffer: &mut RgbaImage,
        rect_x: i32,
        rect_y: i32,
        width: u32,
        height: u32,
        corner_radius: u32,
        fill_color: &Rgba<u8>,
    ) {
        let x = rect_x as u32;
        let y = rect_y as u32;
        let (img_width, img_height) = buffer.dimensions();

        // 限制圆角半径
        let max_radius = (width / 2).min(height / 2);
        let radius = corner_radius.min(max_radius);

        // 遍历绘制区域
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;

                if px >= img_width || py >= img_height {
                    continue;
                }

                // 使用浮点数计算像素中心
                let fx = dx as f64 + 0.5;
                let fy = dy as f64 + 0.5;
                let fw = width as f64;
                let fh = height as f64;
                let fr = radius as f64;

                // 判断是否在圆角矩形内
                let in_rect = Self::inside_rounded_rect(fx, fy, fw, fh, fr);
                if !in_rect {
                    continue;
                }

                // 计算抗锯齿
                let dist = Self::distance_to_edge(fx, fy, fw, fh, fr);
                let aa_range = 1.5;

                let final_color = if dist < aa_range && dist >= 0.0 {
                    let alpha = dist / aa_range;
                    Self::blend_colors(Rgba([0, 0, 0, 0]), *fill_color, alpha)
                } else {
                    *fill_color
                };

                Self::draw_pixel_aa(buffer, px, py, &final_color, 1.0);
            }
        }
    }

    /// 绘制带边框的圆角矩形
    fn draw_rounded_rectangle_with_border(
        buffer: &mut RgbaImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        corner_radius: u32,
        fill_color: &Rgba<u8>,
        border_width: u32,
        border_color: &Rgba<u8>,
    ) {
        let (img_width, img_height) = buffer.dimensions();

        // 限制圆角半径
        let max_radius = (width / 2).min(height / 2);
        let radius = corner_radius.min(max_radius);

        // 遍历绘制区域
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;

                if px >= img_width || py >= img_height {
                    continue;
                }

                // 使用浮点数计算像素中心
                let fx = dx as f64 + 0.5;
                let fy = dy as f64 + 0.5;
                let fw = width as f64;
                let fh = height as f64;
                let fr = radius as f64;

                // 判断是否在外部圆角矩形内
                let in_outer = Self::inside_rounded_rect(fx, fy, fw, fh, fr);
                if !in_outer {
                    continue;
                }

                // 计算颜色和抗锯齿
                let final_color = {
                    let bw = border_width as f64;
                    let inner_w = (fw - 2.0 * bw).max(0.0);
                    let inner_h = (fh - 2.0 * bw).max(0.0);
                    let inner_r = (fr - bw).max(0.0);

                    // 计算到外边界的距离
                    let outer_dist = Self::distance_to_edge(fx, fy, fw, fh, fr);

                    // 计算到内边界的距离（边框与填充的交界）
                    let inner_dist = if inner_w > 0.0 && inner_h > 0.0 {
                        Self::distance_to_edge(fx - bw, fy - bw, inner_w, inner_h, inner_r)
                    } else {
                        f64::INFINITY
                    };

                    let aa_range = 1.5;

                    // 判断是在填充区还是边框区
                    let in_fill = inner_w > 0.0
                        && inner_h > 0.0
                        && Self::inside_rounded_rect(fx - bw, fy - bw, inner_w, inner_h, inner_r);

                    if in_fill {
                        // 填充区：检查是否靠近内边界
                        if inner_dist < aa_range && inner_dist >= 0.0 {
                            let alpha = inner_dist / aa_range;
                            Self::blend_colors(*border_color, *fill_color, alpha)
                        } else {
                            *fill_color
                        }
                    } else {
                        // 边框区：需要检查外边界和内边界
                        let mut color = *border_color;

                        // 外边界抗锯齿（与背景混合）
                        if outer_dist < aa_range && outer_dist >= 0.0 {
                            let alpha = outer_dist / aa_range;
                            color = Self::blend_colors(Rgba([0, 0, 0, 0]), color, alpha);
                        }

                        // 内边界抗锯齿（与填充混合）- 仅当靠近内边界时
                        if inner_dist < aa_range && inner_dist >= 0.0 {
                            let alpha = inner_dist / aa_range;
                            color = Self::blend_colors(color, *fill_color, 1.0 - alpha);
                        }

                        color
                    }
                };

                Self::draw_pixel_aa(buffer, px, py, &final_color, 1.0);
            }
        }
    }

    fn apply_rectangle(
        &self,
        values: &RectangleValues,
        buffer: &mut RgbaImage,
        opacity: f32,
    ) -> Result<()> {
        // Convert normalized coordinates to pixel coordinates
        let x = (values.x * buffer.width() as f32).round() as i32;
        let y = (values.y * buffer.height() as f32).round() as i32;
        let width = (values.width * buffer.width() as f32).round() as u32;
        let height = (values.height * buffer.height() as f32).round() as u32;
        let corner_radius = values.corner_radius.min(width.min(height) / 2);

        // Apply opacity to colors
        let apply_opacity = |color: (u8, u8, u8, u8)| -> [u8; 4] {
            [
                color.0,
                color.1,
                color.2,
                (color.3 as f32 * opacity).clamp(0.0, 255.0) as u8,
            ]
        };

        self.draw_rectangle_direct(
            buffer,
            x,
            y,
            width,
            height,
            corner_radius,
            values.border_width,
            values.fill_color,
            values.border_color,
            apply_opacity,
        );

        Ok(())
    }

    /// Draw rectangle directly without rotation
    fn draw_rectangle_direct(
        &self,
        buffer: &mut RgbaImage,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        corner_radius: u32,
        border_width: u32,
        fill_color: Option<(u8, u8, u8, u8)>,
        border_color: Option<(u8, u8, u8, u8)>,
        apply_opacity: impl Fn((u8, u8, u8, u8)) -> [u8; 4],
    ) {
        // Draw filled rectangle
        // Skip if we have a rounded rectangle with border (will be handled together)
        if let Some(fill) = fill_color {
            let fill_rgba = Rgba(apply_opacity(fill));

            // Only draw fill separately if: no border OR corner_radius is 0
            let should_draw_fill_separately = border_color.is_none() || corner_radius == 0;

            if should_draw_fill_separately {
                if corner_radius == 0 {
                    // Simple filled rectangle
                    for dy in 0..height {
                        for dx in 0..width {
                            let px = x + dx as i32;
                            let py = y + dy as i32;
                            if px >= 0
                                && py >= 0
                                && px < buffer.width() as i32
                                && py < buffer.height() as i32
                            {
                                Self::draw_pixel_aa(buffer, px as u32, py as u32, &fill_rgba, 1.0);
                            }
                        }
                    }
                } else {
                    // Rounded rectangle fill (no border)
                    Self::draw_filled_rounded_rect(
                        buffer,
                        x,
                        y,
                        width,
                        height,
                        corner_radius,
                        &fill_rgba,
                    );
                }
            }
        }

        // Draw border
        if let Some(border) = border_color {
            let border_rgba = Rgba(apply_opacity(border));

            if corner_radius == 0 {
                // Simple rectangle border (original implementation)
                let bw = border_width;

                // Top edge
                for dy in 0..bw {
                    for dx in 0..width {
                        let px = x + dx as i32;
                        let py = y + dy as i32;
                        if px >= 0
                            && py >= 0
                            && px < buffer.width() as i32
                            && py < buffer.height() as i32
                        {
                            Self::draw_pixel_aa(buffer, px as u32, py as u32, &border_rgba, 1.0);
                        }
                    }
                }

                // Bottom edge
                for dy in 0..bw {
                    for dx in 0..width {
                        let px = x + dx as i32;
                        let py = y + height as i32 - dy as i32 - 1;
                        if px >= 0
                            && py >= 0
                            && px < buffer.width() as i32
                            && py < buffer.height() as i32
                        {
                            Self::draw_pixel_aa(buffer, px as u32, py as u32, &border_rgba, 1.0);
                        }
                    }
                }

                // Left edge
                for dy in bw..height as u32 - bw {
                    for dx in 0..bw {
                        let px = x + dx as i32;
                        let py = y + dy as i32;
                        if px >= 0
                            && py >= 0
                            && px < buffer.width() as i32
                            && py < buffer.height() as i32
                        {
                            Self::draw_pixel_aa(buffer, px as u32, py as u32, &border_rgba, 1.0);
                        }
                    }
                }

                // Right edge
                for dy in bw..height as u32 - bw {
                    for dx in 0..bw {
                        let px = x + width as i32 - dx as i32 - 1;
                        let py = y + dy as i32;
                        if px >= 0
                            && py >= 0
                            && px < buffer.width() as i32
                            && py < buffer.height() as i32
                        {
                            Self::draw_pixel_aa(buffer, px as u32, py as u32, &border_rgba, 1.0);
                        }
                    }
                }
            } else {
                // Rounded rectangle with border support
                let fill_rgba = fill_color
                    .map(|f| Rgba(apply_opacity(f)))
                    .unwrap_or(Rgba([0, 0, 0, 0]));

                Self::draw_rounded_rectangle_with_border(
                    buffer,
                    x.max(0) as u32,
                    y.max(0) as u32,
                    width,
                    height,
                    corner_radius,
                    &fill_rgba,
                    border_width,
                    &border_rgba,
                );
            }
        }
    }
}

impl VideoFilter for DrawRectangleFilter {
    crate::impl_default_video_filter!(DrawRectangleFilter);

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
        // x, y, width, height are normalized (0-1) values, no scaling needed
        // corner_radius, border_width are pixel values relative to 1080p
        let scaled_values = RectangleValues {
            x: values.x,
            y: values.y,
            width: values.width,
            height: values.height,
            corner_radius: scale_pixel_for_height(values.corner_radius, output_height),
            border_width: scale_pixel_for_height(values.border_width, output_height),
            fill_color: values.fill_color,
            border_color: values.border_color,
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // Render with opacity 1.0 (keyframes or static values)
                self.apply_rectangle(&scaled_values, buffer, 1.0)?;
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

        if let Some(track) = tracks.get_track("position")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "position",
                time_ms,
                KeyframeValue::Float2(self.x, self.y),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("size")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "size",
                time_ms,
                KeyframeValue::Float2(self.width as f32, self.height as f32),
            );
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
