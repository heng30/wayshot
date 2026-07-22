use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct OutlineWidthFilter {
    #[derivative(Default(value = "Some(2)"))]
    pub width: Option<u32>,
}

impl OutlineWidthFilter {
    pub const NAME: &'static str = "outline width";

    pub fn new(width: i32) -> Self {
        Self {
            width: if width > 0 { Some(width as u32) } else { None },
        }
    }
}

impl SubtitleFilter for OutlineWidthFilter {
    crate::impl_default_subtitle_filter!(OutlineWidthFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.outline_width = self.width;
    }
}

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct BorderRadiusFilter {
    #[derivative(Default(value = "Some(0)"))]
    pub radius: Option<u32>,
}

impl BorderRadiusFilter {
    pub const NAME: &'static str = "border radius";

    pub fn new(radius: i32) -> Self {
        Self {
            radius: if radius > 0 {
                Some(radius as u32)
            } else {
                None
            },
        }
    }
}

impl SubtitleFilter for BorderRadiusFilter {
    crate::impl_default_subtitle_filter!(BorderRadiusFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.border_radius = self.radius;
    }
}
