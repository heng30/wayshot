use crate::{Language, TerminalStyle};
use std::path::PathBuf;

#[derive(Debug, Clone, derivative::Derivative, derive_setters::Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct CodeHighlightConfig {
    #[derivative(Default(value = "false"))]
    pub line_numbers: bool,

    #[derivative(Default(value = "String::from(\"Solarized (dark)\")"))]
    pub theme: String,

    #[derivative(Default(value = "16.0"))]
    pub font_size: f64,

    #[derivative(Default(value = "1.5"))]
    pub line_height_ratio: f64,

    #[derivative(Default(value = "20.0"))]
    pub padding: f64,

    #[derivative(Default(value = "2.0"))]
    pub scale: f64,

    #[derivative(Default(value = "String::from(\"JetBrains Mono\")"))]
    pub ascii_font: String,

    #[derivative(Default(value = "String::from(\"Source Han Sans CN\")"))]
    pub non_ascii_font: String,

    pub ascii_font_path: PathBuf,

    pub non_ascii_font_path: PathBuf,

    pub bg_color: Option<String>,

    #[derivative(Default(value = "Language::default()"))]
    pub language: Language,

    pub terminal: Option<TerminalStyle>,

    pub terminal_title: Option<String>,
}

impl CodeHighlightConfig {
    pub fn new(ascii_font_path: PathBuf, non_ascii_font_path: PathBuf) -> Self {
        Self {
            ascii_font_path,
            non_ascii_font_path,
            ..Self::default()
        }
    }

    pub fn line_height(&self) -> f64 {
        self.font_size * self.line_height_ratio
    }
}
