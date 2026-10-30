use anyhow::{Context, Result, anyhow};
use code_image::{CodeHighlightConfig, Language, TerminalStyle};
use serde_json::json;
use std::path::PathBuf;

use crate::util;

#[derive(clap::Args)]
pub struct Args {
    /// 源码文件
    pub input: PathBuf,

    /// 输出图片路径（.png）或 .svg
    #[arg(short, long)]
    pub output: PathBuf,

    /// 语言，缺省按文件扩展名推断
    #[arg(short = 'L', long)]
    pub language: Option<String>,

    /// syntect 主题名
    #[arg(short, long, default_value = "Solarized (dark)")]
    pub theme: String,

    /// 显示行号
    #[arg(short = 'l', long)]
    pub line_numbers: bool,

    #[arg(long, default_value_t = 16.0)]
    pub font_size: f64,

    #[arg(long, default_value_t = 2.0)]
    pub scale: f64,

    #[arg(long, default_value_t = 20.0)]
    pub padding: f64,

    /// 终端窗口外框样式（macos/macos-dark/windows/windows-dark/gnome/iterm）
    #[arg(long)]
    pub terminal: Option<String>,

    /// ASCII 字体文件（必填，可用 `font list` 查找）
    #[arg(long)]
    pub ascii_font: PathBuf,

    /// 非 ASCII 字体文件（必填，可用 `font list` 查找）
    #[arg(long)]
    pub non_ascii_font: PathBuf,

    /// 自定义背景色（如 #1e1e1e）
    #[arg(long)]
    pub bg_color: Option<String>,
}

pub fn run(args: &Args) -> Result<serde_json::Value> {
    let code = std::fs::read_to_string(&args.input)
        .with_context(|| format!("无法读取源码文件: {}", args.input.display()))?;

    let language = match &args.language {
        Some(name) => Language::from_str(name).ok_or_else(|| anyhow!("不支持的语言: {name}"))?,
        None => args
            .input
            .extension()
            .and_then(|e| e.to_str())
            .and_then(Language::from_str)
            .unwrap_or_default(),
    };

    let mut config = CodeHighlightConfig::new(args.ascii_font.clone(), args.non_ascii_font.clone());
    config.ascii_font = font_family(&args.ascii_font);
    config.non_ascii_font = font_family(&args.non_ascii_font);
    config.language = language;
    config.theme = args.theme.clone();
    config.line_numbers = args.line_numbers;
    config.font_size = args.font_size;
    config.scale = args.scale;
    config.padding = args.padding;
    config.bg_color = args.bg_color.clone();
    config.terminal = match &args.terminal {
        Some(name) => {
            Some(TerminalStyle::from_str(name).ok_or_else(|| anyhow!("不支持的终端样式: {name}"))?)
        }
        None => None,
    };

    let svg_output = args.output.extension().and_then(|e| e.to_str()) == Some("svg");
    if svg_output {
        let svg =
            code_image::render_to_svg(&code, &config).map_err(|e| anyhow!("渲染失败: {e}"))?;
        std::fs::write(&args.output, svg)?;
        return Ok(json!({ "path": args.output.display().to_string(), "format": "svg" }));
    }

    let img = code_image::render_to_image(&code, &config).map_err(|e| anyhow!("渲染失败: {e}"))?;
    util::save_rgba(&img, &args.output)?;
    Ok(util::image_json(&args.output, &img))
}

/// 从字体文件读取族名（SVG 里引用的 family），不依赖 fontconfig。
fn font_family(path: &PathBuf) -> String {
    video_editor::font::get_font_family_from_file(path)
        .map(|(family, _style)| family)
        .unwrap_or_default()
}
