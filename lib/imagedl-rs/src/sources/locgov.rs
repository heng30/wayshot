//! Library of Congress image search source.
//!
//! Replaces Python's `LocGovImageClient`. Uses the Library of Congress
//! search API with JSON response parsing.
//!
//! Each result contains an `image_url` array with multiple resolution URLs,
//! which are used as candidate download URLs (highest quality first).

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for LoC API requests.
const DEFAULT_PAGE_SIZE: usize = 50;

/// Library of Congress image search source.
pub struct LocGovImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl LocGovImageSource {
    /// Create a new Library of Congress image source.
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

impl Default for LocGovImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for LocGovImageSource {
    fn source_name(&self) -> &str {
        "locgov"
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
        let base_url = "https://www.loc.gov/search?";
        let page_size = filters
            .get("c")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}q={}&fo=json&fa=online-format:image&c={}&sp={}",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    pn + 1,
                );
                for (key, value) in filters {
                    if key != "c" && key != "q" && key != "fo" && key != "fa" && key != "sp"
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

                // The "image_url" field is an array of URLs at different resolutions.
                // Reverse the array so highest quality URLs come first.
                let image_urls = item
                    .get("image_url")
                    .and_then(|v| v.as_array());

                let candidate_urls: Vec<String> = if let Some(urls) = image_urls {
                    urls.iter()
                        .rev()
                        .filter_map(|v| v.as_str())
                        .filter(|s| s.starts_with("http"))
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                if candidate_urls.is_empty() {
                    continue;
                }

                // Use the first (highest quality) URL as the identifier
                let identifier = candidate_urls[0].clone();

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
        let source = LocGovImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("civil war", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=civil%20war"));
        assert!(urls[0].url.contains("fo=json"));
        assert!(urls[0].url.contains("sp=1"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = LocGovImageSource::new();
        let json = r#"{
            "results": [
                {
                    "title": "Civil War Photograph",
                    "image_url": [
                        "https://www.loc.gov/image/small/img1.jpg",
                        "https://www.loc.gov/image/medium/img1.jpg",
                        "https://www.loc.gov/image/large/img1.jpg"
                    ]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // Reversed: large first, then medium, then small
        assert_eq!(results[0].candidate_download_urls[0], "https://www.loc.gov/image/large/img1.jpg");
        assert_eq!(results[0].candidate_download_urls[2], "https://www.loc.gov/image/small/img1.jpg");
        assert_eq!(results[0].description, "Civil War Photograph");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = LocGovImageSource::new();
        let json = r#"{"results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_image_url_skipped() {
        let source = LocGovImageSource::new();
        let json = r#"{
            "results": [
                {
                    "title": "No image URL"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
