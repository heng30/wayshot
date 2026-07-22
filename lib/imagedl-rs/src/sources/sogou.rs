//! Sogou image search source.
//!
//! Uses Sogou's image search page and extracts results from the
//! `__INITIAL_STATE__` JSON embedded in the HTML, since the direct
//! API endpoint (`/napi/pc/searchList`) now returns "forbid".

use reqwest::header::{HeaderMap, REFERER, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Sogou page size (items per page in __INITIAL_STATE__).
const PAGE_SIZE: usize = 48;

/// Sogou image search source.
pub struct SogouImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl SogouImageSource {
    /// Create a new Sogou image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            REFERER,
            "https://pic.sogou.com/".parse().unwrap(),
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

    /// Extract `__INITIAL_STATE__` JSON from the HTML page.
    fn extract_initial_state(body: &str) -> Option<String> {
        let marker = "__INITIAL_STATE__=";
        let start = body.find(marker)?;
        let json_start = start + marker.len();
        let rest = &body[json_start..];
        // Find the end of the JSON object by counting braces
        let mut depth = 0i32;
        for (i, c) in rest.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(rest[..=i].to_string());
                    }
                }
                _ => {}
            }
        }
        None
    }
}

impl Default for SogouImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for SogouImageSource {
    fn source_name(&self) -> &str {
        "sogou"
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
        let num_pages =
            ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let start = pn * PAGE_SIZE;
                SearchUrl::new(format!(
                    "https://pic.sogou.com/pics?query={}&start={}",
                    urlencoding::encode(keyword),
                    start,
                ))
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let json_str = Self::extract_initial_state(body).ok_or_else(|| ImageDlError::Parse {
            origin: self.source_name().to_string(),
            reason: "Failed to extract __INITIAL_STATE__ from HTML".to_string(),
        })?;

        let data: Value = serde_json::from_str(&json_str).map_err(|e| ImageDlError::Parse {
            origin: self.source_name().to_string(),
            reason: format!("JSON parse error: {}", e),
        })?;

        let items = data
            .get("searchList")
            .and_then(|v| v.get("searchList"))
            .and_then(|v| v.as_array());

        let items = match items {
            Some(i) => i,
            None => return Ok(vec![]),
        };

        let mut image_infos = Vec::new();

        for item in items {
            if !item.is_object() {
                continue;
            }

            let url_keys = ["picUrl", "oriPicUrl", "locImageLink", "thumbUrl"];
            let candidate_urls: Vec<String> = url_keys
                .iter()
                .filter_map(|key| {
                    item.get(*key)
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty() && s.starts_with("http"))
                        .map(|s| s.to_string())
                })
                .collect();

            if candidate_urls.is_empty() {
                continue;
            }

            let identifier = item
                .get("mf_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    item.get("docId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
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

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = SogouImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("pic.sogou.com/pics"));
        assert!(urls[0].url.contains("query=cats"));
        assert!(urls[0].url.contains("start=0"));
    }

    #[test]
    fn test_extract_initial_state() {
        let html = r#"<script>window.__INITIAL_STATE__={"searchList":{"searchList":[{"picUrl":"https://example.com/cat.jpg","mf_id":"mf_123","title":"A cat"}]}}</script>"#;
        let json = SogouImageSource::extract_initial_state(html).unwrap();
        assert!(json.contains("mf_123"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = SogouImageSource::new();
        let html = r#"<script>window.__INITIAL_STATE__={"searchList":{"searchList":[{"mf_id":"mf_123","oriPicUrl":"https://example.com/cat1_orig.jpg","thumbUrl":"https://example.com/cat1_thumb.jpg","title":"A cute cat"},{"docId":"doc_456","oriPicUrl":"https://example.com/cat2_orig.jpg"}]}}</script>"#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "mf_123");
        assert_eq!(results[0].description, "A cute cat");
        assert_eq!(results[1].identifier, "doc_456");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = SogouImageSource::new();
        let html = r#"<script>window.__INITIAL_STATE__={"searchList":{"searchList":[]}}</script>"#;
        let results = source.parse_search_result(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_data() {
        let source = SogouImageSource::new();
        let html = r#"<html><body>No state here</body></html>"#;
        let results = source.parse_search_result(html);
        assert!(results.is_err());
    }
}
