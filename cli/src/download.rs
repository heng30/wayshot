use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use downloader::{DownloadState, Downloader};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum Cmd {
    /// 下载 URL 到本地文件
    Url {
        url: String,
        #[arg(short, long)]
        output: PathBuf,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Url { url, output } => url_download(url, output),
    }
}

fn url_download(url: &str, output: &Path) -> Result<Value> {
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建目录: {}", parent.display()))?;
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))?;

    let downloader = Downloader::new(url.to_string(), output.to_path_buf());
    let state = runtime
        .block_on(downloader.start(|_, _, _| {}))
        .map_err(|e| anyhow!("下载失败: {e}"))?;

    let state = match state {
        DownloadState::Finsished => "finished",
        DownloadState::Cancelled => "cancelled",
        DownloadState::Incompleted => "incompleted",
    };

    let size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);

    Ok(json!({
        "path": output.display().to_string(),
        "state": state,
        "bytes": size,
        "url": url,
    }))
}
