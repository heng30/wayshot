//! Yande.re image board source.
//!
//! Replaces Python's `YandeImageClient`. Uses the Yande.re JSON API
//! to search for images with pagination. Only URLs starting with "http"
//! are accepted as valid candidate URLs.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per page for the Yande.re API.
const YANDE_PAGE_SIZE: usize = 50;

/// Yande.re image board source.
///
/// Uses the Yande.re API at `https://yande.re/post.json` with
/// paginated requests. Only URLs starting with `"http"` are accepted.
/// The identifier is the `file_url` from the API response.
pub struct YandeImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl YandeImageSource {
    /// Create a new Yande.re image source.
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

impl Default for YandeImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for YandeImageSource {
    fn source_name(&self) -> &str {
        "yande"
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
        let base_url = "https://yande.re/post.json?";
        let num_pages =
            ((params.search_limits as f64 * 1.2 / YANDE_PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url_params = vec![
                    format!("tags={}", urlencoding::encode(keyword)),
                    format!("page={}", pn + 1),
                    format!("limit={}", YANDE_PAGE_SIZE),
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
            let jpeg_url = item
                .get("jpeg_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());
            let sample_url = item
                .get("sample_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());
            let preview_url = item
                .get("preview_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
                .map(|s| s.to_string());

            let candidate_urls: Vec<String> =
                [file_url, jpeg_url, sample_url, preview_url]
                    .into_iter()
                    .flatten()
                    .collect();

            if candidate_urls.is_empty() {
                continue;
            }

            // Identifier is file_url per the Python implementation
            let identifier = item
                .get("file_url")
                .and_then(|v| v.as_str())
                .filter(|s| s.starts_with("http"))
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

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = YandeImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("yande.re/post.json"));
        assert!(urls[0].url.contains("tags=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
        assert!(urls[0].url.contains("limit=50"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = YandeImageSource::new();
        let json = r#"[
            {
                "id": 777,
                "file_url": "https://yande.re/image/777.png",
                "jpeg_url": "https://yande.re/jpeg/777.jpg",
                "sample_url": "https://yande.re/sample/777.jpg",
                "preview_url": "https://yande.re/preview/777.jpg"
            },
            {
                "id": 888,
                "file_url": "https://yande.re/image/888.jpg",
                "preview_url": "https://yande.re/preview/888.jpg"
            }
        ]"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        // Identifier should be file_url
        assert_eq!(results[0].identifier, "https://yande.re/image/777.png");
        assert_eq!(results[0].candidate_download_urls.len(), 4);
        assert_eq!(results[1].identifier, "https://yande.re/image/888.jpg");
        assert_eq!(results[1].candidate_download_urls.len(), 2);
    }

    #[test]
    fn test_parse_empty_results() {
        let source = YandeImageSource::new();
        let json = r#"[]"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_filters_non_http_urls() {
        let source = YandeImageSource::new();
        let json = r#"[
            {
                "id": 999,
                "file_url": "https://yande.re/image/999.jpg",
                "jpeg_url": "//yande.re/jpeg/999.jpg",
                "preview_url": "https://yande.re/preview/999.jpg"
            }
        ]"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // Only http URLs should be included (jpeg_url starting with // is filtered out)
        assert_eq!(results[0].candidate_download_urls.len(), 2);
    }
}
