use crate::{
    Result,
    filters::{
        subtitle::{
            renderer::{calculate_text_size, draw_glyph, load_font},
            style::scale_pixel_for_height,
        },
        traits::{GlobalFilter, GlobalFilterData},
    },
};
use cosmic_text::fontdb::{Style as FontStyle, Weight as FontWeight};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use image::{Rgba, RgbaImage};
use std::{path::PathBuf, time::Duration};

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
pub enum TimerMode {
    #[default]
    CountUp = 0,
    CountDown = 1,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimerSegment {
    pub start_time: Duration,
    pub end_time: Duration,
    pub mode: TimerMode,
    pub position_x: f32,
    pub position_y: f32,
    pub font_size: u32,
    pub font_path: Option<PathBuf>,
    pub font_family: Option<String>,
    pub font_style: Option<String>,
    pub text_color: (u8, u8, u8, u8),
    pub background_color: (u8, u8, u8, u8),
    pub padding: u32,
    pub border_radius: u32,
}

impl Default for TimerSegment {
    fn default() -> Self {
        Self {
            start_time: Duration::ZERO,
            end_time: Duration::from_secs(10),
            mode: TimerMode::CountUp,
            position_x: 0.5,
            position_y: 0.5,
            font_size: 24,
            font_path: None,
            font_family: None,
            font_style: None,
            text_color: (255, 255, 255, 255),
            background_color: (40, 40, 40, 180),
            padding: 8,
            border_radius: 4,
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
#[non_exhaustive]
pub struct TimerFilter {
    #[derivative(Default(value = "Vec::new()"))]
    pub segments: Vec<TimerSegment>,
}

impl TimerFilter {
    pub const NAME: &'static str = "timer";

    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn add_segment(&mut self, segment: TimerSegment) {
        self.segments.push(segment);
        self.segments
            .sort_by(|a, b| a.start_time.cmp(&b.start_time));
    }

    fn format_time(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{:02}:{:02}", mins, secs)
        }
    }

    fn draw_timer(
        &self,
        buffer: &mut RgbaImage,
        current_offset: Duration,
        output_height: u32,
    ) -> Result<()> {
        let img_width = buffer.width();
        let img_height = buffer.height();

        for segment in &self.segments {
            // Check if current time is within segment's time range
            if current_offset < segment.start_time || current_offset > segment.end_time {
                continue;
            }

            // Calculate display time based on mode
            let display_duration = match segment.mode {
                TimerMode::CountUp => current_offset - segment.start_time,
                TimerMode::CountDown => segment.end_time - current_offset,
            };

            let time_text = Self::format_time(display_duration);

            // Scale parameters for resolution
            let font_size = scale_pixel_for_height(segment.font_size, output_height);
            let padding = scale_pixel_for_height(segment.padding, output_height);
            let border_radius = scale_pixel_for_height(segment.border_radius, output_height);

            // Load font and prepare text rendering
            let mut font_system_opt = None;
            let mut font_family_opt = None;
            let mut font_weight_opt = FontWeight::NORMAL;
            let mut font_fs_style_opt = FontStyle::Normal;
            let mut swash_cache_opt = None;
            let mut text_dimensions = None;

            if let Some(font_path) = &segment.font_path {
                let mut fs = FontSystem::new();
                let preferred_style = segment.font_style.as_deref();
                if let Ok((family, weight, style)) =
                    load_font(font_path, &mut fs, None, preferred_style)
                {
                    if let Ok((width, height, baseline, _)) = calculate_text_size(
                        &mut fs,
                        &time_text,
                        font_size as f32,
                        &family,
                        weight,
                        style,
                        None,
                    ) {
                        text_dimensions = Some((width, height, baseline));
                        font_system_opt = Some(fs);
                        font_family_opt = Some(family);
                        font_weight_opt = weight;
                        font_fs_style_opt = style;
                        swash_cache_opt = Some(SwashCache::new());
                    }
                }
            }

            let (text_width, text_height, baseline_offset) = if let Some(dim) = text_dimensions {
                dim
            } else {
                let estimated_width = (font_size as u32) * time_text.len() as u32 / 2;
                let estimated_height = font_size as u32;
                (
                    estimated_width,
                    estimated_height,
                    estimated_height as i32 / 4,
                )
            };

            let box_width = text_width + 2 * padding;
            let box_height = text_height + 2 * padding;

            let box_x = (segment.position_x * img_width as f32 - box_width as f32 / 2.0) as i32;
            let box_y = (segment.position_y * img_height as f32 - box_height as f32 / 2.0) as i32;

            self.draw_rounded_rect(
                buffer,
                box_x,
                box_y,
                box_width,
                box_height,
                border_radius,
                segment.background_color,
            );

            if let (Some(mut fs), Some(family), Some(mut cache)) =
                (font_system_opt, font_family_opt, swash_cache_opt)
            {
                self.draw_timer_text(
                    buffer,
                    &mut fs,
                    &mut cache,
                    &family,
                    font_weight_opt,
                    font_fs_style_opt,
                    box_x + padding as i32,
                    box_y + padding as i32 + baseline_offset,
                    &time_text,
                    font_size,
                    segment.text_color,
                );
            }
        }

        Ok(())
    }

    fn draw_rounded_rect(
        &self,
        buffer: &mut RgbaImage,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        radius: u32,
        color: (u8, u8, u8, u8),
    ) {
        let rgba = Rgba([color.0, color.1, color.2, color.3]);

        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx as i32;
                let py = y + dy as i32;

                if px < 0 || py < 0 || px >= buffer.width() as i32 || py >= buffer.height() as i32 {
                    continue;
                }

                // Check if inside rounded corners
                let inside_corner = if radius > 0 {
                    // Top-left corner
                    if dx < radius && dy < radius {
                        let corner_x = radius - dx;
                        let corner_y = radius - dy;
                        corner_x * corner_x + corner_y * corner_y <= radius * radius
                    }
                    // Top-right corner
                    else if dx >= width - radius && dy < radius {
                        let corner_x = dx - (width - radius);
                        let corner_y = radius - dy;
                        corner_x * corner_x + corner_y * corner_y <= radius * radius
                    }
                    // Bottom-left corner
                    else if dx < radius && dy >= height - radius {
                        let corner_x = radius - dx;
                        let corner_y = dy - (height - radius);
                        corner_x * corner_x + corner_y * corner_y <= radius * radius
                    }
                    // Bottom-right corner
                    else if dx >= width - radius && dy >= height - radius {
                        let corner_x = dx - (width - radius);
                        let corner_y = dy - (height - radius);
                        corner_x * corner_x + corner_y * corner_y <= radius * radius
                    } else {
                        true
                    }
                } else {
                    true
                };

                if inside_corner {
                    self.blend_pixel(buffer, px as u32, py as u32, &rgba);
                }
            }
        }
    }

