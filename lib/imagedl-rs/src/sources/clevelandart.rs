//! Cleveland Museum of Art image search source.
//!
//! Replaces Python's `ClevelandArtImageClient`. Uses the Cleveland Museum of
//! Art's open access API with JSON response parsing.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Cleveland Museum of Art image search source.
pub struct ClevelandArtImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl ClevelandArtImageSource {
    /// Create a new Cleveland Art image source.
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

        let mut search_headers = HeaderMap::new();
        search_headers.insert(USER_AGENT, ua.parse().unwrap());

        let mut download_headers = HeaderMap::new();
        download_headers.insert(USER_AGENT, ua.parse().unwrap());

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for ClevelandArtImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for ClevelandArtImageSource {
    fn source_name(&self) -> &str {
        "clevelandart"
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
        let base_url = "https://openaccess-api.clevelandart.org/api/artworks/?";
        let page_size = filters
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(50) as usize;
        let page_size = page_size.clamp(1, 100);

        let num_pages = (params.search_limits as f64 * 1.2 / page_size as f64).ceil() as usize;

        (0..num_pages)
            .map(|pn| {
                let skip = pn * page_size;
                let mut url = format!(
                    "{}q={}&has_image=1&limit={}&skip={}",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    skip,
                );
                // Add extra filter params
                for (key, value) in filters {
                    if key != "q" && key != "has_image" && key != "limit" && key != "skip" {
                        if let Some(s) = value.as_str() {
                            url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
                        }
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

                let images = item.get("images").and_then(|v| v.as_object());
                let images = match images {
                    Some(img) => img,
                    None => continue,
                };

                // Build candidate URLs in priority order: full, print, web
                let mut candidate_urls = Vec::new();

                if let Some(url) = images
                    .get("full")
                    .and_then(|f| f.get("url"))
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                {
                    candidate_urls.push(url.to_string());
                }

                if let Some(url) = images
                    .get("print")
                    .and_then(|f| f.get("url"))
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                {
                    candidate_urls.push(url.to_string());
                }

                if let Some(url) = images
                    .get("web")
                    .and_then(|f| f.get("url"))
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                {
                    candidate_urls.push(url.to_string());
                }

                if candidate_urls.is_empty() {
                    continue;
                }

                // Use the item id as identifier, fallback to first URL
                let identifier = item
                    .get("id")
                    .and_then(|v| {
                        // id could be a number or string
                        v.as_str()
                            .map(|s| s.to_string())
                            .or_else(|| v.as_i64().map(|n| n.to_string()))
                    })
                    .or_else(|| item.get("accession_number").and_then(|v| v.as_str()).map(|s| s.to_string()))
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
        let source = ClevelandArtImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("painting", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=painting"));
        assert!(urls[0].url.contains("has_image=1"));
        assert!(urls[0].url.contains("limit=50"));
        assert!(urls[0].url.contains("skip=0"));
        assert!(urls[1].url.contains("skip=50"));
        assert!(urls[2].url.contains("skip=100"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = ClevelandArtImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 123,
                    "accession_number": "1916.123",
                    "title": "Water Lilies",
                    "images": {
                        "full": {"url": "https://example.com/full.jpg"},
                        "print": {"url": "https://example.com/print.jpg"},
                        "web": {"url": "https://example.com/web.jpg"}
                    }
                },
                {
                    "id": 456,
                    "accession_number": "1920.456",
                    "title": "Sunflowers",
                    "images": {
                        "web": {"url": "https://example.com/sun-web.jpg"}
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "123");
        assert_eq!(results[0].description, "Water Lilies");
        assert_eq!(results[0].candidate_download_urls.len(), 3);
        // Full quality first
        assert_eq!(results[0].candidate_download_urls[0], "https://example.com/full.jpg");
        assert_eq!(results[1].candidate_download_urls.len(), 1);
        assert_eq!(results[1].candidate_download_urls[0], "https://example.com/sun-web.jpg");
    }

    #[test]
    fn test_parse_skips_items_without_images() {
        let source = ClevelandArtImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 789,
                    "title": "No Image Item",
                    "images": null
                },
                {
                    "id": 999,
                    "title": "Broken Images",
                    "images": {}
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_skips_non_http_urls() {
        let source = ClevelandArtImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 111,
                    "title": "Bad URL",
                    "images": {
                        "web": {"url": "ftp://not-http.com/file.jpg"}
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_empty_results() {
        let source = ClevelandArtImageSource::new();
        let json = r#"{"data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
