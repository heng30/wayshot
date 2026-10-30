use anyhow::{Result, anyhow};
use clap::Subcommand;
use imagedl_rs::{ImageClient, ImageFormat, ImageInfo};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum Cmd {
    /// 在多个图片来源搜索网络图片
    Search {
        keyword: String,
        /// 来源，逗号分隔（缺省 bing,baidu）
        #[arg(long)]
        sources: Option<String>,
        /// 每个来源最多取多少张
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// 结果与下载目录
        #[arg(long, default_value = "imagedl_outputs")]
        work_dir: PathBuf,
        /// 搜索后立即下载
        #[arg(long)]
        download: bool,
        /// 单个 HTTP 请求超时（秒）
        #[arg(long, default_value_t = 20)]
        timeout_secs: u64,
    },

    /// 列出可用的图片来源
    Sources,
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Search {
            keyword,
            sources,
            limit,
            work_dir,
            download,
            timeout_secs,
        } => search(
            keyword,
            sources.as_deref(),
            *limit,
            work_dir,
            *download,
            *timeout_secs,
        ),
        Cmd::Sources => {
            let client = ImageClient::builder()
                .with_builtin_sources()
                .build()
                .map_err(|e| anyhow!("初始化失败: {e}"))?;
            Ok(json!({ "sources": client.source_names() }))
        }
    }
}

fn search(
    keyword: &str,
    sources: Option<&str>,
    limit: usize,
    work_dir: &Path,
    download: bool,
    timeout_secs: u64,
) -> Result<Value> {
    let source_names = source_names(sources);
    let client = build_client(limit, work_dir, timeout_secs)?;
    let runtime = runtime()?;
    let results = runtime.block_on(client.search(keyword, &source_names));

    let mut found: HashMap<String, Vec<ImageInfo>> = HashMap::new();
    let mut source_states = Vec::new();
    for (name, result) in &results {
        match result {
            imagedl_rs::SearchResult::Ok(images) => {
                source_states.push(json!({ "source": name, "count": images.len() }));
                if !images.is_empty() {
                    found.insert(name.clone(), images.clone());
                }
            }
            imagedl_rs::SearchResult::Err(err) => {
                source_states.push(json!({ "source": name, "error": err }));
            }
        }
    }

    let files = if download && !found.is_empty() {
        download_images(&runtime, &client, &found, work_dir)?
    } else {
        Vec::new()
    };

    Ok(json!({
        "keyword": keyword,
        "sources": source_states,
        "downloaded": files.len(),
        "files": files,
    }))
}

fn source_names(sources: Option<&str>) -> Vec<&str> {
    sources
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec!["bing", "baidu"])
}

fn build_client(limit: usize, work_dir: &Path, timeout_secs: u64) -> Result<ImageClient> {
    ImageClient::builder()
        .with_builtin_sources()
        .search_limits(limit)
        .work_dir(work_dir)
        .timeout(std::time::Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|e| anyhow!("初始化失败: {e}"))
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))
}

fn download_images(
    runtime: &tokio::runtime::Runtime,
    client: &ImageClient,
    found: &HashMap<String, Vec<ImageInfo>>,
    work_dir: &Path,
) -> Result<Vec<Value>> {
    std::fs::create_dir_all(work_dir)
        .map_err(|e| anyhow!("无法创建目录 {}: {e}", work_dir.display()))?;

    let mut files = Vec::new();
    for downloaded in runtime.block_on(client.download_all(found)) {
        let info = &downloaded.image_info;
        let name = info
            .save_name
            .clone()
            .unwrap_or_else(|| info.identifier.clone());
        let ext = match &info.ext {
            Some(ext) => ext.as_str(),
            None => extension_of(downloaded.format),
        };
        let path = work_dir.join(format!("{name}.{ext}"));
        std::fs::write(&path, &downloaded.data)
            .map_err(|e| anyhow!("写入 {} 失败: {e}", path.display()))?;
        files.push(json!({
            "source": info.source,
            "path": path.display().to_string(),
            "description": info.description,
            "bytes": downloaded.data.len(),
        }));
    }
    Ok(files)
}

fn extension_of(format: ImageFormat) -> &'static str {
    format.extension()
}
