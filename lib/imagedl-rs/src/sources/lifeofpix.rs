//! Life of Pix image search source.
//!
//! Replaces Python's `LifeOfPixImageClient`. Uses the Life of Pix REST API
//! with JSON response parsing.
//!
//! The API endpoint format is:
//! `https://www.lifeofpix.com/api/search/photos/{keyword}/{page_size}.json?page={pn}`

use reqwest::header::{HeaderMap, ACCEPT, ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Life of Pix API requests.
const DEFAULT_PAGE_SIZE: usize = 40;

/// Life of Pix image search source.
pub struct LifeOfPixImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl LifeOfPixImageSource {
    /// Create a new Life of Pix image source.
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
            "application/json, text/plain, */*".parse().unwrap(),
        );
        search_headers.insert(
            ACCEPT_LANGUAGE,
            "en-US,en;q=0.9".parse().unwrap(),
        );
        search_headers.insert(
            REFERER,
            "https://www.lifeofpix.com/".parse().unwrap(),
        );
        search_headers.insert(
            "x-requested-with",
            "XMLHttpRequest".parse().unwrap(),
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

impl Default for LifeOfPixImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for LifeOfPixImageSource {
    fn source_name(&self) -> &str {
        "lifeofpix"
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
        let page_size = DEFAULT_PAGE_SIZE;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        // Convert keyword: replace whitespace with hyphens, then URL-encode
        let slug = keyword
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join("-");

        (0..num_pages)
            .map(|pn| {
                SearchUrl::new(format!(
                    "https://www.lifeofpix.com/api/search/photos/{}/{}.json?page={}",
                    urlencoding::encode(&slug),
                    page_size,
                    pn + 1,
                ))
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

        // The API can return data as an array at the top level or in "data"
        let data = search_result
            .get("data")
            .and_then(|v| v.as_array())
            .or_else(|| search_result.as_array());

        if let Some(items) = data {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Extract candidate URLs in priority order
                let keys = [
                    "urlDownload",
                    "url",
                    "istockRelatedThumbnailLargeUrl",
                    "istockRelatedThumbnailSmallUrl",
                    "istockRelatedThumbnailPreviewUrl",
                    "thumbnail",
                    "istockRelatedThumbnailMosaicUrl",
                ];

                let candidate_urls: Vec<String> = keys
                    .iter()
                    .filter_map(|k| item.get(*k).and_then(|v| v.as_str()))
                    .filter(|s| s.starts_with("http"))
                    .map(|s| s.to_string())
                    .collect();

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
        let source = LifeOfPixImageSource::new();
        let params = SearchParams {
            search_limits: 80,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cute dogs", &params, &Filters::new());
        // 80 * 1.2 / 40 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("cute-dogs"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("40.json"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = LifeOfPixImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 42,
                    "urlDownload": "https://lifeofpix.com/download/img1.jpg",
                    "url": "https://lifeofpix.com/img/img1.jpg",
                    "thumbnail": "https://lifeofpix.com/thumb/img1.jpg"
                },
                {
                    "id": 99,
                    "url": "https://lifeofpix.com/img/img2.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "42");
        assert_eq!(results[0].candidate_download_urls[0], "https://lifeofpix.com/download/img1.jpg");
        assert_eq!(results[1].identifier, "99");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = LifeOfPixImageSource::new();
        let json = r#"{"data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_urls_skipped() {
        let source = LifeOfPixImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 100
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
