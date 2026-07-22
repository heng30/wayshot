# musicdl-rs

基于 Rust 的音乐搜索与下载库，从 [musicdl](https://github.com/CharlesPikachu/musicdl) Python 项目提取核心搜索和下载逻辑重构而成。提供搜索和下载等基础功能，内置 **4 个中文音乐源**，支持自定义扩展。

## 特性

- 🔍 **多源搜索** — 同时从多个音乐源并发搜索
- ⬇️ **并发下载** — 自动尝试候选 URL，并发下载音频文件
- 🔌 **可扩展** — 实现 `MusicSource` trait 即可添加新的音乐源
- 🎵 **两阶段解析** — 搜索获取歌曲元数据，再解析下载 URL（支持多音质级别和第三方 API 回退）
- 🎶 **丰富元数据** — 歌手、专辑、时长、比特率、歌词、封面等完整信息
- 🔊 **格式检测** — 基于魔数字节自动检测音频格式（MP3/FLAC/AAC/M4A/OGG/Opus/WAV/WMA/APE）
- ⚡ **异步优先** — 基于 tokio + reqwest 的全异步架构
- 🔁 **自动重试** — 内置 HTTP 请求重试机制
- 🌐 **代理支持** — 支持 socks5 / http / https 代理

## 快速开始

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
musicdl-rs = "0.1"
tokio = { version = "1", features = ["full"] }
```

### 搜索歌曲

```rust
use musicdl_rs::{MusicClient, SearchResult};

#[tokio::main]
async fn main() -> musicdl_rs::Result<()> {
    // 创建客户端，加载内置音乐源
    let client = MusicClient::builder()
        .with_builtin_sources()
        .search_limits(5)
        .build()?;

    // 从多个源搜索歌曲
    let results = client.search("周杰伦", &["netease", "kuwo"]).await;

    // 查看搜索结果
    for (source, result) in &results {
        match result {
            SearchResult::Ok(songs) => {
                println!("从 {} 找到 {} 首歌", source, songs.len());
                for song in songs {
                    println!(
                        "  {} - {} [{}]",
                        song.singers.as_deref().unwrap_or("?"),
                        song.song_name.as_deref().unwrap_or("?"),
                        song.ext.as_deref().unwrap_or("?"),
                    );
                }
            }
            SearchResult::Err(err) => {
                println!("源 {} 搜索失败: {}", source, err);
            }
        }
    }

    Ok(())
}
```

### 下载歌曲

```rust
// 下载某个源的搜索结果
if let Some(SearchResult::Ok(songs)) = results.get("netease") {
    let downloaded = client.download("netease", songs).await?;
    println!("成功下载 {} 首歌", downloaded.len());

    // 保存到文件
    for item in &downloaded {
        let ext = item.format.extension();
        let path = format!(
            "output/{} - {}.{}",
            item.song_info.singers.as_deref().unwrap_or("unknown"),
            item.song_info.song_name.as_deref().unwrap_or("unknown"),
            ext
        );
        std::fs::write(&path, &item.data)?;
    }
}
```

### 使用代理

```rust
// 配置 socks5 代理
let client = MusicClient::builder()
    .with_builtin_sources()
    .proxy("socks5://127.0.0.1:1084")
    .build()?;

// 配置 http 代理
let client = MusicClient::builder()
    .with_builtin_sources()
    .proxy("http://proxy.example.com:8080")
    .build()?;
```

## 示例

项目包含一个基于 [clap](https://docs.rs/clap) 的命令行搜索示例。

### 运行示例

```bash
# 搜索关键词，默认使用全部 4 个源
cargo run --example search -- "周杰伦"

# 只搜索指定源（逗号分隔）
cargo run --example search -- "周杰伦" --sources netease,kuwo

# 查看帮助
cargo run --example search -- --help
```

### 命令行参数

| 参数 | 说明 |
|---|---|
| `<KEYWORD>` | 必填，搜索关键词 |
| `--sources <SOURCES>` | 可选，指定音乐源（逗号分隔），默认 `netease,kugou,kuwo,qianqian` |

## 自定义音乐源

实现 `MusicSource` trait 即可添加自己的音乐源。音乐源的核心是**两阶段流程**：

1. **搜索阶段** — `construct_search_urls()` → 获取页面 → `parse_search_result()` 返回基本歌曲元数据（`download_url` 可为空）
2. **解析阶段** — `parse_download_url()` 解析实际下载 URL（可尝试多个音质级别和第三方 API 回退）

```rust
use async_trait::async_trait;
use musicdl_rs::{
    MusicSource, SongInfo, SearchParams, SearchUrl, Filters,
    HttpClient, DownloadConfig, DownloadedSongInfo, Result,
};

struct MySource;

#[async_trait]
impl MusicSource for MySource {
    fn source_name(&self) -> &str {
        "my_source"
    }

    fn construct_search_urls(
        &self,
        keyword: &str,
        params: &SearchParams,
        _filters: &Filters,
    ) -> Vec<SearchUrl> {
        vec![SearchUrl::new(format!(
            "https://example.com/search?q={}",
            keyword
        ))]
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<SongInfo>> {
        // 解析 HTTP 响应，返回 SongInfo 列表
        // 此时 download_url 可以是 None，稍后在 parse_download_url 中解析
        Ok(vec![])
    }

    async fn parse_download_url(
        &self,
        song_info: &mut SongInfo,
        http: &HttpClient,
    ) -> Result<()> {
        // 解析下载 URL：尝试多音质、第三方 API 回退等
        // 将结果写入 song_info.download_url, song_info.ext, song_info.file_size_bytes 等
        Ok(())
    }
}

// 注册并使用
let client = MusicClient::builder()
    .register_source("my_source", || Box::new(MySource))
    .build()?;
```

## 内置音乐源（4 个）

| 源 | 名称 | 搜索方式 | 下载 URL 解析 | 备注 |
|---|---|---|---|---|
| 网易云音乐 | `"netease"` | POST form-encoded API | 官方 player/url API | 支持多音质级别（320/192/128kbps） |
| 酷狗音乐 | `"kugou"` | GET JSON API | 官方移动端 getSongInfo API | VIP 歌曲需付费 |
| 酷我音乐 | `"kuwo"` | GET JSON API | 官方 antiserver API | VIP 歌曲返回试听片段 |
| 千千音乐 | `"qianqian"` | GET JSON API（需签名） | 官方 tracklink API（需签名） | 原百度音乐，API 需要 MD5 签名认证 |

## SongInfo 字段

搜索结果中的每首歌以 `SongInfo` 结构体表示：

| 字段 | 类型 | 说明 |
|---|---|---|
| `source` | `String` | 来源标识（如 `"netease"`） |
| `song_name` | `Option<String>` | 歌曲名 |
| `singers` | `Option<String>` | 歌手名（逗号分隔） |
| `album` | `Option<String>` | 专辑名 |
| `ext` | `Option<String>` | 音频扩展名（如 `"mp3"`, `"flac"`） |
| `file_size_bytes` | `Option<u64>` | 文件大小（字节） |
| `file_size` | `Option<String>` | 人类可读大小（如 `"4.20 MB"`） |
| `duration_s` | `Option<u64>` | 时长（秒） |
| `duration` | `Option<String>` | 人类可读时长（如 `"3:45"`） |
| `bitrate` | `Option<u32>` | 比特率（kbps） |
| `lyric` | `Option<String>` | LRC 格式歌词 |
| `cover_url` | `Option<String>` | 封面图片 URL |
| `download_url` | `Option<String>` | 解析后的下载 URL |
| `identifier` | `String` | 源内唯一标识（用于去重） |
| `work_dir` | `PathBuf` | 保存目录 |
| `save_path` | `Option<PathBuf>` | 完整保存路径 |

## 架构

```
src/
  lib.rs              — 库入口，重新导出公共 API
  error.rs            — 错误类型定义（MusicDlError）
  types.rs            — 核心数据类型（SongInfo, SearchParams, AudioFormat 等）
  filter.rs           — 过滤器规则引擎
  detect.rs           — 音频格式检测
  utils.rs            — 工具函数（safe_extract_from_dict, legalize_string 等）
  client/
    mod.rs            — 模块导出
    http.rs           — HTTP 客户端封装（重试、代理、JSON、音频链接测试）
    source.rs         — MusicSource trait, SourceRegistry, MusicClient, MusicClientBuilder
  sources/
    mod.rs            — 内置音乐源汇总
    netease.rs        — 网易云音乐源
    kugou.rs          — 酷狗音乐源
    kuwo.rs           — 酷我音乐源
    qianqian.rs       — 千千音乐源
```

### 核心设计

- **两阶段模板方法** — `MusicSource` trait 的默认 `search()` 实现：搜索获取元数据 → 解析下载 URL → 去重 → 分配路径
- **两阶段解析** — 与 imagedl-rs 不同，音乐源需要先搜索获取歌曲列表，再逐首解析下载 URL（多音质 + 第三方 API 回退）
- **音频链接测试** — `HttpClient::test_audio_link()` 使用 Range 请求验证 URL 有效性，检测格式和大小
- **自动去重** — 基于 `identifier` 字段去重搜索结果
- **格式检测** — 基于 `infer` 库的魔数字节检测，Content-Type 和 URL 扩展名推测
- **代理支持** — 通过 `reqwest::Proxy::all()` 支持 socks5 / http / https 代理

## 许可证

Apache License 2.0
