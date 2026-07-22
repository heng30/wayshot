use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct MarginVerticalFilter {
    #[derivative(Default(value = "Some(30)"))]
    pub margin: Option<u32>,
}

impl MarginVerticalFilter {
    pub const NAME: &'static str = "margin vertical";

    pub fn new(margin: Option<u32>) -> Self {
        Self { margin }
    }
}

impl SubtitleFilter for MarginVerticalFilter {
    crate::impl_default_subtitle_filter!(MarginVerticalFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.margin_vertical = self.margin;
    }
}

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct MarginHorizontalFilter {
    #[derivative(Default(value = "None"))]
    pub margin: Option<u32>,
}

impl MarginHorizontalFilter {
    pub const NAME: &'static str = "margin_horizontal";

    pub fn new(margin: i32) -> Self {
        Self {
            margin: if margin >= 0 {
                Some(margin as u32)
            } else {
                None
            },
        }
    }
}

impl SubtitleFilter for MarginHorizontalFilter {
    crate::impl_default_subtitle_filter!(MarginHorizontalFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.margin_horizontal = self.margin;
    }
}
