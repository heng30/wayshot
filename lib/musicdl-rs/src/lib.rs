//! # musicdl-rs
//!
//! A Rust library for searching and downloading music from multiple sources.
//!
//! This library provides an extensible framework for music search and download,
//! with built-in support for Netease, Migu, Kugou, Kuwo, QQ, and Qianqian
//! music sources.
//!
//! # Quick Start
//!
//! ```ignore
//! use musicdl::MusicClient;
//!
//! #[tokio::main]
//! async fn main() -> musicdl::Result<()> {
//!     // Create a client with built-in sources
//!     let client = MusicClient::builder()
//!         .with_builtin_sources()
//!         .search_limits(5)
//!         .build()?;
//!
//!     // Search for songs
//!     let results = client.search("周杰伦", &["netease", "migu"]).await;
//!
//!     // Print results (SearchResult distinguishes success from failure)
//!     for (source, result) in &results {
//!         match result {
//!             musicdl::SearchResult::Ok(songs) => {
//!                 println!("Found {} songs from {}", songs.len(), source);
//!                 for song in songs {
//!                     println!("  {} - {} ({})",
//!                         song.singers.as_deref().unwrap_or("?"),
//!                         song.song_name.as_deref().unwrap_or("?"),
//!                         song.ext.as_deref().unwrap_or("?"),
//!                     );
//!                 }
//!             }
//!             musicdl::SearchResult::Err(err) => {
//!                 println!("Source {} failed: {}", source, err);
//!             }
//!         }
//!     }
//!
//!     // Download songs from a source
//!     if let Some(musicdl::SearchResult::Ok(songs)) = results.get("netease") {
//!         let downloaded = client.download("netease", songs).await?;
//!         println!("Downloaded {} songs", downloaded.len());
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Custom Source
//!
//! You can implement your own music source by implementing the `MusicSource` trait:
//!
//! ```ignore
//! use async_trait::async_trait;
//! use musicdl::{MusicSource, SongInfo, SearchParams, SearchUrl, Filters, Result};
//!
//! struct MySource;
//!
//! #[async_trait]
//! impl MusicSource for MySource {
//!     fn source_name(&self) -> &str { "my_source" }
//!
//!     fn construct_search_urls(&self, keyword: &str, params: &SearchParams, _: &Filters) -> Vec<SearchUrl> {
//!         vec![SearchUrl::new(format!("https://example.com/search?q={}", keyword))]
//!     }
//!
//!     fn parse_search_result(&self, body: &str) -> Result<Vec<SongInfo>> {
//!         // Parse the response and return SongInfo items
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
pub mod utils;

pub use client::{
    MusicClient, MusicClientBuilder, MusicSource, SearchResult, SourceRegistry, http::HttpClient,
};
pub use detect::AudioFormatDetector;
pub use error::{MusicDlError, Result};
pub use filter::Filter;
pub use sources::{KugouMusicSource, KuwoMusicSource, NeteaseMusicSource, QianqianMusicSource};
pub use types::{
    AudioFormat, DownloadConfig, DownloadContent, DownloadProtocol, DownloadUrlStatus,
    DownloadedSongInfo, FilterValue, Filters, HttpMethod, SearchParams, SearchUrl, SongInfo,
};
