use thiserror::Error;

pub type Result<T> = std::result::Result<T, CodeImageError>;

#[derive(Debug, Error)]
pub enum CodeImageError {
    #[error("Failed to parse font: {0}")]
    FontParseError(String),

    #[error("Failed to load font file '{path}': {source}")]
    FontLoadError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse SVG: {0}")]
    SvgParseError(String),

    #[error("Failed to render PNG: {0}")]
    RenderError(String),

    #[error("Theme '{0}' not found")]
    ThemeNotFoundError(String),

    #[error("Syntax for extension '{0}' not found")]
    SyntaxNotFoundError(String),

    #[error("Failed to parse code with tree-sitter")]
    TreeSitterParseError,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

