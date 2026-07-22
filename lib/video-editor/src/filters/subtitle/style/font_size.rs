use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct FontSizeFilter {
    #[derivative(Default(value = "20"))]
    pub font_size: u32,
}

impl FontSizeFilter {
    pub const NAME: &'static str = "font size";

    pub fn new(font_size: u32) -> Self {
        Self { font_size }
    }
}

impl SubtitleFilter for FontSizeFilter {
    crate::impl_default_subtitle_filter!(FontSizeFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.font_size = self.font_size;
    }
}
