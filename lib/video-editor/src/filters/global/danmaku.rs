use crate::{
    Result,
    filters::{
        subtitle::{
            renderer::{create_text_image_with_cache, load_font},
            style::{
                SubtitleStyle, TextAlignment, scale_pixel_f32_for_height, scale_pixel_for_height,
            },
        },
        traits::{GlobalFilter, GlobalFilterData},
    },
};
use cosmic_text::{FontSystem, SwashCache};
use draw_utils::composite_image;
use image::RgbaImage;
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
pub enum DanmakuDistributionMode {
    StartDense = 0,
    #[default]
    Uniform = 1,
    EndDense = 2,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DanmakuItem {
    pub text: String,
}

impl Default for DanmakuItem {
    fn default() -> Self {
        Self {
            text: String::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct DanmakuStyle {
    pub font_path: PathBuf,
    pub font_family: String,
    pub font_style: String,
    #[derivative(Default(value = "36"))]
    pub font_size: u32,
    #[derivative(Default(value = "(255, 255, 255, 255)"))]
    pub color: (u8, u8, u8, u8),
    #[derivative(Default(value = "1"))]
    pub outline_width: u32,
    #[derivative(Default(value = "(0, 0, 0, 255)"))]
    pub outline_color: (u8, u8, u8, u8),
    #[derivative(Default(value = "8"))]
    pub line_spacing: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct DanmakuSegment {
    pub start_time: Duration,
    pub end_time: Duration,
    #[derivative(Default(value = "200.0"))]
    pub scroll_speed: f32,
    pub distribution: DanmakuDistributionMode,
    pub track_count: u32,
    pub track_distribution: DanmakuDistributionMode,
    pub position: f32,
    pub items: Vec<DanmakuItem>,
    pub style: DanmakuStyle,
}

impl DanmakuSegment {
    fn duration_secs(&self) -> f32 {
        (self.end_time - self.start_time).as_secs_f32()
    }

    fn compute_start_times(&self) -> Vec<f32> {
        let n = self.items.len();
        if n == 0 {
            return Vec::new();
        }
        let d = self.duration_secs();
        match self.distribution {
            DanmakuDistributionMode::Uniform => (0..n).map(|i| d * i as f32 / n as f32).collect(),
            DanmakuDistributionMode::StartDense => {
                (0..n).map(|i| d * (i as f32 / n as f32).powi(2)).collect()
            }
            DanmakuDistributionMode::EndDense => (0..n)
                .map(|i| d * (1.0 - (1.0 - i as f32 / n as f32).powi(2)))
                .collect(),
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
pub struct DanmakuFilter {
    pub segments: Vec<DanmakuSegment>,
    pub default_style: DanmakuStyle,
}

impl DanmakuFilter {
    pub const NAME: &'static str = "danmaku";

    pub fn new() -> Self {
        Self::default()
    }

    /// Convert a DanmakuStyle to a SubtitleStyle for reuse with the shared renderer.
    fn style_to_subtitle_style(style: &DanmakuStyle, output_height: u32) -> SubtitleStyle {
        let scaled_font_size = scale_pixel_for_height(style.font_size, output_height);
        let scaled_outline_width = scale_pixel_for_height(style.outline_width, output_height);

        SubtitleStyle::new()
            .with_font_path(style.font_path.clone())
            .with_font_family(style.font_family.clone())
            .with_font_style(style.font_style.clone())
            .with_font_size(scaled_font_size)
            .with_primary_color(Some(image::Rgba([
                style.color.0,
                style.color.1,
                style.color.2,
                style.color.3,
            ])))
            .with_outline_color(Some(image::Rgba([
                style.outline_color.0,
                style.outline_color.1,
                style.outline_color.2,
                style.outline_color.3,
            ])))
            .with_outline_width(Some(scaled_outline_width))
            .with_padding(Some(0))
            .with_border_width(Some(0))
            .with_border_radius(Some(0))
            .with_margin_vertical(Some(0))
            .with_margin_horizontal(None)
            .with_text_alignment(TextAlignment::Left)
    }

    fn draw_danmaku(
        &self,
        buffer: &mut RgbaImage,
        timeline_offset: Duration,
        output_height: u32,
    ) -> Result<()> {
        let img_width = buffer.width();
        let img_height = buffer.height();

        for segment in &self.segments {
            if timeline_offset < segment.start_time || timeline_offset > segment.end_time {
                continue;
            }

            let elapsed = (timeline_offset - segment.start_time).as_secs_f32();

            let scaled_font_size = scale_pixel_for_height(segment.style.font_size, output_height);
            let scaled_line_spacing =
                scale_pixel_for_height(segment.style.line_spacing, output_height);
            let scroll_speed = scale_pixel_f32_for_height(segment.scroll_speed, output_height);

            if segment.style.font_path.as_os_str().is_empty() {
                continue;
            }

            // Build SubtitleStyle once per segment (shared font settings)
            let subtitle_style = Self::style_to_subtitle_style(&segment.style, output_height);

            // Create FontSystem and SwashCache once per segment for batch rendering
            let mut font_system = FontSystem::new();
            let mut swash_cache = SwashCache::new();

            // Load font once to verify it's valid
            let preferred = if segment.style.font_family.is_empty() {
                None
            } else {
                Some(segment.style.font_family.as_str())
            };
            let preferred_style = if segment.style.font_style.is_empty() {
                None
            } else {
                Some(segment.style.font_style.as_str())
            };
            if load_font(
                &segment.style.font_path,
                &mut font_system,
                preferred,
                preferred_style,
            )
            .is_err()
            {
                continue;
            }

            // Track setup
            let line_height = scaled_font_size as f32 * 1.2;
            let track_height = line_height.ceil() as u32 + scaled_line_spacing;
            let num_tracks = if segment.track_count > 0 {
                segment.track_count.min(img_height / track_height)
            } else {
                img_height / track_height
            };
            if num_tracks == 0 || track_height == 0 {
                continue;
            }

            // Position: 0=top, 1=bottom — offset the track area vertically
            let position = segment.position.clamp(0.0, 1.0);
            let total_track_height = num_tracks as u32 * track_height;
            let max_y_offset = (img_height as i64 - total_track_height as i64).max(0) as u32;
            let y_offset = (position * max_y_offset as f32).round() as u32;

            // Compute start_times from distribution mode
            let start_times = segment.compute_start_times();

            // Track end times for assignment
            let mut track_end_times: Vec<f32> = vec![0.0; num_tracks as usize];

            for (i, item) in segment.items.iter().enumerate() {
                let item_start = start_times[i];
                let t = elapsed - item_start;
                if t < 0.0 {
                    continue;
                }

                let x = img_width as f32 - t * scroll_speed;

                // Render text image using the shared renderer
                let item_img = match create_text_image_with_cache(
                    &item.text,
                    &subtitle_style,
                    &mut font_system,
                    &mut swash_cache,
                ) {
                    Ok(img) => img,
                    Err(_) => continue,
                };

                let item_width = item_img.width();
                let item_height = item_img.height();

                if item_width == 0 || item_height == 0 || item.text.is_empty() {
                    continue;
                }

                // Assign track
                let track = assign_track(
                    &track_end_times,
                    item_start,
                    segment.track_distribution,
                    segment.track_count == 0,
                    i,
                );

                let item_width_f = item_width as f32;
                let duration = (item_width_f + img_width as f32) / scroll_speed;
                track_end_times[track as usize] = item_start + duration;

                // Skip if completely off-screen
                if x + item_width_f < 0.0 || x >= img_width as f32 {
                    continue;
                }

                let track_y = y_offset + track * track_height;

                composite_image(buffer, &item_img, x as i32, track_y as i32);
            }
        }

        Ok(())
    }
}

fn assign_track(
    track_end_times: &[f32],
    start_time: f32,
    distribution: DanmakuDistributionMode,
    auto_tracks: bool,
    item_index: usize,
) -> u32 {
    // Collect available tracks
    let available: Vec<usize> = track_end_times
        .iter()
        .enumerate()
        .filter(|&(_, &end_time)| end_time <= start_time)
        .map(|(i, _)| i)
        .collect();

    if !available.is_empty() {
        if auto_tracks {
            // In auto mode, pick a random available track to avoid regular patterns
            let hash = simple_hash(item_index);
            let idx = hash as usize % available.len();
            return available[idx] as u32;
        }
        return match distribution {
            DanmakuDistributionMode::StartDense | DanmakuDistributionMode::Uniform => {
                available[0] as u32
            }
            DanmakuDistributionMode::EndDense => *available.last().unwrap() as u32,
        };
    }

    // All tracks busy — pick the one that finishes earliest
    let (track, _) = track_end_times
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));
    track as u32
}

fn simple_hash(n: usize) -> u32 {
    // xorshift32 for fast pseudo-random from index
    let mut x = (n as u32).wrapping_add(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

impl GlobalFilter for DanmakuFilter {
    crate::impl_default_global_filter!(DanmakuFilter);

    fn apply(&self, data: &mut GlobalFilterData) -> Result<()> {
        let output_height = data.image.height();
        self.draw_danmaku(&mut data.image, data.timeline_offset, output_height)
    }

    fn apply_post_composite(&self) -> bool {
        true
    }
}
