pub mod config;
pub mod error;
pub mod render;
pub mod theme;

pub use config::CodeHighlightConfig;
pub use error::{CodeImageError, Result};
pub use render::{render_to_image, render_to_png, render_to_svg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Rust,
    Python,
    JavaScript,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    PHP,
    HTML,
    CSS,
    JSON,
    YAML,
    Markdown,
    Shell,
    SQL,
    Lua,
    Scala,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalStyle {
    #[default]
    MacOS,
    MacOSDark,
    Windows,
    WindowsDark,
    Gnome,
    ITerm,
}

impl TerminalStyle {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "macos" => Some(TerminalStyle::MacOS),
            "macos-dark" | "macosdark" => Some(TerminalStyle::MacOSDark),
            "windows" => Some(TerminalStyle::Windows),
            "windows-dark" | "windowsdark" => Some(TerminalStyle::WindowsDark),
            "gnome" => Some(TerminalStyle::Gnome),
            "iterm" | "iterm2" => Some(TerminalStyle::ITerm),
            _ => None,
        }
    }
}

impl Language {
    pub fn extension(&self) -> &'static str {
        match self {
            Language::Rust => "rs",
            Language::Python => "py",
            Language::JavaScript => "js",
            Language::Go => "go",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::Java => "java",
            Language::Ruby => "rb",
            Language::PHP => "php",
            Language::HTML => "html",
            Language::CSS => "css",
            Language::JSON => "json",
            Language::YAML => "yaml",
            Language::Markdown => "md",
            Language::Shell => "sh",
            Language::SQL => "sql",
            Language::Lua => "lua",
            Language::Scala => "scala",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Some(Language::Rust),
            "python" | "py" => Some(Language::Python),
            "javascript" | "js" => Some(Language::JavaScript),
            "typescript" | "ts" | "tsx" => Some(Language::JavaScript), // fallback to JS
            "go" | "golang" => Some(Language::Go),
            "c" => Some(Language::C),
            "cpp" | "c++" | "cc" | "cxx" | "hpp" => Some(Language::Cpp),
            "java" => Some(Language::Java),
            "kotlin" | "kt" | "kts" => Some(Language::Java), // fallback to Java (similar syntax)
            "ruby" | "rb" => Some(Language::Ruby),
            "php" => Some(Language::PHP),
            "html" => Some(Language::HTML),
            "css" => Some(Language::CSS),
            "json" => Some(Language::JSON),
            "yaml" | "yml" => Some(Language::YAML),
            "markdown" | "md" => Some(Language::Markdown),
            "shell" | "sh" | "bash" | "zsh" => Some(Language::Shell),
            "sql" => Some(Language::SQL),
            "toml" => Some(Language::JSON), // fallback to JSON (similar format)
            "lua" => Some(Language::Lua),
            "swift" => Some(Language::Cpp), // fallback to C++ (similar syntax)
            "scala" => Some(Language::Scala),
            _ => None,
        }
    }
}

pub fn highlight_code(code: &str, config: &CodeHighlightConfig) -> Result<Vec<u8>> {
    render::render_to_png(code, config)
}

pub fn highlight_code_to_image(
    code: &str,
    config: &CodeHighlightConfig,
) -> Result<image::RgbaImage> {
    render::render_to_image(code, config)
}
