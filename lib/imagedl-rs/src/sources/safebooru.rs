//! Safebooru image board source.
//!
//! Replaces Python's `SafebooruImageClient`. Uses the Safebooru JSON API
//! to search for images. Single page request with no pagination.
//! URL prefixes like "//" are prefixed with "https:".

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Safebooru image board source.
///
/// Uses the Safebooru API at `https://safebooru.org/index.php` with
/// a single page request. URLs that start with `"//"` are prefixed with
/// `"https:"` to form valid absolute URLs.
pub struct SafebooruImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl SafebooruImageSource {
    /// Create a new Safebooru image source.
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

    /// Fix URL prefix: prepend "https:" to protocol-relative URLs (starting with "//").
    fn fix_url(url: &str) -> Option<String> {
        if url.is_empty() {
            return None;
        }
        if url.starts_with("//") {
            Some(format!("https:{}", url))
        } else if url.starts_with("http") {
            Some(url.to_string())
        } else {
            None
        }
    }
}

impl Default for SafebooruImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for SafebooruImageSource {
    fn source_name(&self) -> &str {
        "safebooru"
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
        let base_url = "https://safebooru.org/index.php?";
        let mut url_params = vec![
            "page=dapi".to_string(),
            "s=post".to_string(),
            "q=index".to_string(),
            "json=1".to_string(),
            format!("tags={}", urlencoding::encode(keyword)),
            format!("limit={}", params.search_limits),
        ];
        for (key, value) in filters {
            if key != "page" && key != "s" && key != "q" && key != "json"
                && key != "tags" && key != "limit"
            {
                if let Some(s) = value.as_str() {
                    url_params.push(format!(
                        "{}={}",
                        urlencoding::encode(key),
                        urlencoding::encode(s)
                    ));
                }
            }
        }
        vec![SearchUrl::new(format!(
            "{}{}",
            base_url,
            url_params.join("&")
        ))]
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
                .and_then(Self::fix_url);
            let sample_url = item
                .get("sample_url")
                .and_then(|v| v.as_str())
                .and_then(Self::fix_url);
            let preview_url = item
                .get("preview_url")
                .and_then(|v| v.as_str())
                .and_then(Self::fix_url);

            let candidate_urls: Vec<String> = [file_url, sample_url, preview_url]
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
                    item.get("image")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .or_else(|| {
                    item.get("hash")
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
        let source = SafebooruImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        assert_eq!(urls.len(), 1);
        assert!(urls[0].url.contains("safebooru.org/index.php"));
        assert!(urls[0].url.contains("page=dapi"));
        assert!(urls[0].url.contains("s=post"));
        assert!(urls[0].url.contains("q=index"));
        assert!(urls[0].url.contains("json=1"));
        assert!(urls[0].url.contains("tags=cats"));
        assert!(urls[0].url.contains("limit=100"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = SafebooruImageSource::new();
        let json = r#"[
            {
                "id": 555,
                "image": "abc123.jpg",
                "hash": "def456",
                "file_url": "https://safebooru.org/images/555/abc123.jpg",
                "sample_url": "https://safebooru.org/samples/555/abc123.jpg",
                "preview_url": "https://safebooru.org/thumbnails/555/abc123.jpg"
            },
            {
                "id": 666,
                "file_url": "//safebooru.org/images/666/test.jpg"
            }
        ]"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "555");
        assert_eq!(results[0].candidate_download_urls.len(), 3);
        // Protocol-relative URL should be fixed
        assert!(results[1].candidate_download_urls[0].starts_with("https://"));
    }

    #[test]
    fn test_parse_empty_results() {
        let source = SafebooruImageSource::new();
        let json = r#"[]"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_identifier_fallback() {
        let source = SafebooruImageSource::new();
        let json = r#"[
            {
                "image": "fallback.jpg",
                "file_url": "https://safebooru.org/images/test.jpg"
            }
        ]"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // No "id", should fall back to "image"
        assert_eq!(results[0].identifier, "fallback.jpg");
    }

    #[test]
    fn test_fix_url() {
        assert_eq!(
            SafebooruImageSource::fix_url("//example.com/img.jpg"),
            Some("https://example.com/img.jpg".to_string())
        );
        assert_eq!(
            SafebooruImageSource::fix_url("https://example.com/img.jpg"),
            Some("https://example.com/img.jpg".to_string())
        );
        assert_eq!(SafebooruImageSource::fix_url(""), None);
    }
}
