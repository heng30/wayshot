//! Core trait, registry, and orchestrator for music sources.
//!
//! This module contains:
//! - `MusicSource` trait — the central abstraction that every source must implement
//! - `SourceRegistry` — runtime registry of available sources
//! - `MusicClient` — top-level orchestrator for multi-source search and download
//! - `MusicClientBuilder` — builder for configuring `MusicClient`

use super::http::HttpClient;
pub use crate::types::{
    DownloadConfig, DownloadContent, DownloadProtocol, DownloadedSongInfo, Filters, SearchParams,
    SearchUrl, SongInfo,
};
use crate::{
    detect::AudioFormatDetector,
    error::{MusicDlError, Result},
    filter::Filter,
    types::AudioFormat,
};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::header::HeaderMap;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// The core trait that every music source must implement.
///
/// This follows the Template Method pattern with a two-phase flow:
/// 1. **Search phase**: `construct_search_urls` → fetch → `parse_search_result`
///    returns basic song metadata (download_url may be None)
/// 2. **Parse phase**: `parse_download_url` resolves the actual download URL,
///    often trying multiple quality levels and third-party API fallbacks
///
/// The default `search` implementation orchestrates both phases.
#[async_trait]
pub trait MusicSource: Send + Sync {
    /// A short identifier for this source (e.g. "netease", "migu").
    fn source_name(&self) -> &str;

    /// Construct the list of search URLs to fetch for the given keyword.
    fn construct_search_urls(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
    ) -> Vec<SearchUrl>;

    /// Parse a single search result page (the HTTP response body) into a list
    /// of `SongInfo` items. At this stage, download_url may be None — it gets
    /// resolved in Phase 2 by `parse_download_url`.
    fn parse_search_result(&self, body: &str) -> Result<Vec<SongInfo>>;

    /// Resolve the download URL for a single song.
    ///
    /// This is where quality-level iteration and third-party API fallback chains
    /// live. The default implementation is a no-op (assumes URL already resolved).
    async fn parse_download_url(
        &self,
        _song_info: &mut SongInfo,
        _http: &HttpClient,
    ) -> Result<()> {
        Ok(())
    }

