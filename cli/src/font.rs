use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Cmd {
    /// 列出系统可用字体（族名 + 样式 + 文件路径）
    List {
        /// 只显示族名包含该关键字的字体
        #[arg(long)]
        query: Option<String>,
    },

    /// 查看字体文件的族名与样式
    Info { path: PathBuf },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::List { query } => list(query.as_deref()),
        Cmd::Info { path } => info(path),
    }
}

fn list(query: Option<&str>) -> Result<Value> {
    let fonts = video_editor::font::get_fonts_info().map_err(|e| anyhow!("读取字体失败: {e}"))?;

    let fonts: Vec<Value> = fonts
        .into_iter()
        .filter(|(family, _, style)| {
            query.is_none_or(|q| {
                family.to_lowercase().contains(&q.to_lowercase())
                    || style.to_lowercase().contains(&q.to_lowercase())
            })
        })
        .map(|(family, path, style)| {
            json!({
                "family": family,
                "style": style,
                "path": path.display().to_string(),
            })
        })
        .collect();

    Ok(json!({ "count": fonts.len(), "fonts": fonts }))
}

fn info(path: &PathBuf) -> Result<Value> {
    let (family, style) = video_editor::font::get_font_family_from_file(path)
        .map_err(|e| anyhow!("读取字体文件失败: {e}"))?;

    Ok(json!({
        "path": path.display().to_string(),
        "family": family,
        "style": style,
    }))
}
