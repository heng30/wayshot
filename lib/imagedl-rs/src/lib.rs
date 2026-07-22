//! # imagedl-rs
//!
//! A Rust library for searching and downloading images from multiple sources.
//!
//! This library provides an extensible framework for image search and download,
//! with built-in support for Bing, Baidu, Unsplash, and Google image sources.
//!
//! # Quick Start
//!
//! ```ignore
//! use imagedl::ImageClient;
//!
//! #[tokio::main]
//! async fn main() -> imagedl::Result<()> {
//!     // Create a client with built-in sources
//!     let client = ImageClient::builder()
//!         .with_builtin_sources()
//!         .search_limits(100)
//!         .build()?;
//!
//!     // Search for images
//!     let results = client.search("cats", &["bing", "unsplash"]).await;
//!
//!     // Print results (SearchResult distinguishes success from failure)
//!     for (source, result) in &results {
//!         match result {
//!             imagedl::SearchResult::Ok(images) => {
//!                 println!("Found {} images from {}", images.len(), source);
//!             }
//!             imagedl::SearchResult::Err(err) => {
//!                 println!("Source {} failed: {}", source, err);
//!             }
//!         }
//!     }
//!
//!     // Download images from a source
//!     if let Some(imagedl::SearchResult::Ok(images)) = results.get("bing") {
//!         let downloaded = client.download("bing", images).await?;
//!         println!("Downloaded {} images", downloaded.len());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Proxy Support
//!
//! ```ignore
//! // Configure a socks5 proxy
//! let client = ImageClient::builder()
//!     .with_builtin_sources()
//!     .proxy("socks5://127.0.0.1:1084")
//!     .build()?;
//!
//! // Configure an http proxy
//! let client = ImageClient::builder()
//!     .with_builtin_sources()
//!     .proxy("http://proxy.example.com:8080")
//!     .build()?;
//! ```
//!
//! # Custom Source
//!
//! You can implement your own image source by implementing the `ImageSource` trait:
//!
//! ```ignore
//! use async_trait::async_trait;
//! use imagedl::{ImageSource, ImageInfo, SearchParams, SearchUrl, Filters, Result};
//!
//! struct MySource;
//!
//! #[async_trait]
//! impl ImageSource for MySource {
//!     fn source_name(&self) -> &str { "my_source" }
//!
//!     fn construct_search_urls(&self, keyword: &str, params: &SearchParams, _: &Filters) -> Vec<SearchUrl> {
//!         vec![SearchUrl::new(format!("https://example.com/search?q={}", keyword))]
//!     }
//!
//!     fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
//!         // Parse the response and return ImageInfo items
//!         Ok(vec![])
//!     }
//! }
//! ```

pub mod client;
pub mod detect;
pub mod error;
pub mod filter;
pub mod sources;
pub mod types;

pub use client::{
    ImageClient, ImageClientBuilder, ImageSource, SearchResult, SourceRegistry,
    http::{HttpClient, HttpClientBuilder},
};
pub use detect::ImageFormatDetector;
pub use error::{ImageDlError, Result};
pub use filter::Filter;
pub use sources::{
    AicImageSource, BaiduImageSource, BingImageSource, BlueskyImageSource, ClevelandArtImageSource,
    DimtownImageSource, DuckduckgoImageSource, EverypixelImageSource, FlickrImageSource,
    FoodiesfeedImageSource, FreeimagesImageSource, FreenaturestockImageSource, GbifImageSource,
    GelbooruImageSource, GoogleImageSource, GratisographyImageSource, HuabanImageSource,
    I360ImageSource, INaturalistImageSource, InternetArchiveImageSource, KonachanImageSource,
    LifeOfPixImageSource, LocGovImageSource, MetropolitanImageSource, NasaImageSource,
    OpenLibraryImageSource, OpenverseImageSource, PexelsImageSource, PicjumboImageSource,
    PixabayImageSource, SafebooruImageSource, SmkImageSource, SogouImageSource,
    StocksnapImageSource, VamImageSource, WallhavenImageSource, WeiboImageSource,
    WellcomeImageSource, WikipediaImageSource, YandeImageSource, YandexImageSource,
};
pub use types::{
    DownloadConfig, DownloadedInfo, FilterValue, Filters, HttpMethod, ImageFormat, ImageInfo,
    SearchParams, SearchUrl,
};
