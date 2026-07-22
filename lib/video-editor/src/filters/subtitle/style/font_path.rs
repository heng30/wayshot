use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;
use std::path::PathBuf;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct FontPathFilter {
    pub font_path: PathBuf,
    pub font_family: String,
    pub font_style: String,
}

impl FontPathFilter {
    pub const NAME: &'static str = "font path";

    pub fn new(font_path: PathBuf, font_family: String, font_style: String) -> Self {
        Self {
            font_path,
            font_family,
            font_style,
        }
    }
}

impl SubtitleFilter for FontPathFilter {
    crate::impl_default_subtitle_filter!(FontPathFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.font_path = self.font_path.clone();
        style.font_family = self.font_family.clone();
        style.font_style = self.font_style.clone();
    }
}
