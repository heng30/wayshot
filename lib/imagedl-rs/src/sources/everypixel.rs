//! Everypixel image search source.
//!
//! Replaces Python's `EverypixelImageClient`. Uses Everypixel's REST API
//! with JSON response parsing.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Everypixel API requests.
const DEFAULT_PAGE_SIZE: usize = 50;

/// Everypixel image search source.
pub struct EverypixelImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl EverypixelImageSource {
    /// Create a new Everypixel image source.
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

impl Default for EverypixelImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for EverypixelImageSource {
    fn source_name(&self) -> &str {
        "everypixel"
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
        let base_url = "https://www.everypixel.com/search/search?";
        let page_size = filters
            .get("per_page")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}q={}&is_id=64&limit={}&json=1&page={}",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    pn + 1,
                );
                for (key, value) in filters {
                    if key != "per_page" && key != "q" && key != "page" && key != "limit"
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

        // The "images" field is a dict of lists; flatten all values
        let images = search_result.get("images").and_then(|v| v.as_object());
        if let Some(image_map) = images {
            for (_key, value) in image_map {
                let items = if value.is_array() {
                    value.as_array().unwrap()
                } else {
                    continue;
                };
                for item in items {
                    if !item.is_object() {
                        continue;
                    }

                    // Extract candidate URLs: prefer "url", fallback to "thumb_url"
                    let mut candidate_urls = Vec::new();
                    if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
                        if url.starts_with("http") {
                            candidate_urls.push(url.to_string());
                        }
                    }
                    if let Some(url) = item.get("thumb_url").and_then(|v| v.as_str()) {
                        if url.starts_with("http") && !candidate_urls.contains(&url.to_string()) {
                            candidate_urls.push(url.to_string());
                        }
                    }

                    if candidate_urls.is_empty() {
                        continue;
                    }

                    let identifier = item
                        .get("basic_img_id")
                        .or_else(|| item.get("id"))
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
        }

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = EverypixelImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("limit=50"));
        assert!(urls[1].url.contains("page=2"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = EverypixelImageSource::new();
        let json = r#"{
            "images": {
                "photos": [
                    {
                        "basic_img_id": "ep123",
                        "url": "https://example.com/photo1.jpg",
                        "thumb_url": "https://example.com/thumb1.jpg"
                    },
                    {
                        "id": "ep456",
                        "url": "https://example.com/photo2.jpg"
                    }
                ],
                "vectors": [
                    {
                        "basic_img_id": "ep789",
                        "url": "https://example.com/vector1.svg"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].identifier, "ep123");
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        assert_eq!(results[1].identifier, "ep456");
        assert_eq!(results[2].identifier, "ep789");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = EverypixelImageSource::new();
        let json = r#"{"images": {}}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_url_skipped() {
        let source = EverypixelImageSource::new();
        let json = r#"{
            "images": {
                "photos": [
                    {
                        "basic_img_id": "ep_no_url"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
