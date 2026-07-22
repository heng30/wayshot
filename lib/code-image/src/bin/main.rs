//! CLI tool for generating syntax-highlighted code images.

use clap::Parser;
use code_image::{CodeHighlightConfig, Language, TerminalStyle, highlight_code};
use std::path::PathBuf;

/// Generate syntax-highlighted code images from source files.
#[derive(Parser)]
#[command(name = "code-image")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input source file to highlight
    #[arg(value_name = "INPUT_FILE")]
    input: PathBuf,

    /// Output image file path (PNG format)
    #[arg(short, long, value_name = "OUTPUT_FILE")]
    output: PathBuf,

    /// ASCII font file path (e.g., JetBrains Mono)
    #[arg(short = 'a', long, value_name = "FONT_PATH")]
    ascii_font: PathBuf,

    /// Non-ASCII font file path (e.g., Source Han Sans for Chinese)
    #[arg(short = 'n', long, value_name = "FONT_PATH")]
    non_ascii_font: PathBuf,

    /// Language for syntax highlighting (rust, python, javascript, go, c, cpp, java, ruby, php, html, css, json, yaml, markdown, shell, sql, lua, scala)
    #[arg(short = 'L', long, default_value = "rust")]
    language: String,

    /// Show line numbers
    #[arg(short = 'l', long)]
    line_numbers: bool,

    /// Syntax theme name
    #[arg(short = 't', long, default_value = "Solarized (dark)")]
    theme: String,

    /// Font size in pixels
    #[arg(short = 's', long, default_value = "16.0")]
    font_size: f64,

    /// Line height ratio (multiplier of font size)
    #[arg(long, default_value = "1.5")]
    line_height_ratio: f64,

    /// Padding around code in pixels
    #[arg(short = 'p', long, default_value = "20.0")]
    padding: f64,

    /// Resolution scale factor for high-DPI output
    #[arg(short = 'r', long, default_value = "2.0")]
    scale: f64,

    /// Custom background color (overrides theme default)
    #[arg(short = 'b', long, value_name = "COLOR")]
    bg_color: Option<String>,

    /// Terminal window style (macos, macos-dark, windows, windows-dark, gnome, iterm)
    #[arg(long, value_name = "STYLE")]
    terminal: Option<String>,

    /// Terminal window title text
    #[arg(long, value_name = "TITLE")]
    terminal_title: Option<String>,
}

fn main() -> code_image::Result<()> {
    let args = Args::parse();

    // Read input file
    let code =
        std::fs::read_to_string(&args.input).map_err(|e| code_image::CodeImageError::IoError(e))?;

    // Parse language from argument
    let language = Language::from_str(&args.language).ok_or_else(|| {
        code_image::CodeImageError::SyntaxNotFoundError(format!(
            "Unknown language: {}. Supported: rust, python, javascript, go, c, cpp, java, ruby, php, html, css, json, yaml, markdown, shell, sql, lua, scala",
            args.language
        ))
    })?;

    // Parse terminal style from argument
    let terminal = args.terminal.as_ref().and_then(|s| TerminalStyle::from_str(s));
    if args.terminal.is_some() && terminal.is_none() {
        return Err(code_image::CodeImageError::SyntaxNotFoundError(format!(
            "Unknown terminal style: {}. Supported: macos, macos-dark, windows, windows-dark, gnome, iterm",
            args.terminal.as_ref().unwrap()
        )));
    }

    // Build config
    let config = CodeHighlightConfig::new(args.ascii_font, args.non_ascii_font)
        .with_line_numbers(args.line_numbers)
        .with_theme(args.theme)
        .with_font_size(args.font_size)
        .with_line_height_ratio(args.line_height_ratio)
        .with_padding(args.padding)
        .with_scale(args.scale)
        .with_bg_color(args.bg_color)
        .with_language(language)
        .with_terminal(terminal)
        .with_terminal_title(args.terminal_title);

    // Generate output
    let png_bytes = highlight_code(&code, &config)?;
    std::fs::write(&args.output, png_bytes)?;

    println!("Generated: {}", args.output.display());

    Ok(())
}
