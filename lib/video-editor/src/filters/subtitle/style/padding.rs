use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct PaddingFilter {
    #[derivative(Default(value = "Some(4)"))]
    pub padding: Option<u32>,
}

impl PaddingFilter {
    pub const NAME: &'static str = "padding";

    pub fn new(padding: i32) -> Self {
        Self {
            padding: if padding > 0 {
                Some(padding as u32)
            } else {
                None
            },
        }
    }
}

impl SubtitleFilter for PaddingFilter {
    crate::impl_default_subtitle_filter!(PaddingFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.padding = self.padding;
    }
}
