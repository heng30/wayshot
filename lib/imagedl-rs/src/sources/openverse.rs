//! Openverse image search source.
//!
//! Replaces Python's `OpenverseImageClient`. Uses the Openverse REST API
//! with JSON response parsing.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Openverse API requests.
const DEFAULT_PAGE_SIZE: usize = 20;

/// Openverse image search source.
pub struct OpenverseImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl OpenverseImageSource {
    /// Create a new Openverse image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for OpenverseImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for OpenverseImageSource {
    fn source_name(&self) -> &str {
        "openverse"
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
        let base_url = "https://api.openverse.org/v1/images/?";
        let page_size = filters
            .get("page_size")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}q={}&page={}&page_size={}",
                    base_url,
                    urlencoding::encode(keyword),
                    pn + 1,
                    page_size,
                );
                for (key, value) in filters {
                    if key != "page_size" && key != "q" && key != "page"
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

        let results = search_result.get("results").and_then(|v| v.as_array());
        if let Some(items) = results {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Extract candidate URLs: prefer "url", fallback to "thumbnail"
                let mut candidate_urls = Vec::new();
                if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
                    if url.starts_with("http") {
                        candidate_urls.push(url.to_string());
                    }
                }
                if let Some(url) = item.get("thumbnail").and_then(|v| v.as_str()) {
                    if url.starts_with("http") && !candidate_urls.contains(&url.to_string()) {
                        candidate_urls.push(url.to_string());
                    }
                }

                if candidate_urls.is_empty() {
                    continue;
                }

                let identifier = item
                    .get("id")
                    .and_then(|v| {
                        // "id" can be string or number
                        v.as_str()
                            .map(|s| s.to_string())
                            .or_else(|| v.as_i64().map(|n| n.to_string()))
                    })
                    .unwrap_or_else(|| candidate_urls[0].clone());

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
                    description: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
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
        let source = OpenverseImageSource::new();
        let params = SearchParams {
            search_limits: 40,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 40 * 1.2 / 20 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("page_size=20"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = OpenverseImageSource::new();
        let json = r#"{
            "results": [
                {
                    "id": "abc123",
                    "title": "A cute cat",
                    "url": "https://example.com/cat1.jpg",
                    "thumbnail": "https://example.com/thumb1.jpg"
                },
                {
                    "id": "def456",
                    "url": "https://example.com/cat2.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "abc123");
        assert_eq!(results[0].description, "A cute cat");
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        assert_eq!(results[1].identifier, "def456");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = OpenverseImageSource::new();
        let json = r#"{"results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_numeric_id() {
        let source = OpenverseImageSource::new();
        let json = r#"{
            "results": [
                {
                    "id": 42,
                    "url": "https://example.com/cat.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "42");
    }
}
