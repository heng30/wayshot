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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProgressSegment {
    pub text: String,
    pub start_time: Duration,
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
pub struct ProgressBarFilter {
    /// Position Y (normalized, 0=bottom, 1=top)
    #[derivative(Default(value = "0.0"))]
    pub position_y: f32,

    /// Vertical padding around text in pixels (scaled for resolution)
    #[derivative(Default(value = "4"))]
    pub padding: u32,

    /// Horizontal margin from edges in pixels (scaled for resolution)
    #[derivative(Default(value = "0"))]
    pub margin_h: u32,

    /// Background bar color (R, G, B, A)
    #[derivative(Default(value = "(40, 40, 40, 180)"))]
    pub background_color: (u8, u8, u8, u8),

    /// Filled progress color (R, G, B, A)
    #[derivative(Default(value = "(100, 200, 100, 200)"))]
    pub progress_color: (u8, u8, u8, u8),

    /// Vertical separator color (R, G, B, A)
    #[derivative(Default(value = "(255, 255, 255, 150)"))]
    pub separator_color: (u8, u8, u8, u8),

    /// Segment label text color (R, G, B, A)
    #[derivative(Default(value = "(255, 255, 255, 255)"))]
    pub text_color: (u8, u8, u8, u8),

    /// Segments sorted by start_time
    pub segments: Vec<ProgressSegment>,

    /// Font size in pixels (scaled for resolution)
    #[derivative(Default(value = "14"))]
    pub font_size: u32,

    /// Font file path (optional, uses default if None)
    pub font_path: Option<PathBuf>,

    /// Font file path (optional, uses default if None)
    pub font_family: Option<String>,

    /// Font style (e.g. "Bold", "Italic", "Bold Italic", "Normal")
    pub font_style: Option<String>,

    /// Separator width in pixels (scaled for resolution)
    #[derivative(Default(value = "2"))]
    pub separator_width: u32,
}

impl ProgressBarFilter {
    pub const NAME: &'static str = "progress bar";

    pub fn new() -> Self {
        Self {
            position_y: 0.0,
            padding: 4,
            margin_h: 0,
            background_color: (40, 40, 40, 180),
            progress_color: (100, 200, 100, 200),
            separator_color: (255, 255, 255, 150),
            text_color: (255, 255, 255, 255),
            segments: Vec::new(),
            font_size: 14,
            font_path: None,
            font_family: None,
            font_style: None,
            separator_width: 2,
        }
    }

    pub fn add_segment(&mut self, text: String, start_time: Duration) {
        self.segments.push(ProgressSegment { text, start_time });
        self.segments
            .sort_by(|a, b| a.start_time.cmp(&b.start_time));
    }

    fn calculate_progress(&self, timeline_offset: Duration, total_duration: Duration) -> f32 {
        if total_duration == Duration::ZERO {
            return 0.0;
        }
        let progress = timeline_offset.as_secs_f32() / total_duration.as_secs_f32();
        progress.clamp(0.0, 1.0)
    }

    fn draw_progress_bar(
        &self,
        buffer: &mut RgbaImage,
        current_offset: Duration,
        total_duration: Duration,
    ) -> Result<()> {
        let img_width = buffer.width();
        let img_height = buffer.height();
        let output_height = img_height;

        let margin_h = scale_pixel_for_height(self.margin_h, output_height);
        let separator_width = scale_pixel_for_height(self.separator_width, output_height);
        let padding = scale_pixel_for_height(self.padding, output_height);
        let font_size = scale_pixel_for_height(self.font_size, output_height);

        let mut font_system = None;
        let mut font_family = None;
        let mut font_weight = FontWeight::NORMAL;
        let mut font_fs_style = FontStyle::Normal;
        let mut swash_cache = None;
        let mut text_height = None;

        if let Some(font_path) = &self.font_path {
            let mut fs = FontSystem::new();
            let preferred_style = self.font_style.as_deref();
            if let Ok((family, weight, style)) =
                load_font(font_path, &mut fs, None, preferred_style)
            {
                if let Ok((_, height, _, _)) = calculate_text_size(
                    &mut fs,
                    "M",
                    font_size as f32,
                    &family,
                    weight,
                    style,
                    None,
                ) {
                    font_system = Some(fs);
                    font_family = Some(family);
                    font_weight = weight;
                    font_fs_style = style;
                    swash_cache = Some(SwashCache::new());
                    text_height = Some(height);
                }
            }
        }

        let bar_height = match text_height {
            Some(h) => h + 2 * padding,
            None => padding.max(4),
        };

        let bar_y = if self.position_y == 0.0 {
            img_height - bar_height
        } else {
            ((1.0 - self.position_y) * img_height as f32 - bar_height as f32 / 2.0) as u32
        };

        let bar_width = img_width - 2 * margin_h;
        let bar_x = margin_h;

        let progress = self.calculate_progress(current_offset, total_duration);
        let progress_width = (bar_width as f32 * progress) as u32;

        self.draw_horizontal_bar(
            buffer,
            bar_x,
            bar_y,
            bar_width,
            bar_height,
            self.background_color,
        );

        if progress_width > 0 {
            self.draw_horizontal_bar(
                buffer,
                bar_x,
                bar_y,
                progress_width,
                bar_height,
                self.progress_color,
            );
        }

        self.draw_vertical_separator(
            buffer,
            bar_x,
            bar_y,
            bar_height,
            separator_width,
            self.separator_color,
        );

        let separator_positions: Vec<u32> = self
            .segments
            .iter()
            .map(|segment| {
                let segment_progress = if total_duration == Duration::ZERO {
                    0.0
                } else {
                    segment.start_time.as_secs_f32() / total_duration.as_secs_f32()
                };
                bar_x + (bar_width as f32 * segment_progress) as u32
            })
            .collect();

        // Draw separators at each segment start position
        for &separator_x in &separator_positions {
            if separator_x >= bar_x && separator_x <= bar_x + bar_width {
                self.draw_vertical_separator(
                    buffer,
                    separator_x,
                    bar_y,
                    bar_height,
                    separator_width,
                    self.separator_color,
                );
            }
        }

        // Draw end separator if segments exist
        if !self.segments.is_empty() {
            self.draw_vertical_separator(
                buffer,
                bar_x + bar_width - separator_width / 2,
                bar_y,
                bar_height,
                separator_width,
                self.separator_color,
            );
        }

        // Draw text for each segment in its correct region
        for (i, segment) in self.segments.iter().enumerate() {
            if segment.text.is_empty() || font_system.is_none() {
                continue;
            }

            // Each segment's text is drawn from its start_time to the next segment's start_time
            // If first segment's start_time > 0, the region from 0 to first start_time has no text
            let region_start = separator_positions[i];
            let region_end = if i == self.segments.len() - 1 {
                bar_x + bar_width
            } else {
                separator_positions[i + 1]
            };

            self.draw_text_label_in_bar(
                buffer,
                &mut font_system,
                &mut swash_cache,
                &font_family,
                font_weight,
                font_fs_style,
                region_start,
                region_end,
                bar_y,
                bar_height,
                text_height.unwrap_or(font_size),
                &segment.text,
                font_size,
                self.text_color,
            );
        }

        Ok(())
    }

    fn draw_horizontal_bar(
        &self,
        buffer: &mut RgbaImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: (u8, u8, u8, u8),
    ) {
        let rgba = Rgba([color.0, color.1, color.2, color.3]);

        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;

                if px >= buffer.width() || py >= buffer.height() {
                    continue;
                }

                self.blend_pixel(buffer, px, py, &rgba);
            }
        }
    }

    fn draw_vertical_separator(
        &self,
        buffer: &mut RgbaImage,
        x: u32,
        y: u32,
        height: u32,
        width: u32,
        color: (u8, u8, u8, u8),
    ) {
        let rgba = Rgba([color.0, color.1, color.2, color.3]);

        for dy in 0..height {
            for dx in 0..width {
                let px = x.saturating_sub(width / 2) + dx;
                let py = y + dy;

                if px >= buffer.width() || py >= buffer.height() {
                    continue;
                }

                self.blend_pixel(buffer, px, py, &rgba);
            }
        }
    }

    fn draw_text_label_in_bar(
        &self,
        buffer: &mut RgbaImage,
        font_system: &mut Option<FontSystem>,
        swash_cache: &mut Option<SwashCache>,
        font_family: &Option<String>,
        font_weight: FontWeight,
        font_fs_style: FontStyle,
        region_start: u32,
        region_end: u32,
        bar_y: u32,
        bar_height: u32,
        text_height: u32,
        text: &str,
        font_size: u32,
        color: (u8, u8, u8, u8),
    ) {
        // Get font system, family, and cache
        let fs = match font_system.as_mut() {
            Some(fs) => fs,
            None => return,
        };
        let family = match font_family.as_ref() {
            Some(f) => f,
            None => return,
        };
        let cache = match swash_cache.as_mut() {
            Some(c) => c,
            None => return,
        };

        // Create a single Buffer and shape it (used for both bounds calculation and drawing)
        let metrics = Metrics {
            font_size: font_size as f32,
            line_height: font_size as f32 * 1.2,
        };
        let attrs = Attrs::new()
            .family(Family::Name(family))
            .weight(font_weight)
            .style(font_fs_style);

        let mut text_buffer = Buffer::new(fs, metrics);
        text_buffer.set_text(text, &attrs, Shaping::Basic, None);
        text_buffer.shape_until_scroll(fs, false);

        // Calculate visual bounds from the same Buffer
        let mut min_x: Option<i32> = None;
        let mut max_x: Option<i32> = None;

        for run in text_buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(glyph_img) = cache.get_image(fs, physical.cache_key) {
                    let glyph_start = physical.x + glyph_img.placement.left;
                    let glyph_end =
                        physical.x + glyph_img.placement.left + glyph_img.placement.width as i32;
                    min_x = Some(min_x.unwrap_or(glyph_start).min(glyph_start));
                    max_x = Some(max_x.unwrap_or(glyph_end).max(glyph_end));
                }
            }
        }

        let min_x = min_x.unwrap_or(0);
        let max_x = max_x.unwrap_or(0);
        let visual_width = (max_x - min_x) as u32;

        if visual_width == 0 {
            return;
        }

        let region_width = region_end - region_start;

        // Calculate centering offset
        let center_x = if visual_width <= region_width {
            let region_center = region_start as i32 + (region_width / 2) as i32;
            let text_visual_center = min_x + (visual_width as i32 / 2);
            region_center - text_visual_center
        } else {
            region_start as i32 - min_x
        };

        // Calculate baseline position
        let (_, _, baseline_offset, _) = match calculate_text_size(
            fs,
            text,
            font_size as f32,
            family,
            font_weight,
            font_fs_style,
            None,
        ) {
            Ok(result) => result,
            Err(_) => return,
        };

        let baseline_y = bar_y as i32 + ((bar_height - text_height) / 2) as i32 + baseline_offset;
        let rgba = Rgba([color.0, color.1, color.2, color.3]);

        // Draw glyphs directly from the pre-shaped Buffer (not creating a new one)
        for run in text_buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(glyph_img) = cache.get_image(fs, physical.cache_key) {
                    let glyph_x = center_x + physical.x;
                    let glyph_y = baseline_y + physical.y;
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

impl GlobalFilter for ProgressBarFilter {
    crate::impl_default_global_filter!(ProgressBarFilter);

    fn apply(&self, data: &mut GlobalFilterData) -> Result<()> {
        self.draw_progress_bar(&mut data.image, data.timeline_offset, data.total_duration)
    }
}
