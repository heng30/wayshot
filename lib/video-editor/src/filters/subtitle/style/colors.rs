use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;
use image::Rgba;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct PrimaryColorFilter {
    #[derivative(Default(value = "Some(Rgba([255, 255, 255, 255]))"))]
    #[serde(with = "crate::project::filters::color_serde")]
    pub color: Option<Rgba<u8>>,
}

impl PrimaryColorFilter {
    pub const NAME: &'static str = "primary color";

    pub fn new(color: Option<Rgba<u8>>) -> Self {
        Self { color }
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            color: Some(Rgba([r, g, b, a])),
        }
    }
}

impl SubtitleFilter for PrimaryColorFilter {
    crate::impl_default_subtitle_filter!(PrimaryColorFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.primary_color = self.color;
    }
}

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct OutlineColorFilter {
    #[derivative(Default(value = "Some(Rgba([0, 0, 0, 255]))"))]
    #[serde(with = "crate::project::filters::color_serde")]
    pub color: Option<Rgba<u8>>,
}

impl OutlineColorFilter {
    pub const NAME: &'static str = "outline_color";

    pub fn new(color: Option<Rgba<u8>>) -> Self {
        Self { color }
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            color: Some(Rgba([r, g, b, a])),
        }
    }
}

impl SubtitleFilter for OutlineColorFilter {
    crate::impl_default_subtitle_filter!(OutlineColorFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.outline_color = self.color;
    }
}

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct BackgroundColorFilter {
    #[serde(with = "crate::project::filters::color_serde")]
    pub color: Option<Rgba<u8>>,
}

impl BackgroundColorFilter {
    pub const NAME: &'static str = "background_color";

    pub fn new(color: Option<Rgba<u8>>) -> Self {
        Self { color }
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            color: Some(Rgba([r, g, b, a])),
        }
    }
}

impl SubtitleFilter for BackgroundColorFilter {
    crate::impl_default_subtitle_filter!(BackgroundColorFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.background_color = self.color;
    }
}
