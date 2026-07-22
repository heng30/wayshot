//! 360 Search (So.com) image search source.
//!
//! Replaces Python's `I360ImageClient`. Uses the 360 Search image API
//! at `https://image.so.com/j` with JSON response parsing.
//! URLs that start with `"//"` are prefixed with `"https:"`.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::filter::{Filter, FilterRule};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// 360 Search image search source.
pub struct I360ImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

/// 360 Search page size.
const PAGE_SIZE: usize = 60;

impl I360ImageSource {
    /// Create a new 360 Search image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            "accept-language",
            "zh-CN,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2"
                .parse()
                .unwrap(),
        );
        search_headers.insert("connection", "keep-alive".parse().unwrap());
        search_headers.insert(
            "upgrade-insecure-requests",
            "1".parse().unwrap(),
        );
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
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

impl Default for I360ImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for I360ImageSource {
    fn source_name(&self) -> &str {
        "i360"
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
        let base_url = "https://image.so.com/j?";
        let num_pages =
            ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        let filter_str = self.build_filter().apply(filters, "&").unwrap_or_default();

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}pn={}&q={}&sn={}",
                    base_url,
                    PAGE_SIZE,
                    urlencoding::encode(keyword),
                    pn * PAGE_SIZE,
                );
                if !filter_str.is_empty() {
                    url.push_str(&format!("&{}", filter_str));
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

        let list = search_result.get("list").and_then(|v| v.as_array());
        if let Some(items) = list {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Extract candidate URLs in priority order
                let url_keys = ["img", "thumb_bak", "thumb", "_thumb_bak", "_thumb"];
                let candidate_urls: Vec<String> = url_keys
                    .iter()
                    .filter_map(|key| {
                        item.get(*key)
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                            .and_then(Self::fix_url)
                    })
                    .collect();

                if candidate_urls.is_empty() {
                    continue;
                }

                // Use id as identifier, fallback to first URL
                let identifier = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        item.get("id")
                            .and_then(|v| v.as_i64())
                            .map(|id| id.to_string())
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
        }

        Ok(image_infos)
    }

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // Size filter
        f.add_rule(FilterRule::with_string_choices(
            "size",
            vec!["all", "small", "medium", "large", "extralarge", "wallpaper"],
            |v| {
                let code = match v {
                    "all" => 0,
                    "small" => 1,
                    "medium" => 2,
                    "large" => 3,
                    "extralarge" => 4,
                    "wallpaper" => 4,
                    _ => 0,
                };
                format!("z={}", code)
            },
        ));

        // Color filter
        f.add_rule(FilterRule::with_string_choices(
            "color",
            vec![
                "red",
                "blue",
                "black",
                "white",
                "pink",
                "orange",
                "yellow",
                "green",
                "purple",
                "brown",
                "teal",
            ],
            |v| format!("imgcolor={}", v),
        ));

        // Type filter
        f.add_rule(FilterRule::with_string_choices(
            "type",
            vec!["animated", "static"],
            |v| {
                let code = match v {
                    "animated" => 1,
                    "static" => 2,
                    _ => 2,
                };
                format!("stype={}", code)
            },
        ));

        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = I360ImageSource::new();
        let params = SearchParams {
            search_limits: 120,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 120 * 1.2 / 60 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("image.so.com/j"));
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("pn=60"));
        assert!(urls[0].url.contains("sn=0"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = I360ImageSource::new();
        let json = r#"{
            "list": [
                {
                    "id": "abc123",
                    "img": "https://example.com/cat1.jpg",
                    "thumb": "https://example.com/thumb1.jpg",
                    "title": "A cute cat"
                },
                {
                    "id": 456,
                    "img": "//example.com/cat2.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "abc123");
        assert_eq!(results[0].description, "A cute cat");
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        // Protocol-relative URL should be fixed
        assert!(results[1].candidate_download_urls[0].starts_with("https://"));
        assert_eq!(results[1].identifier, "456");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = I360ImageSource::new();
        let json = r#"{"list": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_fix_url() {
        assert_eq!(
            I360ImageSource::fix_url("//example.com/img.jpg"),
            Some("https://example.com/img.jpg".to_string())
        );
        assert_eq!(
            I360ImageSource::fix_url("https://example.com/img.jpg"),
            Some("https://example.com/img.jpg".to_string())
        );
        assert_eq!(I360ImageSource::fix_url(""), None);
        assert_eq!(I360ImageSource::fix_url("notaurl"), None);
    }

    #[test]
    fn test_build_filter_size() {
        let source = I360ImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("size".to_string(), crate::types::FilterValue::from("large"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "z=3");
    }

    #[test]
    fn test_build_filter_color() {
        let source = I360ImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("color".to_string(), crate::types::FilterValue::from("red"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "imgcolor=red");
    }
}
