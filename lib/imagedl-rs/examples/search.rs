//! imagedl-rs 示例：搜索并展示/下载图片
//!
//! 用法：
//!   # 搜索关键词，展示每个源前 3 个结果
//!   cargo run --example search -- 猫咪
//!
//!   # 指定搜索数量
//!   cargo run --example search -- 猫咪 -n 5
//!
//!   # 只搜索照片类型
//!   cargo run --example search -- 猫咪 --type photo
//!
//!   # 搜索彩色大图
//!   cargo run --example search -- 猫咪 -c color --size large
//!
//!   # 搜索并下载到指定目录
//!   cargo run --example search -- 猫咪 -d ./downloads
//!
//!   # 组合使用：搜索 5 张黑白线描图并下载
//!   cargo run --example search -- 猫咪 -n 5 -t linedrawing -c bw -d ./downloads

use clap::Parser;
use imagedl_rs::{FilterValue, Filters, ImageClient, SearchResult};
use std::collections::HashMap;

/// imagedl-rs 图片搜索与下载示例
#[derive(Parser, Debug)]
#[command(name = "imagedl-search", version, about = "搜索并展示/下载图片")]
struct Args {
    /// 搜索关键词
    keyword: String,

    /// 每个源展示/下载的最大数量（默认 3）
    #[arg(short, long, default_value = "3")]
    number: usize,

    /// 指定搜索的图片源（如 bing, baidu, unsplash, google 等），默认搜索全部
    #[arg(short, long, num_args = 1.., value_delimiter = ',')]
    sources: Vec<String>,

    /// 图片类型过滤（photo, clipart, linedrawing, face, animated）
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    filter_type: Option<String>,

    /// 颜色过滤（color=彩色, bw=黑白）
    #[arg(short, long)]
    color: Option<String>,

    /// 尺寸过滤（large, medium, small）
    #[arg(long)]
    size: Option<String>,

    /// 下载图片到指定目录（如不提供则仅展示搜索结果）
    #[arg(short, long)]
    download: Option<String>,

    /// HTTP 代理地址（如 socks5://127.0.0.1:1084 或 http://proxy:8080）
    #[arg(short = 'p', long)]
    proxy: Option<String>,
}

