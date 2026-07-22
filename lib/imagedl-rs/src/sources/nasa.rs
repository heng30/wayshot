//! NASA Image and Video Library search source.
//!
//! Replaces Python's `NASAImageClient`. Uses the NASA Images API
//! at `https://images-api.nasa.gov/search` with JSON response parsing.
//!
//! NASA API returns results under `collection.items[]`, where each item has
//! `links[]` (with preview/image hrefs) and `data[]` (with metadata like
//! title and description). Thumbnail URLs are upgraded to larger sizes by
//! replacing `~thumb.jpg` with `~orig.jpg`, `~large.jpg`, or `~medium.jpg`.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// NASA image search source.
pub struct NasaImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

/// NASA API page size.
const PAGE_SIZE: usize = 100;

impl NasaImageSource {
    /// Create a new NASA image source.
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

impl Default for NasaImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for NasaImageSource {
    fn source_name(&self) -> &str {
        "nasa"
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
        let base_url = "https://images-api.nasa.gov/search?";
        let num_pages =
            ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url_params = vec![
                    format!("q={}", urlencoding::encode(keyword)),
                    "media_type=image".to_string(),
                    format!("page={}", pn + 1),
                ];

                // Add extra filter params
                for (key, value) in filters {
                    if let Some(s) = value.as_str() {
                        url_params.push(format!(
                            "{}={}",
                            urlencoding::encode(key),
                            urlencoding::encode(s)
                        ));
                    }
                }

                SearchUrl::new(format!("{}{}", base_url, url_params.join("&")))
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

        let items = search_result
            .get("collection")
            .and_then(|c| c.get("items"))
            .and_then(|v| v.as_array());

        if let Some(items) = items {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Get the first link href (typically a thumbnail)
                let href = item
                    .get("links")
                    .and_then(|v| v.as_array())
                    .and_then(|links| links.first())
                    .and_then(|link| link.get("href"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if !href.starts_with("http") {
                    continue;
                }

                // Build candidate URLs by upgrading thumbnail to larger sizes
                let orig_url = href.replace("~thumb.jpg", "~orig.jpg");
                let large_url = href.replace("~thumb.jpg", "~large.jpg");
                let medium_url = href.replace("~thumb.jpg", "~medium.jpg");

                let candidate_urls: Vec<String> = [orig_url, large_url, medium_url, href.to_string()]
                    .into_iter()
                    .filter(|u| u.starts_with("http"))
                    .collect();

                if candidate_urls.is_empty() {
                    continue;
                }

                // Get description from data[0]
                let data = item
                    .get("data")
                    .and_then(|v| v.as_array())
                    .and_then(|d| d.first());

                let description = data
                    .and_then(|d| d.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Get NASA ID from data[0]
                let nasa_id = data
                    .and_then(|d| d.get("nasa_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| candidate_urls[0].clone());

                // Get title from data[0]
                let title = data
                    .and_then(|d| d.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Combine title and description
                let full_description = if title.is_empty() {
                    description.to_string()
                } else if description.is_empty() {
                    title.to_string()
                } else {
                    format!("{}: {}", title, description)
                };

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
                    description: full_description,
                    identifier: nasa_id,
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
        let source = NasaImageSource::new();
        let params = SearchParams {
            search_limits: 200,
            ..Default::default()
        };
        let urls = source.construct_search_urls("mars", &params, &Filters::new());
        // 200 * 1.2 / 100 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("images-api.nasa.gov/search"));
        assert!(urls[0].url.contains("q=mars"));
        assert!(urls[0].url.contains("media_type=image"));
        assert!(urls[0].url.contains("page=1"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = NasaImageSource::new();
        let json = r#"{
            "collection": {
                "items": [
                    {
                        "data": [
                            {
                                "nasa_id": "PIA12345",
                                "title": "Mars Surface",
                                "description": "A photo of Mars surface"
                            }
                        ],
                        "links": [
                            {
                                "href": "https://images.nasa.gov/images/PIA12345~thumb.jpg",
                                "rel": "preview"
                            }
                        ]
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "PIA12345");
        assert!(results[0].description.contains("Mars Surface"));
        // Should have orig, large, medium, and thumb URLs
        assert_eq!(results[0].candidate_download_urls.len(), 4);
        assert!(results[0].candidate_download_urls[0].contains("~orig.jpg"));
        assert!(results[0].candidate_download_urls[1].contains("~large.jpg"));
        assert!(results[0].candidate_download_urls[2].contains("~medium.jpg"));
        assert!(results[0].candidate_download_urls[3].contains("~thumb.jpg"));
    }

    #[test]
    fn test_parse_empty_results() {
        let source = NasaImageSource::new();
        let json = r#"{"collection": {"items": []}}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_collection() {
        let source = NasaImageSource::new();
        let json = r#"{}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_no_links() {
        let source = NasaImageSource::new();
        let json = r#"{
            "collection": {
                "items": [
                    {
                        "data": [{"nasa_id": "PIA999", "title": "No links"}]
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
