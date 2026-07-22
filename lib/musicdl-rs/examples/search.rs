//! Example: Search and download songs using musicdl-rs.
//!
//! Usage:
//!   cargo run --example search -- "周杰伦"
//!   cargo run --example search -- "周杰伦" -d ./downloads
//!   cargo run --example search -- "周杰伦" -d ./downloads --content song,cover,lyric
//!   cargo run --example search -- "周杰伦" -d ./downloads --content song,lyric
//!   cargo run --example search -- "周杰伦" --sources netease,kuwo -d ./downloads

use clap::Parser;
use musicdl_rs::{DownloadContent, MusicClient, Result, SearchResult};

#[derive(Parser, Debug)]
#[command(
    name = "musicdl-search",
    about = "Search and download songs using musicdl-rs"
)]
struct Args {
    /// Search keyword
    keyword: String,

    /// Comma-separated list of sources (default: all built-in)
    #[arg(long, default_value = "netease,kugou,kuwo,qianqian")]
    sources: String,

    /// Download songs to the specified directory (if not provided, only show search results)
    #[arg(short, long)]
    download: Option<String>,

    /// Comma-separated list of content to download: song, cover, lyric (default: all)
    #[arg(long, value_delimiter = ',', default_value = "song,cover,lyric")]
    content: Vec<String>,
}

