//! Freeimages image search source.
//!
//! Replaces Python's `FreeImagesImageClient`. Uses the iStockphoto API
//! with JSON response parsing to extract image URLs.

use reqwest::header::{HeaderMap, ACCEPT, REFERER, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Freeimages API request.
const PAGE_SIZE: usize = 60;

/// Freeimages image search source.
pub struct FreeimagesImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl FreeimagesImageSource {
    /// Create a new Freeimages image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            ACCEPT,
            "application/json".parse().unwrap(),
        );
        search_headers.insert(
            REFERER,
            "https://www.istockphoto.com/search/2/image-film"
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
        download_headers.insert(
            REFERER,
            "https://www.istockphoto.com/".parse().unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for FreeimagesImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for FreeimagesImageSource {
    fn source_name(&self) -> &str {
        "freeimages"
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
                    "https://www.istockphoto.com/search/2/image-film?phrase={}&page={}",
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

        // Navigate to gallery.assets
        let assets = search_result
            .get("gallery")
            .and_then(|v| v.get("assets"))
            .and_then(|v| v.as_array());

        if let Some(items) = assets {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Only process image assets
                let asset_type = item.get("assetType").and_then(|v| v.as_str());
                if asset_type != Some("image") {
                    continue;
                }

                // Extract thumbnail URL
                let thumb_url = match item.get("thumbUrl").and_then(|v| v.as_str()) {
                    Some(url) if url.starts_with("http") => url.to_string(),
                    _ => continue,
                };

                // Use assetId or id as identifier, fallback to thumbUrl
                let identifier = item
                    .get("assetId")
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("id").and_then(|v| v.as_str()))
                    .unwrap_or(&thumb_url)
                    .to_string();

                image_infos.push(ImageInfo::with_identifier(
                    self.source_name(),
                    vec![thumb_url],
                    identifier,
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
        let source = FreeimagesImageSource::new();
        let params = SearchParams {
            search_limits: 120,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 120 * 1.2 / 60 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("phrase=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = FreeimagesImageSource::new();
        let json = r#"{
            "gallery": {
                "assets": [
                    {
                        "assetType": "image",
                        "assetId": "img123",
                        "thumbUrl": "https://media.istockphoto.com/thumb123.jpg"
                    },
                    {
                        "assetType": "image",
                        "id": "img456",
                        "thumbUrl": "https://media.istockphoto.com/thumb456.jpg"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "img123");
        assert_eq!(results[0].candidate_download_urls[0], "https://media.istockphoto.com/thumb123.jpg");
        assert_eq!(results[1].identifier, "img456");
    }

    #[test]
    fn test_parse_skips_non_image_assets() {
        let source = FreeimagesImageSource::new();
        let json = r#"{
            "gallery": {
                "assets": [
                    {
                        "assetType": "video",
                        "assetId": "vid123",
                        "thumbUrl": "https://media.istockphoto.com/thumb_vid.jpg"
                    },
                    {
                        "assetType": "image",
                        "assetId": "img123",
                        "thumbUrl": "https://media.istockphoto.com/thumb123.jpg"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "img123");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = FreeimagesImageSource::new();
        let json = r#"{"gallery": {"assets": []}}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
