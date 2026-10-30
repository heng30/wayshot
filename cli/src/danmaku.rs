use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Subcommand)]
pub enum Cmd {
    /// 下载 B 站视频弹幕
    Fetch {
        /// 视频 BV 号，例如 BV1xx411c7mD
        bvid: String,
        /// 分 P 序号
        #[arg(long, default_value_t = 1)]
        page: i32,
        /// 最多下载多少条
        #[arg(long, default_value_t = 5000)]
        limit: usize,
        /// 保存为 JSON 的路径，缺省不保存
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Fetch {
            bvid,
            page,
            limit,
            output,
        } => fetch(bvid, *page, *limit, output.as_deref()),
    }
}

fn fetch(bvid: &str, page: i32, limit: usize, output: Option<&Path>) -> Result<Value> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))?;

    let items = runtime
        .block_on(bili_danmaku::get_all_danmaku_with_limit(
            bvid,
            Some(page),
            limit,
            Duration::from_secs(15),
        ))
        .map_err(|e| anyhow!("获取弹幕失败: {e}"))?;

    let danmaku: Vec<Value> = items
        .iter()
        .map(|d| {
            json!({
                "time_ms": d.progress,
                "mode": d.mode,
                "color": format!("#{:06x}", d.color),
                "text": d.content,
                "weight": d.weight,
            })
        })
        .collect();

    if let Some(path) = output {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&danmaku)?)?;
    }

    Ok(json!({
        "bvid": bvid,
        "page": page,
        "count": danmaku.len(),
        "path": output.map(|p| p.display().to_string()),
        "danmaku": danmaku,
    }))
}
