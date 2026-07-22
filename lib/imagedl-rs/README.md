# imagedl-rs

基于 Rust 的图片搜索与下载库，从 [imagedl](https://github.com/CharlesPikachu/imagedl) Python 项目提取核心逻辑重构而成。

提供搜索和下载等基础功能，内置 **44 个图片源**，支持自定义扩展。

## 特性

- 🔍 **多源搜索** — 同时从多个图片源并发搜索
- ⬇️ **并发下载** — 自动尝试候选 URL，并发下载图片
- 🔌 **可扩展** — 实现 `ImageSource` trait 即可添加新的图片源
- 🧩 **过滤器系统** — 类型、颜色、尺寸、许可证等搜索过滤
- 🖼️ **格式检测** — 基于魔数字节自动检测图片格式（JPEG/PNG/GIF/WebP/BMP/TIFF/AVIF/HEIF）
- ⚡ **异步优先** — 基于 tokio + reqwest 的全异步架构
- 🔁 **自动重试** — 内置 HTTP 请求重试机制
- 🌐 **代理支持** — 支持 socks5 / http / https 代理

## 快速开始

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
imagedl-rs = "0.1"
tokio = { version = "1", features = ["full"] }
```

### 搜索图片

```rust
use imagedl::ImageClient;

#[tokio::main]
async fn main() -> imagedl::Result<()> {
    // 创建客户端，加载内置图片源
    let client = ImageClient::builder()
        .with_builtin_sources()
        .search_limits(100)
        .build()?;

    // 从多个源搜索图片
    let results = client.search("猫咪", &["bing", "unsplash", "wallhaven"]).await;

    // 查看搜索结果
    for (source, result) in &results {
        match result {
            imagedl::SearchResult::Ok(images) => {
                println!("从 {} 找到 {} 张图片", source, images.len());
            }
            imagedl::SearchResult::Err(err) => {
                println!("源 {} 搜索失败: {}", source, err);
            }
        }
    }

    Ok(())
}
```

### 下载图片

```rust
// 下载某个源的搜索结果
if let Some(imagedl::SearchResult::Ok(images)) = results.get("bing") {
    let downloaded = client.download("bing", images).await?;
    println!("成功下载 {} 张图片", downloaded.len());

    // 保存到文件
    for item in &downloaded {
        let ext = item.format.extension();
        let path = format!("output/{}.{}", item.image_info.save_name.as_deref().unwrap_or("unknown"), ext);
        std::fs::write(&path, &item.data)?;
    }
}
```

### 使用代理

```rust
// 配置 socks5 代理
let client = ImageClient::builder()
    .with_builtin_sources()
    .proxy("socks5://127.0.0.1:1084")
    .build()?;

// 配置 http 代理
let client = ImageClient::builder()
    .with_builtin_sources()
    .proxy("http://proxy.example.com:8080")
    .build()?;
```

### 使用过滤器

```rust
use imagedl::{ImageClient, FilterValue, Filters};
use std::collections::HashMap;

let mut filters = HashMap::new();

// Bing 过滤器
let mut bing_filters = Filters::new();
bing_filters.insert("type".to_string(), FilterValue::from("photo"));
bing_filters.insert("color".to_string(), FilterValue::from("color"));
filters.insert("bing".to_string(), bing_filters);

let results = client.search_with_filters("猫咪", &["bing"], &filters).await;
```

## 示例

项目包含一个基于 [clap](https://docs.rs/clap) 的命令行示例，可对每个支持的图片源进行搜索，展示匹配结果的文件地址，并可选下载到指定目录。

### 运行示例

```bash
# 搜索关键词，展示每个源前 3 个结果（默认）
cargo run --example search -- 猫咪

# 指定搜索数量
cargo run --example search -- 猫咪 -n 5

# 只搜索照片类型
cargo run --example search -- 猫咪 -t photo

# 搜索彩色大图
cargo run --example search -- 猫咪 -c color --size large

# 只搜索指定源（逗号分隔）
cargo run --example search -- 猫咪 -s bing,unsplash,wallhaven

# 搜索并下载到指定目录
cargo run --example search -- 猫咪 -d ./downloads

# 使用 socks5 代理搜索 Google
cargo run --example search -- 猫咪 -s google -p socks5://127.0.0.1:1084

# 使用 http 代理搜索
cargo run --example search -- 猫咪 -p http://proxy.example.com:8080

# 组合使用：搜索 5 张黑白线描图并下载
cargo run --example search -- 猫咪 -n 5 -t linedrawing -c bw -d ./downloads

# 查看帮助
cargo run --example search -- --help
```

### 命令行参数

| 参数 | 说明 |
|---|---|
| `<KEYWORD>` | 必填，搜索关键词 |
| `-n, --number <NUMBER>` | 可选，每个源展示/下载的最大数量，默认 3 |
| `-s, --sources <SOURCES>...` | 可选，指定图片源，默认搜索全部 |
| `-t, --type <TYPE>` | 可选，图片类型（photo / clipart / linedrawing / face / animated） |
| `-c, --color <COLOR>` | 可选，颜色模式（color=彩色 / bw=黑白） |
| `--size <SIZE>` | 可选，尺寸（large / medium / small） |
| `-p, --proxy <PROXY>` | 可选，HTTP 代理地址（如 `socks5://127.0.0.1:1084` 或 `http://proxy:8080`） |
| `-d, --download <DIR>` | 可选，提供目录路径则下载指定数量的结果到该目录，不提供则仅展示 |

## 自定义图片源

实现 `ImageSource` trait 即可添加自己的图片源：

```rust
use async_trait::async_trait;
use imagedl::{
    ImageSource, ImageInfo, SearchParams, SearchUrl, Filters, Result,
};

struct MySource;

#[async_trait]
impl ImageSource for MySource {
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

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        // 解析 HTTP 响应，返回 ImageInfo 列表
        Ok(vec![])
    }
}

// 注册并使用
let client = ImageClient::builder()
    .register_source("my_source", || Box::new(MySource))
    .build()?;
```

## 内置图片源（44 个）

### 搜索引擎

| 源 | 名称 | 解析方式 | 支持的过滤器 | 备注 |
|---|---|---|---|---|
| Bing | `"bing"` | HTML + CSS 选择器 | type, color, size, license, layout, people, date | |
| 百度 | `"baidu"` | JSON API | type, color, size | |
| Google | `"google"` | Custom Search JSON API | type, color, size, license, date | 使用 CSE API；免费额度 100 次/天/密钥，内置 4 组密钥轮换；需能访问 googleapis.com |
| DuckDuckGo | `"duckduckgo"` | JSON API | time, size, color, type, layout, license | 需先获取 vqd 令牌 |
| 360 搜索 | `"i360"` | JSON API | size, color, type | |
| 搜狗 | `"sogou"` | JSON API | — | |
| Yahoo | `"yahoo"` | HTML + JSON data 属性 | size, color, type, layout, people, time, license | 尝试多个区域域名 |
| Yandex | `"yandex"` | HTML + data-state JSON | isize, iorient, itype, icolor, file_type | 尝试多个域名 |
| 微博 | `"weibo"` | JSON API | — | 使用移动端 API；自动将缩略图 URL 升级为高清 |

### 图库 / 素材

| 源 | 名称 | 解析方式 | 支持的过滤器 | 备注 |
|---|---|---|---|---|
| Unsplash | `"unsplash"` | REST JSON API | per_page 及其他 API 参数 | |
| Pexels | `"pexels"` | JSON API | — | |
| Pixabay | `"pixabay"` | JSON API | lang, image_type, orientation, category, colors, etc. | 内置 12 组 API 密钥轮换 |
| Flickr | `"flickr"` | REST JSON API | — | 内置 8 组 API 密钥轮换 |
| Wallhaven | `"wallhaven"` | REST JSON API | categories, purity, sorting, order, topRange, atleast, ratios, colors | 壁纸网站 |
| StockSnap | `"stocksnap"` | JSON API | — | |
| Everypixel | `"everypixel"` | JSON API | type, image_type, orientation, people, age, gender, ethnicity | |
| Openverse | `"openverse"` | REST JSON API | — | CC 许可媒体搜索 |
| FreeImages | `"freeimages"` | JSON API | — | |
| Gratisography | `"gratisography"` | HTML + srcset | — | |
| PicJumbo | `"picjumbo"` | HTML + srcset | — | |
| FreeNatureStock | `"freenaturestock"` | HTML + img 标签 | — | |
| Foodiesfeed | `"foodiesfeed"` | JSON API | — | 美食图片 |

### 图像板 (Moebooru)

| 源 | 名称 | 解析方式 | 备注 |
|---|---|---|---|
| Danbooru | `"danbooru"` | JSON API | 动漫图片 |
| Konachan | `"konachan"` | JSON API | 动漫壁纸（自动添加 rating:safe） |
| Safebooru | `"safebooru"` | JSON API | 安全图片 |
| Yande.re | `"yande"` | JSON API | 高质量图片 |
| Gelbooru | `"gelbooru"` | HTML（2 步解析） | 需访问帖子页面获取原图 |

### 博物馆 / 艺术

| 源 | 名称 | 解析方式 | 备注 |
|---|---|---|---|
| AIC (芝加哥艺术学院) | `"aic"` | JSON API + IIIF | |
| Cleveland Art | `"clevelandart"` | JSON API | |
| Metropolitan (大都会博物馆) | `"metropolitan"` | JSON API（2 步） | 先搜索 ID，再逐个获取详情 |
| SMK (丹麦国家美术馆) | `"smk"` | JSON API + IIIF | |
| VAM (维多利亚与阿尔伯特博物馆) | `"vam"` | JSON API + IIIF | |
| Wellcome Collection | `"wellcome"` | JSON API | IIIF 图片 URL 转换 |

### 自然 / 科学

| 源 | 名称 | 解析方式 | 备注 |
|---|---|---|---|
| iNaturalist | `"inaturalist"` | REST JSON API | 自然观察照片 |
| NASA | `"nasa"` | REST JSON API | NASA 图片库；自动将缩略图升级为高清 |
| GBIF | `"gbif"` | REST JSON API | 全球生物多样性信息设施 |

### 其他

| 源 | 名称 | 解析方式 | 备注 |
|---|---|---|---|
| Bluesky | `"bluesky"` | JSON API | 提取帖子中的嵌入图片 |
| 花瓣网 | `"huaban"` | JSON API | |
| Internet Archive | `"internetarchive"` | JSON API | |
| LifeOfPix | `"lifeofpix"` | JSON API | |
| LOC (美国国会图书馆) | `"locgov"` | JSON API | |
| Open Library | `"openlibrary"` | JSON API | 书籍封面 |
| Wikipedia Commons | `"wikipedia"` | JSON API | MediaWiki API |
| DimTown | `"dimtown"` | HTML（2 步解析） | |

### 过滤器选项说明

#### type（图片类型）

| 值 | 说明 |
|---|---|
| `photo` | 照片 |
| `clipart` | 剪贴画 |
| `linedrawing` | 线描 |
| `face` | 人脸 |
| `animated` | 动图 |

#### color（颜色）

| 值 | 说明 |
|---|---|
| `color` | 彩色 |
| `bw` | 黑白 |

#### size（尺寸）

| 值 | 说明 |
|---|---|
| `large` | 大图 |
| `medium` | 中图 |
| `small` | 小图 |

## 架构

```
src/
  lib.rs              — 库入口，重新导出公共 API
  error.rs            — 错误类型定义
  types.rs            — 核心数据类型（ImageInfo, SearchParams, ImageFormat 等）
  filter.rs           — 过滤器规则引擎
  detect.rs           — 图片格式检测
  client/
    mod.rs            — ImageSource trait, SourceRegistry, ImageClient
    http.rs           — HTTP 客户端封装（重试、代理、请求头管理）
  sources/
    mod.rs            — 内置图片源汇总
    bing.rs           — Bing 图片源
    baidu.rs          — 百度图片源
    unsplash.rs       — Unsplash 图片源
    google.rs         — Google 图片源（Custom Search JSON API）
    ... (42 个图片源实现)
```

### 核心设计

- **模板方法模式** — `ImageSource` trait 提供默认的 `search()`/`download()` 流程，实现者只需提供 `construct_search_urls()` 和 `parse_search_result()`
- **两阶段搜索** — 阶段一并发获取页面（仅需要 `HttpClient`，天然 `Send+Sync`），阶段二顺序解析（需要 `&self`）
- **候选项降级** — 每个搜索结果包含多个候选下载 URL，按顺序尝试直到成功
- **自动去重** — 基于 `identifier` 字段去重搜索结果
- **自动检测** — 基于 `infer` 库的魔数字节检测，替代 Python 的 imghdr → filetype → Pillow 三级级联
- **代理支持** — 通过 `reqwest::Proxy::all()` 支持 socks5 / http / https 代理

## 与 Python 版本的差异

| 特性 | Python imagedl | imagedl-rs |
|---|---|---|
| 运行时 | 同步 + 线程池 | 全异步（tokio） |
| HTTP 客户端 | requests / curl_cffi / DrissionPage | reqwest |
| HTML 解析 | BeautifulSoup + lxml | scraper（基于 html5ever） |
| 图片格式检测 | imghdr + filetype + Pillow | infer（单次魔数检测） |
| 代理 | freeproxy 集成 | 单代理配置（socks5/http/https） |
| Google 搜索 | DrissionPage 无头浏览器 | Custom Search JSON API |
| 内置源数量 | 45+ | 44（框架支持任意扩展） |
| 反爬绕过 | cloudscraper / DrissionPage | 无（可集成 headless 浏览器） |

## 许可证

Apache License 2.0
