use crate::filters::{subtitle::style::SubtitleStyle, traits::SubtitleFilter};
use derivative::Derivative;
use derive_setters::Setters;
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextAlignment {
    #[default]
    Center,
    Left,
    Right,
}

impl fmt::Display for TextAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextAlignment::Left => write!(f, "Left"),
            TextAlignment::Center => write!(f, "Center"),
            TextAlignment::Right => write!(f, "Right"),
        }
    }
}

impl FromStr for TextAlignment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(TextAlignment::Left),
            "Center" => Ok(TextAlignment::Center),
            "Right" => Ok(TextAlignment::Right),
            other => Err(format!("Invalid TextAlignment: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Derivative, Setters, serde::Serialize, serde::Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct TextAlignmentFilter {
    pub alignment: Option<TextAlignment>,
}

impl TextAlignmentFilter {
    pub const NAME: &'static str = "text alignment";

    pub fn new(alignment: TextAlignment) -> Self {
        Self {
            alignment: Some(alignment),
        }
    }

    pub fn left() -> Self {
        Self::new(TextAlignment::Left)
    }

    pub fn center() -> Self {
        Self::new(TextAlignment::Center)
    }

    pub fn right() -> Self {
        Self::new(TextAlignment::Right)
    }
}

impl SubtitleFilter for TextAlignmentFilter {
    crate::impl_default_subtitle_filter!(TextAlignmentFilter);

    fn apply(&self, style: &mut SubtitleStyle) {
        style.text_alignment = self.alignment.unwrap_or_default();
    }
}
