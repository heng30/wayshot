use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Subcommand)]
pub enum Cmd {
    /// 复制文本到剪贴板
    SetText { text: String },

    /// 读取剪贴板文本
    GetText,

    /// 复制图片文件到剪贴板
    SetImage { input: PathBuf },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::SetText { text } => set_text(text),
        Cmd::GetText => get_text(),
        Cmd::SetImage { input } => set_image(input),
    }
}

fn set_text(text: &str) -> Result<Value> {
    if run_cmd("wl-copy", &[text]).is_ok() {
        return Ok(json!({ "copied": true }));
    }

    let mut clipboard = arboard::Clipboard::new().map_err(|e| anyhow!("打开剪贴板失败: {e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| anyhow!("复制失败: {e}"))?;
    Ok(json!({ "copied": true }))
}

fn get_text() -> Result<Value> {
    if let Ok(output) = std::process::Command::new("wl-paste").output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        return Ok(json!({ "text": text }));
    }

    let mut clipboard = arboard::Clipboard::new().map_err(|e| anyhow!("打开剪贴板失败: {e}"))?;
    let text = clipboard
        .get_text()
        .map_err(|e| anyhow!("读取剪贴板失败: {e}"))?;
    Ok(json!({ "text": text }))
}

fn set_image(input: &Path) -> Result<Value> {
    let image = image::open(input).map_err(|e| anyhow!("无法打开图片 {}: {e}", input.display()))?;
    let rgba = image.to_rgba8();

    let mut clipboard = arboard::Clipboard::new().map_err(|e| anyhow!("打开剪贴板失败: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: rgba.into_raw().into(),
        })
        .map_err(|e| anyhow!("复制图片失败: {e}"))?;

    Ok(json!({ "copied": true, "path": input.display().to_string() }))
}

fn run_cmd(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{program} 执行失败"))
    }
}
