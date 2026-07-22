//! Stocksnap image search source.
//!
//! Replaces Python's `StockSnapImageClient`. Uses the Stocksnap API
//! with JSON response parsing. Also extracts image IDs from the
//! `results` array to construct CDN URLs.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Stocksnap API request.
const PAGE_SIZE: usize = 40;

/// Stocksnap image search source.
pub struct StocksnapImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl StocksnapImageSource {
    /// Create a new Stocksnap image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for StocksnapImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for StocksnapImageSource {
    fn source_name(&self) -> &str {
        "stocksnap"
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
        _filters: &Filters,
    ) -> Vec<SearchUrl> {
        let num_pages = ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let url = format!(
                    "https://stocksnap.io/api/search-photos/{}/relevance/desc/{}",
                    urlencoding::encode(keyword),
                    pn + 1,
                );
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

        // Parse images array with preview_urls
        if let Some(images) = search_result.get("images").and_then(|v| v.as_array()) {
            for item in images {
                if !item.is_object() {
                    continue;
                }

                let preview_urls = item.get("preview_urls").and_then(|v| v.as_object());
                let mut candidate_urls = Vec::new();

                if let Some(url_map) = preview_urls {
                    for key in &["large", "medium", "small"] {
                        if let Some(url) = url_map.get(*key).and_then(|v| v.as_str()) {
                            if url.starts_with("http") {
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

                let description = item
                    .get("caption")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let mut info = ImageInfo::with_identifier(
                    self.source_name(),
                    candidate_urls,
                    identifier,
                );
                info.description = description;
                image_infos.push(info);
            }
        }

        // Parse results array with img_id -> CDN URL
        if let Some(results) = search_result.get("results").and_then(|v| v.as_array()) {
            for item in results {
                if !item.is_object() {
                    continue;
                }

                let img_id = match item.get("img_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => continue,
                };

                let cdn_url = format!("https://cdn.stocksnap.io/img-thumbs/280h/{}.jpg", img_id);
                image_infos.push(ImageInfo::with_identifier(
                    self.source_name(),
                    vec![cdn_url],
                    img_id,
                ));
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
        let source = StocksnapImageSource::new();
        let params = SearchParams {
            search_limits: 80,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 80 * 1.2 / 40 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("search-photos"));
        assert!(urls[0].url.contains("cats"));
        assert!(urls[0].url.contains("relevance/desc/1"));
        assert!(urls[1].url.contains("relevance/desc/2"));
    }

    #[test]
    fn test_parse_search_result_images() {
        let source = StocksnapImageSource::new();
        let json = r#"{
            "images": [
                {
                    "id": "abc123",
                    "caption": "A cute cat",
                    "preview_urls": {
                        "large": "https://cdn.stocksnap.io/large/abc123.jpg",
                        "medium": "https://cdn.stocksnap.io/medium/abc123.jpg",
                        "small": "https://cdn.stocksnap.io/small/abc123.jpg"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "abc123");
        assert_eq!(results[0].description, "A cute cat");
        assert_eq!(results[0].candidate_download_urls.len(), 3);
    }

    #[test]
    fn test_parse_search_result_results() {
        let source = StocksnapImageSource::new();
        let json = r#"{
            "results": [
                {"img_id": "XYZ789"},
                {"img_id": "DEF456"}
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "XYZ789");
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://cdn.stocksnap.io/img-thumbs/280h/XYZ789.jpg"
        );
    }

    #[test]
    fn test_parse_empty_results() {
        let source = StocksnapImageSource::new();
        let json = r#"{"images": [], "results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
