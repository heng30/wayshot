//! Flickr image search source.
//!
//! Replaces Python's `FlickrImageClient`. Uses the Flickr REST API
//! with JSON response parsing. Multiple API keys are rotated across
//! pages to distribute load.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Flickr API keys for rotation across pages.
const CANDIDATE_API_KEYS: &[&str] = &[
    "0f15ff623f1198a1f7f52550f8c36057",
    "a6365f14201cd3c5f34678e671b9ab8d",
    "f7e7fb8cc34e52db3e5af5e1727d0c0b",
    "ca4dd89d3dfaeaf075144c3fdec76756",
    "9b4439ce94de7e2ec2c2e6ffadc22bcf",
    "6c2dba48efdbccaced44ea0b445fecbf",
    "57bded31ef9c635326e4acfa2c62b7dc",
    "929033444e3a0d9a3859195d56d36552",
];

/// Number of results per Flickr API request.
const PAGE_SIZE: usize = 50;

/// Flickr image search source.
pub struct FlickrImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl FlickrImageSource {
    /// Create a new Flickr image source.
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

impl Default for FlickrImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for FlickrImageSource {
    fn source_name(&self) -> &str {
        "flickr"
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
                let api_key = CANDIDATE_API_KEYS[pn % CANDIDATE_API_KEYS.len()];
                let url = format!(
                    "https://www.flickr.com/services/rest/?method=flickr.photos.search&api_key={}&text={}&format=json&nojsoncallback=1&extras=url_l&per_page={}&page={}&sort=relevance&safe_search=0",
                    api_key,
                    urlencoding::encode(keyword),
                    PAGE_SIZE,
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

        let photos = search_result
            .get("photos")
            .and_then(|v| v.get("photo"))
            .and_then(|v| v.as_array());

        if let Some(items) = photos {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let url_l = item.get("url_l").and_then(|v| v.as_str());
                let url = match url_l {
                    Some(u) if u.starts_with("http") => u.to_string(),
                    _ => continue,
                };

                let identifier = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&url)
                    .to_string();

                image_infos.push(ImageInfo::with_identifier(
                    self.source_name(),
                    vec![url],
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
        let source = FlickrImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("method=flickr.photos.search"));
        assert!(urls[0].url.contains("text=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
        // API key rotation: page 1 uses key 0, page 2 uses key 1
        assert!(urls[0].url.contains(CANDIDATE_API_KEYS[0]));
        assert!(urls[1].url.contains(CANDIDATE_API_KEYS[1]));
    }

    #[test]
    fn test_parse_search_result() {
        let source = FlickrImageSource::new();
        let json = r#"{
            "photos": {
                "photo": [
                    {
                        "id": "12345",
                        "url_l": "https://live.staticflickr.com/photo1.jpg"
                    },
                    {
                        "id": "67890",
                        "url_l": "https://live.staticflickr.com/photo2.jpg"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "12345");
        assert_eq!(results[0].candidate_download_urls[0], "https://live.staticflickr.com/photo1.jpg");
        assert_eq!(results[1].identifier, "67890");
    }

    #[test]
    fn test_parse_skips_items_without_url_l() {
        let source = FlickrImageSource::new();
        let json = r#"{
            "photos": {
                "photo": [
                    {
                        "id": "12345",
                        "url_l": "https://live.staticflickr.com/photo1.jpg"
                    },
                    {
                        "id": "67890"
                    },
                    {
                        "id": "11111",
                        "url_l": "not-a-url"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "12345");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = FlickrImageSource::new();
        let json = r#"{"photos": {"photo": []}}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
