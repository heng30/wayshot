//! Core trait, registry, and orchestrator for image sources.
//!
//! This module contains:
//! - `ImageSource` trait — the central abstraction that every source must implement
//! - `SourceRegistry` — runtime registry of available sources
//! - `ImageClient` — top-level orchestrator for multi-source search and download
//! - `ImageClientBuilder` — builder for configuring `ImageClient`

use super::http::HttpClient;
use crate::{
    detect::ImageFormatDetector,
    error::{ImageDlError, Result},
    filter::Filter,
    types::{DownloadConfig, DownloadedInfo, Filters, ImageInfo, SearchParams, SearchUrl},
};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::header::HeaderMap;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// The core trait that every image source must implement.
///
/// This follows the Template Method pattern: the default implementations of
/// `search` and `download` orchestrate the full flow (URL construction →
/// parallel HTTP fetch → parse → dedup → path assignment → download),
/// while implementors provide source-specific logic via the required methods.
#[async_trait]
pub trait ImageSource: Send + Sync {
    /// A short identifier for this source (e.g. "bing", "baidu").
    fn source_name(&self) -> &str;

    /// Construct the list of search URLs to fetch for the given keyword and parameters.
    ///
    /// Each URL represents one page of results. The `params.search_limits`
    /// indicates approximately how many images the caller wants; the source
    /// should over-fetch slightly (the Python code uses 1.2x) to account for
    /// duplicates and failed downloads.
    fn construct_search_urls(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
    ) -> Vec<SearchUrl>;

    /// Parse a single search result page (the HTTP response body) into a list
    /// of `ImageInfo` items.
    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>>;

    /// Build the filter rules specific to this source.
    /// Returns a Filter with no rules by default; sources override this.
    fn build_filter(&self) -> Filter {
        Filter::new()
    }

    /// Default headers to use for search requests.
    fn search_headers(&self) -> HeaderMap {
        HeaderMap::new()
    }

    /// Default headers to use for download requests.
    fn download_headers(&self) -> HeaderMap {
        HeaderMap::new()
    }

