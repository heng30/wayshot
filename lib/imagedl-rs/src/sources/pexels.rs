//! Pexels image search source.
//!
//! Replaces Python's `PexelsImageClient`. Uses Pexels's REST API
//! with JSON response parsing.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Pexels API requests.
const DEFAULT_PAGE_SIZE: usize = 24;

/// Pexels API secret key (same as Python implementation).
const PEXELS_SECRET_KEY: &str = "H2jk9uKnhRmL6WPwh89zBezWvr";

/// Pexels image search source.
pub struct PexelsImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl PexelsImageSource {
    /// Create a new Pexels image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            "secret-key",
            PEXELS_SECRET_KEY.parse().unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        download_headers.insert(
            "secret-key",
            PEXELS_SECRET_KEY.parse().unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for PexelsImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for PexelsImageSource {
    fn source_name(&self) -> &str {
        "pexels"
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
        let base_url = "https://www.pexels.com/en-us/api/v3/search/photos?";
        let page_size = filters
            .get("per_page")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}query={}&page={}&per_page={}&seo_tags=true",
                    base_url,
                    urlencoding::encode(keyword),
                    pn + 1,
                    page_size,
                );
                for (key, value) in filters {
                    if key != "per_page" && key != "query" && key != "page"
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

                // Navigate to attributes.image
                let image_attr = item
                    .get("attributes")
                    .and_then(|a| a.get("image"))
                    .and_then(|v| v.as_object());

                let candidate_urls: Vec<String> = if let Some(img) = image_attr {
                    let keys = [
                        "download_link",
                        "large",
                        "medium",
                        "small",
                    ];
                    keys.iter()
                        .filter_map(|k| img.get(*k).and_then(|v| v.as_str()))
                        .filter(|s| s.starts_with("http"))
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                if candidate_urls.is_empty() {
                    continue;
                }

                let identifier = item
                    .get("id")
                    .and_then(|v| {
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
                        .get("attributes")
                        .and_then(|a| a.get("alt"))
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
        let source = PexelsImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 50 * 1.2 / 24 = 2.5 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("query=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("per_page=24"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = PexelsImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 12345,
                    "attributes": {
                        "alt": "A cute cat",
                        "image": {
                            "download_link": "https://example.com/cat1_dl.jpg",
                            "large": "https://example.com/cat1_large.jpg",
                            "medium": "https://example.com/cat1_medium.jpg",
                            "small": "https://example.com/cat1_small.jpg"
                        }
                    }
                },
                {
                    "id": 67890,
                    "attributes": {
                        "alt": "Another cat",
                        "image": {
                            "large": "https://example.com/cat2_large.jpg"
                        }
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "12345");
        assert_eq!(results[0].description, "A cute cat");
        assert_eq!(results[0].candidate_download_urls.len(), 4);
        assert_eq!(results[1].identifier, "67890");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = PexelsImageSource::new();
        let json = r#"{"data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_image_attribute_skipped() {
        let source = PexelsImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 99999,
                    "attributes": {
                        "alt": "No image data"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
