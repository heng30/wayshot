use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct AlignmentFilter {
    #[derivative(Default(value = "Some(2)"))]
    pub alignment: Option<u32>,
}

impl AlignmentFilter {
    pub const NAME: &'static str = "alignment";

    pub fn new(alignment: Option<u32>) -> Self {
        Self { alignment }
    }

    pub fn bottom_left() -> Self {
        Self { alignment: Some(1) }
    }

    pub fn bottom_center() -> Self {
        Self { alignment: Some(2) }
    }

    pub fn bottom_right() -> Self {
        Self { alignment: Some(3) }
    }

    pub fn middle_left() -> Self {
        Self { alignment: Some(4) }
    }

    pub fn middle_center() -> Self {
        Self { alignment: Some(5) }
    }

    pub fn middle_right() -> Self {
        Self { alignment: Some(6) }
    }

    pub fn top_left() -> Self {
        Self { alignment: Some(7) }
    }

    pub fn top_center() -> Self {
        Self { alignment: Some(8) }
    }

    pub fn top_right() -> Self {
        Self { alignment: Some(9) }
    }
}

impl SubtitleFilter for AlignmentFilter {
    crate::impl_default_subtitle_filter!(AlignmentFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.alignment = self.alignment;
    }
}