    /// Full search flow: construct URLs → fetch in parallel → parse → dedup → assign paths.
    ///
    /// Override only if the source needs a fundamentally different flow.
    async fn search(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
        http: &HttpClient,
    ) -> Result<Vec<ImageInfo>> {
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

        // Phase 1: Fetch all pages concurrently (only needs HttpClient, which is Send + Sync)
        // Track both successful bodies and the last error for better error reporting.
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
                            let body = search_url.body.as_deref().unwrap_or("");
                            http.post_text(&search_url.url, body, headers).await
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
                Err(e) => last_error = Some(e),
            }
        }

        // If all requests failed, return the last error
        if bodies.is_empty() {
            if let Some(e) = last_error {
                return Err(e);
            }
            // No URLs were attempted
            return Ok(vec![]);
        }

        // Phase 2: Parse sequentially (needs &self, but we're on the original task)
        let mut all_infos = Vec::new();
        for body in &bodies {
            match self.parse_search_result(body) {
                Ok(infos) => all_infos.extend(infos),
                Err(e) => {
                    log::warn!(
                        "[{}] Failed to parse search result: {}",
                        self.source_name(),
                        e
                    );
                }
            }
        }

        // Deduplicate by identifier
        let infos = dedup_by_identifier(all_infos);

        // Assign file paths
        let infos = assign_file_paths(infos, self.source_name(), &params.work_dir, keyword);

        log::info!(
            "[{}] Search completed ({} results)",
            self.source_name(),
            infos.len()
        );

        Ok(infos)
    }

    /// Full download flow: try each candidate URL per image until one succeeds.
    ///
    /// For each image, candidate URLs are tried in order. The first URL that
    /// returns valid image data wins. Downloads are parallelized across images.
    async fn download(
        &self,
        images: &[ImageInfo],
        config: &DownloadConfig,
        http: &HttpClient,
    ) -> Result<Vec<DownloadedInfo>> {
        let dl_headers = self.download_headers();
        let concurrency = config.concurrency;

        log::info!(
            "[{}] Starting download ({} images)",
            self.source_name(),
            images.len()
        );

        let results: Vec<Option<DownloadedInfo>> = stream::iter(images.iter().cloned())
            .map(|image| {
                let http = http.clone();
                let headers = dl_headers.clone();
                async move {
                    for url in &image.candidate_download_urls {
                        match http.get_bytes(url, headers.clone()).await {
                            Ok(bytes) => {
                                if let Some(format) = ImageFormatDetector::detect(&bytes) {
                                    log::debug!(
                                        "Download succeeded: {} (format: {:?})",
                                        url,
                                        format
                                    );
                                    return Some(DownloadedInfo {
                                        image_info: image,
                                        data: bytes,
                                        format,
                                    });
                                } else {
                                    log::warn!(
                                        "Downloaded data is not a recognized image format: {}",
                                        url
                                    );
                                }
                            }
                            Err(e) => {
                                log::warn!("Download attempt failed: {} - {}", url, e);
                            }
                        }
                    }
                    None
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        let results: Vec<DownloadedInfo> = results.into_iter().flatten().collect();
        let downloaded_count = results.len();
        log::info!(
            "[{}] Download completed ({}/{} succeeded)",
            self.source_name(),
            downloaded_count,
            images.len()
        );

        Ok(results)
    }
}

/// Remove duplicate `ImageInfo` items by their `identifier` field, keeping the first occurrence.
pub fn dedup_by_identifier(infos: Vec<ImageInfo>) -> Vec<ImageInfo> {
    let mut seen = std::collections::HashSet::new();
    infos
        .into_iter()
        .filter(|info| seen.insert(info.identifier.clone()))
        .collect()
}

/// Assign unique file paths to each `ImageInfo`.
///
/// Creates a timestamped directory under `work_dir/source/` and assigns
/// sequential filenames like `00000001`, `00000002`, etc.
pub fn assign_file_paths(
    infos: Vec<ImageInfo>,
    source: &str,
    work_dir: &Path,
    keyword: &str,
) -> Vec<ImageInfo> {
    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
    let dir_name = format!("{} {}", timestamp, keyword);
    let dir = work_dir.join(source).join(dir_name);

    infos
        .into_iter()
        .enumerate()
        .map(|(idx, mut info)| {
            let save_name = format!("{:08}", idx + 1);
            info.work_dir = dir.clone();
            info.save_name = Some(save_name);
            info.save_path = Some(dir.join(info.save_name.as_ref().unwrap()));
            info
        })
        .collect()
}

/// Registry of available image sources.
///
/// Replaces Python's `ImageClientBuilder` / `BaseModuleBuilder` pattern.
/// Sources are registered by name and created on demand.
pub struct SourceRegistry {
    builders: HashMap<String, Box<dyn Fn() -> Box<dyn ImageSource> + Send + Sync>>,
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
        F: Fn() -> Box<dyn ImageSource> + Send + Sync + 'static,
    {
        self.builders.insert(name.into(), Box::new(factory));
    }

    /// Create a source instance by name.
    pub fn create(&self, name: &str) -> Option<Box<dyn ImageSource>> {
        self.builders.get(name).map(|f| f())
    }

    /// List all registered source names.
    pub fn source_names(&self) -> Vec<&str> {
        self.builders.keys().map(|s| s.as_str()).collect()
    }

    /// Convenience: a registry pre-loaded with the built-in sources.
    pub fn with_builtin_sources() -> Self {
        let mut reg = Self::new();
        // Search engines
        reg.register("bing", || Box::new(crate::sources::BingImageSource::new()));
        reg.register(
            "baidu",
            || Box::new(crate::sources::BaiduImageSource::new()),
        );
        reg.register("google", || {
            Box::new(crate::sources::GoogleImageSource::new())
        });
        reg.register("duckduckgo", || {
            Box::new(crate::sources::DuckduckgoImageSource::new())
        });
        reg.register("i360", || Box::new(crate::sources::I360ImageSource::new()));
        reg.register(
            "sogou",
            || Box::new(crate::sources::SogouImageSource::new()),
        );
        reg.register("yandex", || {
            Box::new(crate::sources::YandexImageSource::new())
        });
        reg.register(
            "weibo",
            || Box::new(crate::sources::WeiboImageSource::new()),
        );
        // Stock photo / media
        reg.register("pexels", || {
            Box::new(crate::sources::PexelsImageSource::new())
        });
        reg.register("pixabay", || {
            Box::new(crate::sources::PixabayImageSource::new())
        });
        reg.register("flickr", || {
            Box::new(crate::sources::FlickrImageSource::new())
        });
        reg.register("wallhaven", || {
            Box::new(crate::sources::WallhavenImageSource::new())
        });
        reg.register("stocksnap", || {
            Box::new(crate::sources::StocksnapImageSource::new())
        });
        reg.register("everypixel", || {
            Box::new(crate::sources::EverypixelImageSource::new())
        });
        reg.register("openverse", || {
            Box::new(crate::sources::OpenverseImageSource::new())
        });
        reg.register("freeimages", || {
            Box::new(crate::sources::FreeimagesImageSource::new())
        });
        reg.register("gratisography", || {
            Box::new(crate::sources::GratisographyImageSource::new())
        });
        reg.register("picjumbo", || {
            Box::new(crate::sources::PicjumboImageSource::new())
        });
        reg.register("freenaturestock", || {
            Box::new(crate::sources::FreenaturestockImageSource::new())
        });
        reg.register("foodiesfeed", || {
            Box::new(crate::sources::FoodiesfeedImageSource::new())
        });
        // Image boards (Moebooru)
        reg.register("konachan", || {
            Box::new(crate::sources::KonachanImageSource::new())
        });
        reg.register("safebooru", || {
            Box::new(crate::sources::SafebooruImageSource::new())
        });
        reg.register(
            "yande",
            || Box::new(crate::sources::YandeImageSource::new()),
        );
        reg.register("gelbooru", || {
            Box::new(crate::sources::GelbooruImageSource::new())
        });
        // Museum / art
        reg.register("aic", || Box::new(crate::sources::AicImageSource::new()));
        reg.register("clevelandart", || {
            Box::new(crate::sources::ClevelandArtImageSource::new())
        });
        reg.register("metropolitan", || {
            Box::new(crate::sources::MetropolitanImageSource::new())
        });
        reg.register("smk", || Box::new(crate::sources::SmkImageSource::new()));
        reg.register("vam", || Box::new(crate::sources::VamImageSource::new()));
        reg.register("wellcome", || {
            Box::new(crate::sources::WellcomeImageSource::new())
        });
        // Nature / science
        reg.register("inaturalist", || {
            Box::new(crate::sources::INaturalistImageSource::new())
        });
        reg.register("nasa", || Box::new(crate::sources::NasaImageSource::new()));
        reg.register("gbif", || Box::new(crate::sources::GbifImageSource::new()));
        // Other
        reg.register("bluesky", || {
            Box::new(crate::sources::BlueskyImageSource::new())
        });
        reg.register("huaban", || {
            Box::new(crate::sources::HuabanImageSource::new())
        });
        reg.register("internetarchive", || {
            Box::new(crate::sources::InternetArchiveImageSource::new())
        });
        reg.register("lifeofpix", || {
            Box::new(crate::sources::LifeOfPixImageSource::new())
        });
        reg.register("locgov", || {
            Box::new(crate::sources::LocGovImageSource::new())
        });
        reg.register("openlibrary", || {
            Box::new(crate::sources::OpenLibraryImageSource::new())
        });
        reg.register("wikipedia", || {
            Box::new(crate::sources::WikipediaImageSource::new())
        });
        reg.register("dimtown", || {
            Box::new(crate::sources::DimtownImageSource::new())
        });
        reg
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level client for searching and downloading images from multiple sources.
pub struct ImageClient {
    registry: SourceRegistry,
    http: HttpClient,
    search_params: SearchParams,
    download_config: DownloadConfig,
}

/// The outcome of a search for a single source.
#[derive(Debug)]
pub enum SearchResult {
    /// Search succeeded, contains found images.
    Ok(Vec<ImageInfo>),
    /// Search failed, contains a human-readable error message.
    Err(String),
}

impl SearchResult {
    /// Returns `true` if this is a successful result.
    pub fn is_ok(&self) -> bool {
        matches!(self, SearchResult::Ok(_))
    }

    /// Returns the list of images if successful, or an empty list otherwise.
    pub fn unwrap_or_default(&self) -> &[ImageInfo] {
        match self {
            SearchResult::Ok(imgs) => imgs,
            SearchResult::Err(_) => &[],
        }
    }
}

impl ImageClient {
    /// Create a builder for configuring an `ImageClient`.
    pub fn builder() -> ImageClientBuilder {
        ImageClientBuilder::default()
    }

    /// Get a reference to the underlying HttpClient (e.g. for downloading thumbnails with the same proxy).
    pub fn http(&self) -> &HttpClient {
        &self.http
    }

    /// Search for images across the specified sources concurrently.
    ///
    /// Returns a map from source name to `SearchResult`, which indicates
    /// whether each source succeeded or failed (with a reason).
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
                            Ok(imgs) => SearchResult::Ok(imgs),
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

    /// Search for images with custom filters.
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
                            Ok(imgs) => SearchResult::Ok(imgs),
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

    /// Download images for a single source.
    pub async fn download(
        &self,
        source_name: &str,
        images: &[ImageInfo],
    ) -> Result<Vec<DownloadedInfo>> {
        let source = self
            .registry
            .create(source_name)
            .ok_or_else(|| ImageDlError::UnknownSource(source_name.to_string()))?;
        source
            .download(images, &self.download_config, &self.http)
            .await
    }

    /// Download images, automatically routing each to its source.
    pub async fn download_all(
        &self,
        image_infos: &HashMap<String, Vec<ImageInfo>>,
    ) -> Vec<DownloadedInfo> {
        let mut all_results = Vec::new();
        for (source_name, images) in image_infos {
            match self.download(source_name, images).await {
                Ok(results) => all_results.extend(results),
                Err(e) => {
                    log::error!("[{}] Download failed: {}", source_name, e);
                }
            }
        }
        all_results
    }

    /// Get the list of registered source names.
    pub fn source_names(&self) -> Vec<&str> {
        self.registry.source_names()
    }
}

/// Builder for `ImageClient`.
#[derive(Default)]
pub struct ImageClientBuilder {
    registry: SourceRegistry,
    http_builder: super::http::HttpClientBuilder,
    search_params: SearchParams,
    download_config: DownloadConfig,
}

impl ImageClientBuilder {
    /// Use the built-in source registry (Bing, Baidu, Unsplash, Google).
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
        F: Fn() -> Box<dyn ImageSource> + Send + Sync + 'static,
    {
        self.registry.register(name, factory);
        self
    }

    /// Set the search limits (approximate max images per source).
    pub fn search_limits(mut self, limits: usize) -> Self {
        self.search_params.search_limits = limits;
        self
    }

    /// Set the base work directory for saving images.
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

    /// Build the `ImageClient`.
    pub fn build(self) -> Result<ImageClient> {
        let http = self.http_builder.build()?;
        Ok(ImageClient {
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
        let infos = vec![
            ImageInfo::with_identifier("bing", vec!["http://a.jpg".into()], "id1"),
            ImageInfo::with_identifier("bing", vec!["http://b.jpg".into()], "id2"),
            ImageInfo::with_identifier("bing", vec!["http://a.jpg".into()], "id1"), // duplicate
        ];
        let deduped = dedup_by_identifier(infos);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_assign_file_paths() {
        let infos = vec![
            ImageInfo::new("bing", vec!["http://a.jpg".into()]),
            ImageInfo::new("bing", vec!["http://b.jpg".into()]),
        ];
        let assigned = assign_file_paths(infos, "bing", &PathBuf::from("output"), "cats");
        assert_eq!(assigned.len(), 2);
        assert_eq!(assigned[0].save_name.as_deref(), Some("00000001"));
        assert_eq!(assigned[1].save_name.as_deref(), Some("00000002"));
        assert!(assigned[0].work_dir.to_string_lossy().contains("bing"));
        assert!(assigned[0].work_dir.to_string_lossy().contains("cats"));
    }

    #[test]
    fn test_source_registry() {
        struct MockSource;
        #[async_trait]
        impl ImageSource for MockSource {
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
            fn parse_search_result(&self, _: &str) -> Result<Vec<ImageInfo>> {
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
    async fn test_image_client_builder() {
        let client = ImageClient::builder()
            .with_builtin_sources()
            .search_limits(100)
            .build()
            .unwrap();

        let names = client.source_names();
        assert!(names.contains(&"bing"));
        assert!(names.contains(&"baidu"));
        assert!(names.contains(&"pexels"));
        assert!(names.contains(&"google"));
    }
}
