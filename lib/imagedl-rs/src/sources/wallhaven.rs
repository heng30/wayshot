//! Wallhaven image search source.
//!
//! Replaces Python's `WallhavenImageClient`. Uses Wallhaven's REST API
//! with JSON response parsing. Requires User-Agent and Referer headers.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Wallhaven API requests.
const DEFAULT_PAGE_SIZE: usize = 24;

/// Wallhaven image search source.
pub struct WallhavenImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl WallhavenImageSource {
    /// Create a new Wallhaven image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            "accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        search_headers.insert(
            "referer",
            "https://wallhaven.cc/".parse().unwrap(),
        );
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            "referer",
            "https://wallhaven.cc/".parse().unwrap(),
        );
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for WallhavenImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for WallhavenImageSource {
    fn source_name(&self) -> &str {
        "wallhaven"
    }

    fn search_headers(&self) -> HeaderMap {
        self.search_headers.clone()
    }

    fn download_headers(&self) -> HeaderMap {
        self.download_headers.clone()
    }

    fn construct_search_urls(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
    ) -> Vec<SearchUrl> {
        let base_url = "https://wallhaven.cc/api/v1/search?";
        let num_pages = ((params.search_limits as f64 * 1.2 / DEFAULT_PAGE_SIZE as f64).ceil())
            as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}q={}&categories=111&purity=100&sorting=relevance&order=desc&page={}",
                    base_url,
                    urlencoding::encode(keyword),
                    pn + 1,
                );
                for (key, value) in filters {
                    if key != "q" && key != "page" && key != "categories"
                        && key != "purity" && key != "sorting" && key != "order"
                        && let Some(s) = value.as_str()
                    {
                        url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
                    }
                }
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let search_result: Value = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        let mut image_infos = Vec::new();

        let data = search_result.get("data").and_then(|v| v.as_array());
        if let Some(items) = data {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Build candidate URLs: path (full image), then thumbs
                let mut candidate_urls = Vec::new();

                // Full image path
                if let Some(path) = item.get("path").and_then(|v| v.as_str()) {
                    if path.starts_with("http") {
                        candidate_urls.push(path.to_string());
                    }
                }

                // Thumbnail URLs
                if let Some(thumbs) = item.get("thumbs").and_then(|v| v.as_object()) {
                    for key in &["original", "large", "small"] {
                        if let Some(url) = thumbs.get(*key).and_then(|v| v.as_str()) {
                            if url.starts_with("http") && !candidate_urls.contains(&url.to_string())
                            {
                                candidate_urls.push(url.to_string());
                            }
                        }
                    }
                }

                if candidate_urls.is_empty() {
                    continue;
                }

                let identifier = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&candidate_urls[0])
                    .to_string();

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
                    description: String::new(),
                    identifier,
                    work_dir: Default::default(),
                    ext: None,
                    save_name: None,
                    save_path: None,
                    extra: item.clone(),
                });
            }
        }

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = WallhavenImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("nature", &params, &Filters::new());
        // 50 * 1.2 / 24 = 2.5 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=nature"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("sorting=relevance"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = WallhavenImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": "abc123",
                    "path": "https://w.wallhaven.cc/full/abc/wallhaven-abc123.jpg",
                    "thumbs": {
                        "original": "https://w.wallhaven.cc/full/abc/thumb-abc123.jpg",
                        "large": "https://w.wallhaven.cc/large/abc/thumb-abc123.jpg",
                        "small": "https://w.wallhaven.cc/small/abc/thumb-abc123.jpg"
                    }
                },
                {
                    "id": "def456",
                    "path": "https://w.wallhaven.cc/full/def/wallhaven-def456.png",
                    "thumbs": {
                        "large": "https://w.wallhaven.cc/large/def/thumb-def456.jpg"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "abc123");
        assert_eq!(results[0].candidate_download_urls[0], "https://w.wallhaven.cc/full/abc/wallhaven-abc123.jpg");
        // Thumbs should follow the full path
        assert_eq!(results[0].candidate_download_urls.len(), 4);
        assert_eq!(results[1].identifier, "def456");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = WallhavenImageSource::new();
        let json = r#"{"data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