    /// Full search flow: construct URLs → fetch in parallel → parse →
    /// resolve download URLs → dedup → assign paths.
    ///
    /// Override only if the source needs a fundamentally different flow.
    async fn search(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
        http: &HttpClient,
    ) -> Result<Vec<SongInfo>> {
        let urls = self.construct_search_urls(keyword, params, filters);
        let search_headers = self.search_headers();
        let concurrency = params.concurrency;

        log::info!(
            "[{}] Starting search ({} URLs)",
            self.source_name(),
            urls.len()
        );

        if urls.is_empty() {
            return Ok(vec![]);
        }

        // Phase 1: Fetch all pages concurrently
        let fetch_results: Vec<Option<Result<String>>> = stream::iter(urls)
            .map(|search_url| {
                let http = http.clone();
                let headers = search_headers.clone();
                async move {
                    let result = match search_url.method {
                        crate::types::HttpMethod::Get => {
                            http.get_text(&search_url.url, headers).await
                        }
                        crate::types::HttpMethod::Post => {
                            if let Some(json) = &search_url.json {
                                // POST with JSON body — send as application/json
                                match http
                                    .post_json::<serde_json::Value>(&search_url.url, json, headers)
                                    .await
                                {
                                    Ok(val) => Ok(val.to_string()),
                                    Err(e) => Err(e),
                                }
                            } else if let Some(form) = &search_url.form {
                                // POST with form data
                                let form_pairs: Vec<(&str, &str)> =
                                    form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                                match http
                                    .post_form_json::<serde_json::Value>(
                                        &search_url.url,
                                        &form_pairs,
                                        headers,
                                    )
                                    .await
                                {
                                    Ok(val) => Ok(val.to_string()),
                                    Err(e) => Err(e),
                                }
                            } else {
                                // Plain text POST (backwards compat)
                                let body = search_url.body.as_deref().unwrap_or("");
                                http.post_text(&search_url.url, body, headers).await
                            }
                        }
                    };
                    match result {
                        Ok(text) => {
                            log::debug!("Search request succeeded: {}", search_url.url);
                            Some(Ok(text))
                        }
                        Err(e) => {
                            log::warn!("Search request failed: {} - {}", search_url.url, e);
                            Some(Err(e))
                        }
                    }
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        // Separate bodies and errors
        let mut bodies = Vec::new();
        let mut last_error = None;
        for result in fetch_results.into_iter().flatten() {
            match result {
                Ok(text) => bodies.push(text),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        if bodies.is_empty() {
            if let Some(e) = last_error {
                return Err(e);
            }
            return Ok(vec![]);
        }

        // Phase 2: Parse search results (basic metadata)
        let mut all_songs = Vec::new();
        for body in &bodies {
            match self.parse_search_result(body) {
                Ok(songs) => all_songs.extend(songs),
                Err(e) => {
                    log::warn!(
                        "[{}] Failed to parse search result: {}",
                        self.source_name(),
                        e
                    );
                }
            }
        }

        // Phase 3: Resolve download URLs for each song
        for song in &mut all_songs {
            if let Err(e) = self.parse_download_url(song, http).await {
                log::warn!(
                    "[{}] Failed to resolve download URL for '{}': {}",
                    self.source_name(),
                    song.song_name.as_deref().unwrap_or("unknown"),
                    e
                );
            }
        }

        // Filter out songs without valid download URLs
        let all_songs: Vec<SongInfo> = all_songs
            .into_iter()
            .filter(|s| s.with_valid_download_url())
            .collect();

        // Dedup and assign paths
        let songs = dedup_by_identifier(all_songs);
        let songs = assign_file_paths(songs, self.source_name(), &params.work_dir, keyword);

        log::info!(
            "[{}] Search completed ({} results)",
            self.source_name(),
            songs.len()
        );

        Ok(songs)
    }

    /// Full download flow: download audio, cover, and/or lyrics based on `DownloadContent` flags.
    ///
    /// Handles HTTP streaming, HLS, and pre-downloaded content for audio.
    /// Downloads cover images and includes lyrics based on content flags.
    async fn download(
        &self,
        songs: &[SongInfo],
        config: &DownloadConfig,
        http: &HttpClient,
    ) -> Result<Vec<DownloadedSongInfo>> {
        let dl_headers = self.download_headers();
        let concurrency = config.concurrency;
        let content = config.content;

        log::info!(
            "[{}] Starting download ({} songs, audio={}, cover={}, lyric={})",
            self.source_name(),
            songs.len(),
            content.audio,
            content.cover,
            content.lyric,
        );

        let results: Vec<Option<DownloadedSongInfo>> = stream::iter(songs.iter().cloned())
            .map(|song| {
                let http = http.clone();
                let headers = dl_headers.clone();
                let content = content;
                async move {
                    // Download audio if requested
                    let (audio_data, audio_format) = if content.audio {
                        match song.protocol {
                            DownloadProtocol::Hls => {
                                if let Some(url) = &song.download_url {
                                    log::warn!(
                                        "[{}] HLS download not yet fully implemented: {}",
                                        song.source,
                                        url
                                    );
                                }
                                (None, AudioFormat::Unknown)
                            }
                            DownloadProtocol::Http if song.downloaded_contents.is_some() => {
                                let data = song.downloaded_contents.clone().unwrap();
                                let format = AudioFormatDetector::detect(&data)
                                    .unwrap_or(AudioFormat::Unknown);
                                (Some(data), format)
                            }
                            DownloadProtocol::Http => {
                                if let Some(url) = &song.download_url {
                                    let req_headers = song
                                        .default_download_headers
                                        .as_ref()
                                        .unwrap_or(&headers)
                                        .clone();

                                    match http.get_bytes(url, req_headers).await {
                                        Ok(bytes) => {
                                            let format = AudioFormatDetector::detect(&bytes)
                                                .unwrap_or(AudioFormat::Unknown);
                                            log::debug!(
                                                "Download succeeded: {} (format: {:?})",
                                                url,
                                                format
                                            );
                                            (Some(bytes), format)
                                        }
                                        Err(e) => {
                                            log::warn!("Download failed: {} - {}", url, e);
                                            (None, AudioFormat::Unknown)
                                        }
                                    }
                                } else {
                                    (None, AudioFormat::Unknown)
                                }
                            }
                        }
                    } else {
                        (None, AudioFormat::Unknown)
                    };

                    // If audio was requested but failed, skip this song
                    if content.audio && audio_data.is_none() {
                        return None;
                    }

                    // Download cover if requested and URL exists
                    let cover_data = if content.cover {
                        if let Some(cover_url) = &song.cover_url {
                            match http.get_bytes(cover_url, HeaderMap::new()).await {
                                Ok(bytes) if !bytes.is_empty() => Some(bytes),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Include lyric if requested and exists
                    let lyric_data = if content.lyric {
                        song.lyric.clone()
                    } else {
                        None
                    };

                    Some(DownloadedSongInfo {
                        song_info: song,
                        data: audio_data.unwrap_or_default(),
                        format: audio_format,
                        cover_data,
                        lyric_data,
                    })
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        let results: Vec<DownloadedSongInfo> = results.into_iter().flatten().collect();
        let downloaded_count = results.len();
        log::info!(
            "[{}] Download completed ({}/{} succeeded)",
            self.source_name(),
            downloaded_count,
            songs.len()
        );

        Ok(results)
    }

    /// Default headers to use for search requests.
    fn search_headers(&self) -> HeaderMap {
        HeaderMap::new()
    }

    /// Default headers to use for download requests.
    fn download_headers(&self) -> HeaderMap {
        HeaderMap::new()
    }

    /// Default headers to use for parse/resolve requests.
    fn parse_headers(&self) -> HeaderMap {
        HeaderMap::new()
    }

    /// Build the filter rules specific to this source.
    fn build_filter(&self) -> Filter {
        Filter::new()
    }

    /// Parse a playlist URL into a list of songs.
    ///
    /// Default: not supported.
    async fn parse_playlist(
        &self,
        _playlist_url: &str,
        _http: &HttpClient,
    ) -> Result<Vec<SongInfo>> {
        Err(MusicDlError::Other(format!(
            "Playlist parsing not supported by {}",
            self.source_name()
        )))
    }
}

/// Remove duplicate `SongInfo` items by their `identifier` field, keeping the first occurrence.
pub fn dedup_by_identifier(songs: Vec<SongInfo>) -> Vec<SongInfo> {
    let mut seen = std::collections::HashSet::new();
    songs
        .into_iter()
        .filter(|info| seen.insert(info.identifier.clone()))
        .collect()
}

/// Assign unique file paths to each `SongInfo`.
///
/// Creates a timestamped directory under `work_dir/source/` and assigns
/// filenames based on song metadata.
pub fn assign_file_paths(
    songs: Vec<SongInfo>,
    source: &str,
    work_dir: &Path,
    keyword: &str,
) -> Vec<SongInfo> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
    let dir_name = format!("{} {}", timestamp, keyword);
    let dir = work_dir.join(source).join(dir_name);

    songs
        .into_iter()
        .enumerate()
        .map(|(idx, mut song)| {
            song.work_dir = dir.clone();
            song.save_name = Some(format!("{:08}", idx + 1));
            song.compute_save_path();
            song
        })
        .collect()
}

/// Registry of available music sources.
pub struct SourceRegistry {
    builders: HashMap<String, Box<dyn Fn() -> Box<dyn MusicSource> + Send + Sync>>,
}

impl SourceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            builders: HashMap::new(),
        }
    }

    /// Register a source constructor under a given name.
    pub fn register<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn() -> Box<dyn MusicSource> + Send + Sync + 'static,
    {
        self.builders.insert(name.into(), Box::new(factory));
    }

    /// Create a source instance by name.
    pub fn create(&self, name: &str) -> Option<Box<dyn MusicSource>> {
        self.builders.get(name).map(|f| f())
    }

    /// List all registered source names.
    pub fn source_names(&self) -> Vec<&str> {
        self.builders.keys().map(|s| s.as_str()).collect()
    }

    /// Convenience: a registry pre-loaded with the built-in sources.
    pub fn with_builtin_sources() -> Self {
        let mut reg = Self::new();
        reg.register("netease", || {
            Box::new(crate::sources::NeteaseMusicSource::new())
        });
        reg.register(
            "kugou",
            || Box::new(crate::sources::KugouMusicSource::new()),
        );
        reg.register("kuwo", || Box::new(crate::sources::KuwoMusicSource::new()));
        reg.register("qianqian", || {
            Box::new(crate::sources::QianqianMusicSource::new())
        });
        reg
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level client for searching and downloading music from multiple sources.
pub struct MusicClient {
    registry: SourceRegistry,
    http: HttpClient,
    search_params: SearchParams,
    download_config: DownloadConfig,
}

/// The outcome of a search for a single source.
#[derive(Debug)]
pub enum SearchResult {
    /// Search succeeded, contains found songs.
    Ok(Vec<SongInfo>),
    /// Search failed, contains a human-readable error message.
    Err(String),
}

impl SearchResult {
    /// Returns `true` if this is a successful result.
    pub fn is_ok(&self) -> bool {
        matches!(self, SearchResult::Ok(_))
    }

    /// Returns the list of songs if successful, or an empty list otherwise.
    pub fn unwrap_or_default(&self) -> &[SongInfo] {
        match self {
            SearchResult::Ok(songs) => songs,
            SearchResult::Err(_) => &[],
        }
    }
}

impl MusicClient {
    /// Create a builder for configuring a `MusicClient`.
    pub fn builder() -> MusicClientBuilder {
        MusicClientBuilder::default()
    }

    /// Get a reference to the underlying HTTP client.
    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    /// Search for songs across the specified sources concurrently.
    ///
    /// Returns a map from source name to `SearchResult`.
    pub async fn search(&self, keyword: &str, sources: &[&str]) -> HashMap<String, SearchResult> {
        let mut handles = Vec::new();

        for name in sources {
            if let Some(source) = self.registry.create(name) {
                let keyword = keyword.to_string();
                let params = self.search_params.clone();
                let http = self.http.clone();
                let name_owned = name.to_string();
                handles.push(tokio::spawn(async move {
                    let result = source
                        .search(&keyword, &params, &Filters::new(), &http)
                        .await;
                    (name_owned, result)
                }));
            } else {
                log::warn!("Unknown source '{}', skipping", name);
            }
        }

        let mut results = HashMap::new();
        for handle in handles {
            match handle.await {
                Ok((name, result)) => {
                    results.insert(
                        name,
                        match result {
                            Ok(songs) => SearchResult::Ok(songs),
                            Err(e) => SearchResult::Err(e.to_string()),
                        },
                    );
                }
                Err(e) => {
                    log::error!("Search task panicked: {}", e);
                }
            }
        }

        results
    }

    /// Search for songs with custom filters.
    pub async fn search_with_filters(
        &self,
        keyword: &str,
        sources: &[&str],
        filters: &HashMap<String, Filters>,
    ) -> HashMap<String, SearchResult> {
        let mut handles = Vec::new();

        for name in sources {
            if let Some(source) = self.registry.create(name) {
                let keyword = keyword.to_string();
                let params = self.search_params.clone();
                let http = self.http.clone();
                let name_owned = name.to_string();
                let source_filters = filters.get(*name).cloned().unwrap_or_default();
                handles.push(tokio::spawn(async move {
                    let result = source
                        .search(&keyword, &params, &source_filters, &http)
                        .await;
                    (name_owned, result)
                }));
            }
        }

        let mut results = HashMap::new();
        for handle in handles {
            match handle.await {
                Ok((name, result)) => {
                    results.insert(
                        name,
                        match result {
                            Ok(songs) => SearchResult::Ok(songs),
                            Err(e) => SearchResult::Err(e.to_string()),
                        },
                    );
                }
                Err(e) => {
                    log::error!("Search task panicked: {}", e);
                }
            }
        }

        results
    }

    /// Download songs for a single source.
    pub async fn download(
        &self,
        source_name: &str,
        songs: &[SongInfo],
    ) -> Result<Vec<DownloadedSongInfo>> {
        let source = self
            .registry
            .create(source_name)
            .ok_or_else(|| MusicDlError::UnknownSource(source_name.to_string()))?;
        source
            .download(songs, &self.download_config, &self.http)
            .await
    }

    /// Download songs, automatically routing each to its source.
    pub async fn download_all(
        &self,
        song_infos: &HashMap<String, Vec<SongInfo>>,
    ) -> Vec<DownloadedSongInfo> {
        let mut all_results = Vec::new();
        for (source_name, songs) in song_infos {
            match self.download(source_name, songs).await {
                Ok(results) => all_results.extend(results),
                Err(e) => {
                    log::error!("[{}] Download failed: {}", source_name, e);
                }
            }
        }
        all_results
    }

    /// Parse a playlist URL, trying each source until one succeeds.
    pub async fn parse_playlist(&self, playlist_url: &str) -> Result<Vec<SongInfo>> {
        for name in self.registry.source_names() {
            if let Some(source) = self.registry.create(name) {
                match source.parse_playlist(playlist_url, &self.http).await {
                    Ok(songs) if !songs.is_empty() => return Ok(songs),
                    _ => continue,
                }
            }
        }
        Err(MusicDlError::Other(
            "No source could parse the given playlist URL".to_string(),
        ))
    }

    /// Get the list of registered source names.
    pub fn source_names(&self) -> Vec<&str> {
        self.registry.source_names()
    }
}

/// Builder for `MusicClient`.
#[derive(Default)]
pub struct MusicClientBuilder {
    registry: SourceRegistry,
    http_builder: super::http::HttpClientBuilder,
    search_params: SearchParams,
    download_config: DownloadConfig,
}

impl MusicClientBuilder {
    /// Use the built-in source registry (netease, kugou, kuwo, qianqian).
    pub fn with_builtin_sources(mut self) -> Self {
        self.registry = SourceRegistry::with_builtin_sources();
        self
    }

    /// Use a custom source registry.
    pub fn registry(mut self, registry: SourceRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Register a single source.
    pub fn register_source<F>(mut self, name: impl Into<String>, factory: F) -> Self
    where
        F: Fn() -> Box<dyn MusicSource> + Send + Sync + 'static,
    {
        self.registry.register(name, factory);
        self
    }

    /// Set the search limits (approximate max songs per source).
    pub fn search_limits(mut self, limits: usize) -> Self {
        self.search_params.search_limits = limits;
        self
    }

    /// Set the search size per page.
    pub fn search_size_per_page(mut self, size: usize) -> Self {
        self.search_params.search_size_per_page = size;
        self
    }

    /// Set the base work directory for saving songs.
    pub fn work_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.search_params.work_dir = dir.into();
        self
    }

    /// Set the concurrency for search requests per source.
    pub fn search_concurrency(mut self, n: usize) -> Self {
        self.search_params.concurrency = n;
        self
    }

    /// Set the concurrency for downloads.
    pub fn download_concurrency(mut self, n: usize) -> Self {
        self.download_config.concurrency = n;
        self
    }

    /// Set what content to download (audio, cover, lyric).
    pub fn download_content(mut self, content: DownloadContent) -> Self {
        self.download_config.content = content;
        self
    }

    /// Set the maximum number of retries for HTTP requests.
    pub fn max_retries(mut self, n: usize) -> Self {
        self.http_builder = self.http_builder.max_retries(n);
        self
    }

    /// Set a proxy URL for all HTTP requests.
    pub fn proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.http_builder = self.http_builder.proxy(proxy_url);
        self
    }

    /// Set the HTTP request timeout.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.http_builder = self.http_builder.timeout(timeout);
        self
    }

    /// Build the `MusicClient`.
    pub fn build(self) -> Result<MusicClient> {
        let http = self.http_builder.build()?;
        Ok(MusicClient {
            registry: self.registry,
            http,
            search_params: self.search_params,
            download_config: self.download_config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_by_identifier() {
        let songs = vec![
            SongInfo::new("migu", "id1"),
            SongInfo::new("migu", "id2"),
            SongInfo::new("migu", "id1"), // duplicate
        ];
        let deduped = dedup_by_identifier(songs);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_assign_file_paths() {
        let songs = vec![SongInfo::new("migu", "id1"), SongInfo::new("migu", "id2")];
        let assigned = assign_file_paths(songs, "migu", &PathBuf::from("output"), "test");
        assert_eq!(assigned.len(), 2);
        assert_eq!(assigned[0].save_name.as_deref(), Some("00000001"));
        assert_eq!(assigned[1].save_name.as_deref(), Some("00000002"));
        assert!(assigned[0].work_dir.to_string_lossy().contains("migu"));
        assert!(assigned[0].work_dir.to_string_lossy().contains("test"));
    }

    #[test]
    fn test_source_registry() {
        struct MockSource;
        #[async_trait]
        impl MusicSource for MockSource {
            fn source_name(&self) -> &str {
                "mock"
            }
            fn construct_search_urls(
                &self,
                _: &str,
                _: &SearchParams,
                _: &Filters,
            ) -> Vec<SearchUrl> {
                vec![]
            }
            fn parse_search_result(&self, _: &str) -> Result<Vec<SongInfo>> {
                Ok(vec![])
            }
        }

        let mut reg = SourceRegistry::new();
        reg.register("mock", || Box::new(MockSource));
        assert!(reg.create("mock").is_some());
        assert!(reg.create("nonexistent").is_none());
        assert_eq!(reg.source_names(), vec!["mock"]);
    }

    #[tokio::test]
    async fn test_music_client_builder() {
        let client = MusicClient::builder()
            .with_builtin_sources()
            .search_limits(5)
            .build()
            .unwrap();

        let names = client.source_names();
        assert!(names.contains(&"netease"));
        assert!(names.contains(&"kugou"));
        assert!(names.contains(&"kuwo"));
        assert!(names.contains(&"qianqian"));
    }
}