/// 所有内置图片源
const ALL_SOURCES: &[&str] = &[
    "bing", "baidu", "google", "duckduckgo", "i360", "sogou", "yandex", "weibo",
    "pexels", "pixabay", "flickr", "wallhaven", "stocksnap", "everypixel",
    "openverse", "freeimages", "gratisography", "picjumbo", "freenaturestock", "foodiesfeed",
    "konachan", "safebooru", "yande", "gelbooru",
    "aic", "clevelandart", "metropolitan", "smk", "vam", "wellcome",
    "inaturalist", "nasa", "gbif",
    "bluesky", "huaban", "internetarchive", "lifeofpix", "locgov", "openlibrary", "wikipedia",
    "dimtown",
];

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let limit = args.number;

    // 确定搜索源
    let sources: Vec<&str> = if args.sources.is_empty() {
        ALL_SOURCES.to_vec()
    } else {
        args.sources.iter().map(|s| s.as_str()).collect()
    };

    // 构建过滤器：将 type/color/size 参数应用到所有源
    let mut all_filters: HashMap<String, Filters> = HashMap::new();
    let has_filters = args.filter_type.is_some() || args.color.is_some() || args.size.is_some();

    if has_filters {
        for source in &sources {
            let mut filters = Filters::new();
            if let Some(ref v) = args.filter_type {
                filters.insert("type".to_string(), FilterValue::from(v.as_str()));
            }
            if let Some(ref v) = args.color {
                let mapped = match v.as_str() {
                    "bw" => "blackandwhite",
                    other => other,
                };
                filters.insert("color".to_string(), FilterValue::from(mapped));
            }
            if let Some(ref v) = args.size {
                filters.insert("size".to_string(), FilterValue::from(v.as_str()));
            }
            all_filters.insert(source.to_string(), filters);
        }
    }

    // 构建客户端：搜索数量略多于所需，以应对去重和失败
    let mut builder = ImageClient::builder()
        .with_builtin_sources()
        .search_limits((limit * 4).max(30))
        .max_retries(2)
        .timeout(std::time::Duration::from_secs(15));

    if let Some(ref proxy) = args.proxy {
        builder = builder.proxy(proxy);
    }

    let client = builder.build().expect("无法创建 ImageClient");

    println!();
    println!("🔍 搜索关键词: {}", args.keyword);
    println!("📡 图片来源: {}", sources.join(", "));
    println!("🔢 每源数量: {}", limit);
    if has_filters {
        let mut filter_parts = Vec::new();
        if let Some(ref v) = args.filter_type {
            filter_parts.push(format!("类型={}", v));
        }
        if let Some(ref v) = args.color {
            filter_parts.push(format!(
                "颜色={}",
                match v.as_str() {
                    "bw" => "黑白",
                    "color" => "彩色",
                    other => other,
                }
            ));
        }
        if let Some(ref v) = args.size {
            filter_parts.push(format!("尺寸={}", v));
        }
        println!("🎛️  过滤条件: {}", filter_parts.join(", "));
    }
    println!("{}", "─".repeat(60));

    // 搜索
    let results = if has_filters {
        client
            .search_with_filters(&args.keyword, &sources, &all_filters)
            .await
    } else {
        client.search(&args.keyword, &sources).await
    };

    // 展示搜索结果
    let mut total_displayed = 0;
    let mut downloadable: Vec<(&str, &imagedl_rs::ImageInfo)> = Vec::new();

    for source_name in &sources {
        match results.get(*source_name) {
            Some(SearchResult::Ok(images)) => {
                if images.is_empty() {
                    println!("\n  📭 源 \"{}\" 未找到图片", source_name);
                    continue;
                }

                println!("\n  📦 {} — 共找到 {} 张图片", source_name, images.len());

                let display_images = images.iter().take(limit);
                for (i, info) in display_images.enumerate() {
                    total_displayed += 1;
                    println!(
                        "    {}. {}",
                        i + 1,
                        info.candidate_download_urls
                            .first()
                            .unwrap_or(&"<无URL>".to_string())
                    );
                    if !info.description.is_empty() {
                        let desc: String = info.description.chars().take(60).collect();
                        println!(
                            "       描述: {}{}",
                            desc,
                            if info.description.len() > 60 { "..." } else { "" }
                        );
                    }
                    downloadable.push((source_name, info));
                }

                if images.len() > limit {
                    println!("    ... 还有 {} 张", images.len() - limit);
                }
            }
            Some(SearchResult::Err(err)) => {
                println!("\n  ❌ 源 \"{}\" 搜索失败: {}", source_name, err);
            }
            None => {
                println!("\n  ⚠️  源 \"{}\" 无结果", source_name);
            }
        }
    }

    println!("{}", "─".repeat(60));
    println!("📊 共展示 {} 个结果", total_displayed);

    // 下载
    if let Some(download_dir) = &args.download {
        if downloadable.is_empty() {
            println!("\n⚠️  没有可下载的图片");
            return;
        }

        println!("\n⬇️  开始下载到: {}", download_dir);

        // 创建下载目录
        std::fs::create_dir_all(download_dir)
            .unwrap_or_else(|e| panic!("无法创建目录 {}: {}", download_dir, e));

        // 按源分组下载
        let mut source_groups: std::collections::HashMap<&str, Vec<&imagedl_rs::ImageInfo>> =
            std::collections::HashMap::new();
        for (src, info) in &downloadable {
            source_groups.entry(src).or_default().push(*info);
        }

        let mut total_downloaded = 0;

        for (source_name, images) in &source_groups {
            let images_to_download: Vec<imagedl_rs::ImageInfo> =
                images.iter().take(limit).cloned().cloned().collect();

            if images_to_download.is_empty() {
                continue;
            }

            print!(
                "  ⏳ 从 {} 下载 {} 张图片... ",
                source_name,
                images_to_download.len()
            );

            match client.download(source_name, &images_to_download).await {
                Ok(downloaded) => {
                    let count = downloaded.len();
                    // 保存文件
                    for item in &downloaded {
                        let ext = item.format.extension();
                        let save_name = item.image_info.save_name.as_deref().unwrap_or("unknown");
                        let file_path = std::path::Path::new(download_dir)
                            .join(format!("{}_{}.{}", source_name, save_name, ext));
                        if let Err(e) = std::fs::write(&file_path, &item.data) {
                            println!();
                            println!("    ❌ 写入文件失败 {:?}: {}", file_path, e);
                        } else {
                            println!("    ✅ 已保存: {:?}", file_path);
                        }
                    }
                    total_downloaded += count;
                    println!(
                        "  ✅ {} 完成 (成功 {}/{})",
                        source_name,
                        count,
                        images_to_download.len()
                    );
                }
                Err(e) => {
                    println!("❌ 失败: {}", e);
                }
            }
        }

        println!(
            "\n🏁 下载完成: 共 {} 张图片保存到 {}",
            total_downloaded, download_dir
        );
    } else {
        println!("\n💡 提示: 使用 -d <目录> 参数可下载搜索结果，例如:");
        println!(
            "   cargo run --example search -- {} -d ./downloads",
            args.keyword
        );
    }
}
