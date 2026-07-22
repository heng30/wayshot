//! Konachan image board source.
//!
//! Replaces Python's `KonachanImageClient`. Uses the Konachan JSON API
//! to search for images with pagination. Only URLs starting with "http"
//! are accepted as valid candidate URLs.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per page for the Konachan API.
const KONACHAN_PAGE_SIZE: usize = 50;

/// Konachan image board source.
///
/// Uses the Konachan API at `https://konachan.net/post.json` with
/// paginated requests. The `rating:safe` tag is automatically appended
/// to the search keyword. Only URLs starting with `"http"` are accepted.
pub struct KonachanImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl KonachanImageSource {
    /// Create a new Konachan image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            "accept",
            "application/json,text/plain,*/*".parse().unwrap(),
        );
        search_headers.insert(
            "referer",
            "https://konachan.net/".parse().unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        download_headers.insert(
            "referer",
            "https://konachan.net/".parse().unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for KonachanImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for KonachanImageSource {
    fn source_name(&self) -> &str {
        "konachan"
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
        let base_url = "https://konachan.net/post.json?";
        let num_pages =
            ((params.search_limits as f64 * 1.2 / KONACHAN_PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let tags = format!("{} rating:safe", keyword);
                let mut url_params = vec![
                    format!("tags={}", urlencoding::encode(&tags)),
                    format!("page={}", pn + 1),
                    format!("limit={}", KONACHAN_PAGE_SIZE),
                ];
                for (key, value) in filters {
                    if key != "tags" && key != "page" && key != "limit" {
                        if let Some(s) = value.as_str() {
                            url_params.push(format!(
                                "{}={}",
                                urlencoding::encode(key),
                                urlencoding::encode(s)
                            ));
                        }
                    }
                }
                SearchUrl::new(format!("{}{}", base_url, url_params.join("&")))
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let search_result: Vec<Value> = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        let mut image_infos = Vec::new();
        for item in &search_result {
            if !item.is_object() {
                continue;
            }

            let file_url = item
                .get("file_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());
            let sample_url = item
                .get("sample_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());
            let jpeg_url = item
                .get("jpeg_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());
            let preview_url = item
                .get("preview_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());

            let candidate_urls: Vec<String> =
                [file_url, sample_url, jpeg_url, preview_url]
                    .into_iter()
                    .flatten()
                    .collect();

            if candidate_urls.is_empty() {
                continue;
            }

            let identifier = item
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|id| id.to_string())
                .or_else(|| {
                    item.get("file_url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
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

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = KonachanImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("konachan.net/post.json"));
        assert!(urls[0].url.contains("tags="));
        assert!(urls[0].url.contains("rating%3Asafe"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
        assert!(urls[0].url.contains("limit=50"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = KonachanImageSource::new();
        let json = r#"[
            {
                "id": 111,
                "file_url": "https://konachan.net/image/111.jpg",
                "sample_url": "https://konachan.net/sample/111.jpg",
                "jpeg_url": "https://konachan.net/jpeg/111.jpg",
                "preview_url": "https://konachan.net/preview/111.jpg"
            },
            {
                "id": 222,
                "file_url": "https://konachan.net/image/222.png",
                "preview_url": "https://konachan.net/preview/222.png"
            }
        ]"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "111");
        assert_eq!(results[0].candidate_download_urls.len(), 4);
        assert_eq!(results[1].identifier, "222");
        assert_eq!(results[1].candidate_download_urls.len(), 2);
    }

    #[test]
    fn test_parse_empty_results() {
        let source = KonachanImageSource::new();
        let json = r#"[]"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_filters_non_http_urls() {
        let source = KonachanImageSource::new();
        let json = r#"[
            {
                "id": 333,
                "file_url": "//konachan.net/image/333.jpg",
                "preview_url": "https://konachan.net/preview/333.jpg"
            }
        ]"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // Only the https URL should be included
        assert_eq!(results[0].candidate_download_urls.len(), 1);
        assert!(results[0].candidate_download_urls[0].starts_with("https://"));
    }
}
