use anyhow::{Result, anyhow};
use clap::Subcommand;
use musicdl_rs::{DownloadContent, MusicClient, SongInfo};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Cmd {
    /// 在多个音乐平台搜索歌曲
    Search {
        keyword: String,
        /// 来源，逗号分隔（缺省 netease,kuwo,kugou,qianqian）
        #[arg(long)]
        sources: Option<String>,
        /// 每个来源最多取多少首
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// 结果与下载目录
        #[arg(long, default_value = "musicdl_outputs")]
        work_dir: PathBuf,
        /// 搜索后立即下载
        #[arg(long)]
        download: bool,
        /// 同时保存歌词/封面
        #[arg(long)]
        with_assets: bool,
        /// 单个 HTTP 请求超时（秒）
        #[arg(long, default_value_t = 20)]
        timeout_secs: u64,
    },

    /// 列出可用的音乐来源
    Sources,

    /// 解析歌单链接为歌曲列表
    Playlist {
        url: String,
        #[arg(long, default_value = "musicdl_outputs")]
        work_dir: PathBuf,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Search {
            keyword,
            sources,
            limit,
            work_dir,
            download,
            with_assets,
            timeout_secs,
        } => search(
            keyword,
            sources.as_deref(),
            *limit,
            work_dir,
            *download,
            *with_assets,
            *timeout_secs,
        ),
        Cmd::Sources => {
            let client = MusicClient::builder()
                .with_builtin_sources()
                .build()
                .map_err(|e| anyhow!("初始化失败: {e}"))?;
            Ok(json!({ "sources": client.source_names() }))
        }
        Cmd::Playlist { url, work_dir } => playlist(url, work_dir),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))
}

fn song_json(song: &SongInfo) -> Value {
    json!({
        "source": song.source,
        "name": song.song_name,
        "singers": song.singers,
        "album": song.album,
        "duration": song.duration,
        "ext": song.ext,
        "size": song.file_size,
    })
}

#[allow(clippy::too_many_arguments)]
fn search(
    keyword: &str,
    sources: Option<&str>,
    limit: usize,
    work_dir: &std::path::Path,
    download: bool,
    with_assets: bool,
    timeout_secs: u64,
) -> Result<Value> {
    let client = build_client(limit, work_dir, with_assets, timeout_secs)?;
    let source_names = source_names(sources);
    let runtime = runtime()?;
    let results = runtime.block_on(client.search(keyword, &source_names));

    let mut found: HashMap<String, Vec<SongInfo>> = HashMap::new();
    let mut source_states = Vec::new();
    let mut songs = Vec::new();
    for (name, result) in &results {
        match result {
            musicdl_rs::SearchResult::Ok(items) => {
                source_states.push(json!({ "source": name, "count": items.len() }));
                for song in items {
                    songs.push(song_json(song));
                }
                if !items.is_empty() {
                    found.insert(name.clone(), items.clone());
                }
            }
            musicdl_rs::SearchResult::Err(err) => {
                source_states.push(json!({ "source": name, "error": err }));
            }
        }
    }

    let files = if download && !found.is_empty() {
        download_songs(&runtime, &client, &found, work_dir)?
    } else {
        Vec::new()
    };

    Ok(json!({
        "keyword": keyword,
        "sources": source_states,
        "count": songs.len(),
        "songs": songs,
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
        .unwrap_or_else(|| vec!["netease", "kuwo", "kugou", "qianqian"])
}

fn build_client(
    limit: usize,
    work_dir: &std::path::Path,
    with_assets: bool,
    timeout_secs: u64,
) -> Result<MusicClient> {
    let content = if with_assets {
        DownloadContent::default()
    } else {
        DownloadContent {
            cover: false,
            lyric: false,
            ..Default::default()
        }
    };

    MusicClient::builder()
        .with_builtin_sources()
        .search_limits(limit)
        .work_dir(work_dir.to_path_buf())
        .download_content(content)
        .timeout(std::time::Duration::from_secs(timeout_secs.max(1)))
        .build()
        .map_err(|e| anyhow!("初始化失败: {e}"))
}

fn download_songs(
    runtime: &tokio::runtime::Runtime,
    client: &MusicClient,
    found: &HashMap<String, Vec<SongInfo>>,
    work_dir: &std::path::Path,
) -> Result<Vec<Value>> {
    std::fs::create_dir_all(work_dir)
        .map_err(|e| anyhow!("无法创建目录 {}: {e}", work_dir.display()))?;

    let mut files = Vec::new();
    for downloaded in runtime.block_on(client.download_all(found)) {
        files.push(write_song(&downloaded, work_dir)?);
    }
    Ok(files)
}

fn write_song(
    downloaded: &musicdl_rs::DownloadedSongInfo,
    work_dir: &std::path::Path,
) -> Result<Value> {
    let info = &downloaded.song_info;
    let name = info
        .save_name
        .clone()
        .unwrap_or_else(|| info.identifier.clone());
    let ext = info
        .ext
        .clone()
        .unwrap_or_else(|| downloaded.format.extension().to_string());
    let path = work_dir.join(format!("{name}.{ext}"));
    std::fs::write(&path, &downloaded.data)
        .map_err(|e| anyhow!("写入 {} 失败: {e}", path.display()))?;

    let mut item = json!({
        "source": info.source,
        "path": path.display().to_string(),
        "name": info.song_name,
        "bytes": downloaded.data.len(),
    });

    if let Some(cover) = &downloaded.cover_data {
        let cover_path = work_dir.join(format!("{name}.cover.jpg"));
        if std::fs::write(&cover_path, cover).is_ok() {
            item["cover"] = json!(cover_path.display().to_string());
        }
    }
    if let Some(lyric) = &downloaded.lyric_data {
        let lyric_path = work_dir.join(format!("{name}.lrc"));
        if std::fs::write(&lyric_path, lyric).is_ok() {
            item["lyric"] = json!(lyric_path.display().to_string());
        }
    }

    Ok(item)
}

fn playlist(url: &str, work_dir: &std::path::Path) -> Result<Value> {
    let client = MusicClient::builder()
        .with_builtin_sources()
        .work_dir(work_dir.to_path_buf())
        .build()
        .map_err(|e| anyhow!("初始化失败: {e}"))?;

    let songs = runtime()?
        .block_on(client.parse_playlist(url))
        .map_err(|e| anyhow!("解析歌单失败: {e}"))?;

    Ok(json!({
        "count": songs.len(),
        "songs": songs.iter().map(song_json).collect::<Vec<_>>(),
    }))
}
