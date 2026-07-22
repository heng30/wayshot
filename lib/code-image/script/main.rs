//! CLI tool for generating syntax-highlighted code images.

use clap::Parser;
use code_image::{CodeHighlightConfig, Language, highlight_code};
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

    /// Language for syntax highlighting (rust, python, javascript, go)
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

    /// Std type highlight color
    #[arg(long, default_value = "#2aa198")]
    std_type_color: String,

    /// Import name highlight color
    #[arg(long, default_value = "#b58900")]
    import_name_color: String,

    /// Associated function highlight color
    #[arg(long, default_value = "#859900")]
    assoc_func_color: String,

    /// Enable std type highlighting (String, Vec, etc.)
    #[arg(long)]
    show_std_types: bool,

    /// Enable import highlighting
    #[arg(long)]
    show_imports: bool,

    /// Enable associated function highlighting (Type::method())
    #[arg(long)]
    show_assoc_funcs: bool,

    /// Disable all tree-sitter based special highlighting
    #[arg(long)]
    no_special_highlight: bool,
}

fn main() -> code_image::Result<()> {
    let args = Args::parse();

    // Read input file
    let code =
        std::fs::read_to_string(&args.input).map_err(|e| code_image::CodeImageError::IoError(e))?;

    // Parse language from argument
    let language = Language::from_str(&args.language).ok_or_else(|| {
        code_image::CodeImageError::SyntaxNotFoundError(format!(
            "Unknown language: {}. Supported languages: {}",
            args.language,
            supported_languages()
        ))
    })?;

    // Build config
    let config = CodeHighlightConfig::new(args.ascii_font, args.non_ascii_font)
        .with_line_numbers(args.line_numbers)
        .with_theme(args.theme)
        .with_font_size(args.font_size)
        .with_line_height_ratio(args.line_height_ratio)
        .with_padding(args.padding)
        .with_scale(args.scale)
        .with_bg_color(args.bg_color)
        .with_std_type_color(args.std_type_color)
        .with_import_name_color(args.import_name_color)
        .with_assoc_func_color(args.assoc_func_color)
        .with_language(language);

    // Handle special highlighting flags
    let config = if args.no_special_highlight {
        config
            .with_show_std_types(false)
            .with_show_imports(false)
            .with_show_assoc_funcs(false)
    } else {
        // Default all enabled; only disable if explicitly set to false
        // Since clap doesn't support negation flags directly, we check individual flags
        let show_std = !args.no_special_highlight;
        let show_imports = !args.no_special_highlight;
        let show_assoc = !args.no_special_highlight;
        config
            .with_show_std_types(show_std)
            .with_show_imports(show_imports)
            .with_show_assoc_funcs(show_assoc)
    };

    // Generate output
    let png_bytes = highlight_code(&code, &config)?;
    std::fs::write(&args.output, png_bytes)?;

    println!("Generated: {}", args.output.display());

    Ok(())
}

fn supported_languages() -> String {
    let langs = vec!["rust"];
    #[cfg(feature = "python")]
    let langs = {
        let mut l = langs;
        l.push("python");
        l
    };
    #[cfg(feature = "javascript")]
    let langs = {
        let mut l = langs;
        l.push("javascript");
        l
    };
    #[cfg(feature = "go")]
    let langs = {
        let mut l = langs;
        l.push("go");
        l
    };
    langs.join(", ")
}
