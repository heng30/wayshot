pub mod alignment;
pub mod border;
pub mod colors;
pub mod font_path;
pub mod font_size;
pub mod margin;
pub mod padding;
pub mod text_alignment;

pub use alignment::AlignmentFilter;
pub use border::{BorderRadiusFilter, OutlineWidthFilter};
pub use colors::{BackgroundColorFilter, OutlineColorFilter, PrimaryColorFilter};
pub use font_path::FontPathFilter;
pub use font_size::FontSizeFilter;
pub use margin::{MarginHorizontalFilter, MarginVerticalFilter};
pub use padding::PaddingFilter;
pub use text_alignment::{TextAlignment, TextAlignmentFilter};

use crate::filters::SubtitleFilter;
use image::Rgba;
use std::path::PathBuf;

pub const BASE_WIDTH: u32 = 1920;
pub const BASE_HEIGHT: u32 = 1080;

pub fn all_filter_names() -> &'static [&'static str] {
    &[
        AlignmentFilter::NAME,
        BackgroundColorFilter::NAME,
        BorderRadiusFilter::NAME,
        FontPathFilter::NAME,
        FontSizeFilter::NAME,
        MarginHorizontalFilter::NAME,
        MarginVerticalFilter::NAME,
        OutlineColorFilter::NAME,
        OutlineWidthFilter::NAME,
        PaddingFilter::NAME,
        PrimaryColorFilter::NAME,
        TextAlignmentFilter::NAME,
    ]
}

#[derive(Debug, Clone, derivative::Derivative, derive_setters::Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct SubtitleStyle {
    #[derivative(Default(value = "20"))]
    pub font_size: u32,

    pub font_path: PathBuf,
    pub font_family: String,
    pub font_style: String,
    pub background_color: Option<Rgba<u8>>,

    #[derivative(Default(value = "Some(Rgba([255, 255, 255, 255]))"))]
    pub primary_color: Option<Rgba<u8>>,

    #[derivative(Default(value = "Some(Rgba([0, 0, 0, 255]))"))]
    pub outline_color: Option<Rgba<u8>>,

    #[derivative(Default(value = "Some(2)"))]
    pub outline_width: Option<u32>,

    #[derivative(Default(value = "Some(0)"))]
    pub border_radius: Option<u32>,

    // Text alignment: 1-9 (1=bottom-left, 2=bottom-center, 3=bottom-right, etc.)
    #[derivative(Default(value = "Some(2)"))]
    pub alignment: Option<u32>,

    #[derivative(Default(value = "Some(30)"))]
    pub margin_vertical: Option<u32>,

    #[derivative(Default(value = "None"))]
    pub margin_horizontal: Option<u32>,

    #[derivative(Default(value = "Some(4)"))]
    pub padding: Option<u32>,

    pub border_width: Option<u32>,

    pub border_color: Option<Rgba<u8>>,

    pub text_alignment: TextAlignment,
}

impl SubtitleStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_filters(&mut self, filters: &[Box<dyn SubtitleFilter>]) {
        for filter in filters {
            filter.apply(self);
        }
    }

    // Scale all pixel values for the target resolution. Base resolution is 1080p (1920x1080).
    pub fn scaled_for_resolution(&self, _output_width: u32, output_height: u32) -> Self {
        let mut scaled = self.clone();
        scaled.font_size = scale_pixel_for_height(self.font_size, output_height);
        scaled.outline_width = self
            .outline_width
            .map(|v| scale_pixel_for_height(v, output_height));
        scaled.border_radius = self
            .border_radius
            .map(|v| scale_pixel_for_height(v, output_height));
        scaled.margin_vertical = self
            .margin_vertical
            .map(|v| scale_pixel_for_height(v, output_height));
        scaled.margin_horizontal = self
            .margin_horizontal
            .map(|v| scale_pixel_for_height(v, output_height));
        scaled.padding = self
            .padding
            .map(|v| scale_pixel_for_height(v, output_height));
        scaled.border_width = self
            .border_width
            .map(|v| scale_pixel_for_height(v, output_height));
        scaled
    }
}

pub fn scale_pixel_for_height(value: u32, target_height: u32) -> u32 {
    let scale = target_height as f32 / BASE_HEIGHT as f32;
    (value as f32 * scale).round() as u32
}

pub fn scale_pixel_f32_for_height(value: f32, target_height: u32) -> f32 {
    let scale = target_height as f32 / BASE_HEIGHT as f32;
    value * scale
}