fn parse_content_flags(content: &[String]) -> DownloadContent {
    let mut flags = DownloadContent {
        audio: false,
        cover: false,
        lyric: false,
    };
    for item in content {
        match item.trim().to_lowercase().as_str() {
            "song" | "audio" | "music" => flags.audio = true,
            "cover" | "image" | "pic" | "photo" => flags.cover = true,
            "lyric" | "lyrics" | "lrc" => flags.lyric = true,
            _ => {}
        }
    }
    flags
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let content_flags = parse_content_flags(&args.content);

    // Build client
    let client = MusicClient::builder()
        .with_builtin_sources()
        .search_limits(5)
        .download_content(content_flags)
        .build()?;

    let sources: Vec<&str> = args.sources.split(',').map(|s| s.trim()).collect();

    println!();
    println!("🔍 搜索关键词: {}", args.keyword);
    println!("📡 音乐来源: {}", args.sources);
    println!("{}", "─".repeat(60));

    // Search
    let results = client.search(&args.keyword, &sources).await;

    // Print results and collect downloadable songs
    let mut total_songs = 0;
    let mut downloadable: Vec<(&str, &musicdl_rs::SongInfo)> = Vec::new();

    for (source, result) in &results {
        match result {
            SearchResult::Ok(songs) => {
                if songs.is_empty() {
                    println!("\n  📭 源 \"{}\" 未找到歌曲", source);
                    continue;
                }

                println!("\n  📦 {} — 共找到 {} 首歌曲", source, songs.len());
                for (i, song) in songs.iter().enumerate() {
                    println!(
                        "    {}. {} - {} [{}] {}",
                        i + 1,
                        song.singers.as_deref().unwrap_or("?"),
                        song.song_name.as_deref().unwrap_or("?"),
                        song.ext.as_deref().unwrap_or("?"),
                        song.duration.as_deref().unwrap_or(""),
                    );
                    if let Some(album) = &song.album {
                        println!("       专辑: {}", album);
                    }
                    if let Some(file_size) = &song.file_size {
                        println!("       大小: {}", file_size);
                    }
                    // Show cover and lyric availability
                    let mut extras = Vec::new();
                    if song.cover_url.is_some() {
                        extras.push("🖼️ 封面");
                    }
                    if song.lyric.is_some() {
                        extras.push("📝 歌词");
                    }
                    if !extras.is_empty() {
                        println!("       {}", extras.join("  "));
                    }
                    downloadable.push((source, song));
                }
                total_songs += songs.len();
            }
            SearchResult::Err(err) => {
                println!("\n  ❌ 源 \"{}\" 搜索失败: {}", source, err);
            }
        }
    }

    println!("{}", "─".repeat(60));
    println!("📊 共找到 {} 首歌曲", total_songs);

    // Download
    if let Some(download_dir) = &args.download {
        if downloadable.is_empty() {
            println!("\n⚠️  没有可下载的歌曲");
            return Ok(());
        }

        // Show download content summary
        let mut content_labels = Vec::new();
        if content_flags.audio {
            content_labels.push("🎵 歌曲");
        }
        if content_flags.cover {
            content_labels.push("🖼️ 封面");
        }
        if content_flags.lyric {
            content_labels.push("📝 歌词");
        }
        println!("\n⬇️  开始下载到: {} ({})", download_dir, content_labels.join(" + "));

        // Create download directory
        std::fs::create_dir_all(download_dir)
            .unwrap_or_else(|e| panic!("无法创建目录 {}: {}", download_dir, e));

        // Group by source and download
        let mut source_groups: std::collections::HashMap<&str, Vec<&musicdl_rs::SongInfo>> =
            std::collections::HashMap::new();
        for (src, info) in &downloadable {
            source_groups.entry(src).or_default().push(*info);
        }

        let mut total_downloaded = 0;

        for (source_name, songs) in &source_groups {
            let songs_to_download: Vec<musicdl_rs::SongInfo> =
                songs.iter().cloned().cloned().collect();

            if songs_to_download.is_empty() {
                continue;
            }

            println!(
                "  ⏳ 从 {} 下载 {} 首歌曲... ",
                source_name,
                songs_to_download.len()
            );

            match client.download(source_name, &songs_to_download).await {
                Ok(downloaded) => {
                    let count = downloaded.len();
                    // Save files
                    for item in &downloaded {
                        let singers = item.song_info.singers.as_deref().unwrap_or("unknown");
                        let song_name = item.song_info.song_name.as_deref().unwrap_or("unknown");
                        let base_name = format!("{} - {}", singers, song_name);

                        // Save audio
                        if content_flags.audio && !item.data.is_empty() {
                            let ext = item.format.extension();
                            let audio_path = std::path::Path::new(download_dir)
                                .join(format!("{}.{}", base_name, ext));
                            if let Err(e) = std::fs::write(&audio_path, &item.data) {
                                println!("    ❌ 写入歌曲失败 {:?}: {}", audio_path, e);
                                continue;
                            }
                            println!("    ✅ 已保存: {:?}", audio_path);
                        }

                        // Save lyrics
                        if content_flags.lyric {
                            if let Some(lyric) = &item.lyric_data {
                                let lyric_path = std::path::Path::new(download_dir)
                                    .join(format!("{}.lrc", base_name));
                                if let Err(e) = std::fs::write(&lyric_path, lyric) {
                                    println!("    ⚠️  写入歌词失败 {:?}: {}", lyric_path, e);
                                }
                            }
                        }

                        // Save cover
                        if content_flags.cover {
                            if let Some(cover_bytes) = &item.cover_data {
                                let cover_path = std::path::Path::new(download_dir)
                                    .join(format!("{} - cover.jpg", base_name));
                                if let Err(e) = std::fs::write(&cover_path, cover_bytes) {
                                    println!("    ⚠️  写入封面失败 {:?}: {}", cover_path, e);
                                }
                            }
                        }
                    }
                    total_downloaded += count;
                    println!(
                        "  ✅ {} 完成 (成功 {}/{})",
                        source_name,
                        count,
                        songs_to_download.len()
                    );
                }
                Err(e) => {
                    println!("❌ 失败: {}", e);
                }
            }
        }

        println!(
            "\n🏁 下载完成: 共 {} 首歌曲保存到 {}",
            total_downloaded, download_dir
        );
    } else {
        println!("\n💡 提示: 使用 -d <目录> 参数可下载搜索结果，例如:");
        println!(
            "   cargo run --example search -- {} -d ./downloads",
            args.keyword
        );
        println!("   可选 --content song,cover,lyric 控制下载内容");
    }

    Ok(())
}