    fn draw_timer_text(
        &self,
        buffer: &mut RgbaImage,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        font_family: &str,
        font_weight: FontWeight,
        font_fs_style: FontStyle,
        x: i32,
        y: i32,
        text: &str,
        font_size: u32,
        color: (u8, u8, u8, u8),
    ) {
        let metrics = Metrics {
            font_size: font_size as f32,
            line_height: font_size as f32 * 1.2,
        };
        let attrs = Attrs::new()
            .family(Family::Name(font_family))
            .weight(font_weight)
            .style(font_fs_style);

        let mut text_buffer = Buffer::new(font_system, metrics);
        text_buffer.set_text(text, &attrs, Shaping::Basic, None);
        text_buffer.shape_until_scroll(font_system, false);

        let rgba = Rgba([color.0, color.1, color.2, color.3]);

        for run in text_buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(glyph_img) = swash_cache.get_image(font_system, physical.cache_key) {
                    let glyph_x = x + physical.x;
                    let glyph_y = y + physical.y;
                    draw_glyph(buffer, glyph_img, glyph_x, glyph_y, rgba);
                }
            }
        }
    }

    fn blend_pixel(&self, buffer: &mut RgbaImage, x: u32, y: u32, color: &Rgba<u8>) {
        let pixel = buffer.get_pixel_mut(x, y);
        let src_alpha = color[3] as f32 / 255.0;
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
}

impl GlobalFilter for TimerFilter {
    crate::impl_default_global_filter!(TimerFilter);

    fn apply(&self, data: &mut GlobalFilterData) -> Result<()> {
        let output_height = data.image.height();
        self.draw_timer(&mut data.image, data.timeline_offset, output_height)
    }
}
