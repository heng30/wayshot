//! Huaban image search source.
//!
//! Replaces Python's `HuabanImageClient`. Uses Huaban's REST API
//! with JSON response parsing.
//!
//! URLs starting with `//` are prefixed with `https:` to form valid URLs.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Huaban API requests.
const DEFAULT_PAGE_SIZE: usize = 40;

/// Huaban image search source.
pub struct HuabanImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl HuabanImageSource {
    /// Create a new Huaban image source.
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

    /// Fix a URL that starts with `//` by prepending `https:`.
    fn fix_url(url: &str) -> String {
        if url.starts_with("//") {
            format!("https:{}", url)
        } else {
            url.to_string()
        }
    }

    /// Recursively search a JSON value for all "file" keys that contain
    /// objects with a "url" field.
    fn search_file_dicts(node: &Value) -> Vec<&Value> {
        let mut results = Vec::new();

        if let Some(obj) = node.as_object() {
            for (key, value) in obj {
                if key == "file" && value.is_object() {
                    results.push(value);
                }
                results.extend(Self::search_file_dicts(value));
            }
        } else if let Some(arr) = node.as_array() {
            for item in arr {
                results.extend(Self::search_file_dicts(item));
            }
        }

        results
    }
}

impl Default for HuabanImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for HuabanImageSource {
    fn source_name(&self) -> &str {
        "huaban"
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
        let base_url = "https://huaban.com/v3/search/file?";
        let page_size = filters
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}text={}&sort=all&limit={}&page={}&position=search_pins&fields=pins:PIN|total,facets,split_words,relations,rec_topic_material,topics",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    pn + 1,
                );
                for (key, value) in filters {
                    if key != "limit" && key != "text" && key != "page"
                        && key != "sort" && key != "position" && key != "fields"
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

        // Recursively find all "file" dicts in the response
        let file_dicts = Self::search_file_dicts(&search_result);

        for file_dict in file_dicts {
            let file_url = match file_dict.get("url").and_then(|v| v.as_str()) {
                Some(url) => Self::fix_url(url),
                None => continue,
            };

            if !file_url.starts_with("http") {
                continue;
            }

            image_infos.push(ImageInfo {
                source: self.source_name().to_string(),
                download_url: None,
                candidate_download_urls: vec![file_url.clone()],
                description: String::new(),
                identifier: file_url.clone(),
                work_dir: Default::default(),
                ext: None,
                save_name: None,
                save_path: None,
                extra: file_dict.clone(),
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
        let source = HuabanImageSource::new();
        let params = SearchParams {
            search_limits: 80,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 80 * 1.2 / 40 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("text=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("limit=40"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = HuabanImageSource::new();
        let json = r#"{
            "data": [
                {
                    "file": {
                        "url": "//hbimg.huabanimg.com/img1.jpg"
                    }
                },
                {
                    "file": {
                        "url": "https://hbimg.hu.com/img2.jpg"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].candidate_download_urls[0], "https://hbimg.huabanimg.com/img1.jpg");
        assert_eq!(results[1].candidate_download_urls[0], "https://hbimg.hu.com/img2.jpg");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = HuabanImageSource::new();
        let json = r#"{"data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_fix_url() {
        assert_eq!(HuabanImageSource::fix_url("//example.com/img.jpg"), "https://example.com/img.jpg");
        assert_eq!(HuabanImageSource::fix_url("https://example.com/img.jpg"), "https://example.com/img.jpg");
        assert_eq!(HuabanImageSource::fix_url("http://example.com/img.jpg"), "http://example.com/img.jpg");
    }

    #[test]
    fn test_search_file_dicts_nested() {
        let json = serde_json::json!({
            "pins": [
                {"file": {"url": "//img1.jpg"}, "other": "data"},
                {"file": {"url": "//img2.jpg"}}
            ],
            "nested": {
                "deep": {
                    "file": {"url": "//img3.jpg"}
                }
            }
        });
        let results = HuabanImageSource::search_file_dicts(&json);
        assert_eq!(results.len(), 3);
    }
}
