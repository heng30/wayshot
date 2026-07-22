//! Internet Archive image search source.
//!
//! Replaces Python's `InternetArchiveImageClient`. Uses the Internet Archive
//! Advanced Search API with JSON response parsing.
//!
//! Image URLs are constructed from item identifiers using the
//! `https://archive.org/services/img/{identifier}` endpoint.

use reqwest::header::{HeaderMap, ACCEPT, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Internet Archive API requests.
const DEFAULT_PAGE_SIZE: usize = 50;

/// Internet Archive image search source.
pub struct InternetArchiveImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl InternetArchiveImageSource {
    /// Create a new Internet Archive image source.
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
            "application/json,text/plain,*/*".parse().unwrap(),
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

impl Default for InternetArchiveImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for InternetArchiveImageSource {
    fn source_name(&self) -> &str {
        "internetarchive"
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
        let base_url = "https://archive.org/advancedsearch.php?";
        let page_size = filters
            .get("rows")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let query = format!("({}) AND mediatype:image", keyword);
                let mut url = format!(
                    "{}q={}&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=creator&fl%5B%5D=date&fl%5B%5D=mediatype&rows={}&page={}&output=json",
                    base_url,
                    urlencoding::encode(&query),
                    page_size,
                    pn + 1,
                );
                for (key, value) in filters {
                    if key != "rows" && key != "q" && key != "page" && key != "output"
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

        let docs = search_result
            .get("response")
            .and_then(|r| r.get("docs"))
            .and_then(|v| v.as_array());

        if let Some(items) = docs {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let identifier = match item.get("identifier").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => continue,
                };

                // Build image URL from identifier
                let image_url = format!("https://archive.org/services/img/{}", identifier);

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: vec![image_url.clone()],
                    description: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    identifier: identifier.to_string(),
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
        let source = InternetArchiveImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("vintage photos", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("advancedsearch.php"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("output=json"));
        assert!(urls[0].url.contains("mediatype%3Aimage"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = InternetArchiveImageSource::new();
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "identifier": "vintage-photos-001",
                        "title": "Vintage Photo Collection 1",
                        "creator": "John Doe",
                        "mediatype": "image"
                    },
                    {
                        "identifier": "vintage-photos-002",
                        "title": "Vintage Photo Collection 2"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "vintage-photos-001");
        assert_eq!(results[0].description, "Vintage Photo Collection 1");
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://archive.org/services/img/vintage-photos-001"
        );
        assert_eq!(results[1].identifier, "vintage-photos-002");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = InternetArchiveImageSource::new();
        let json = r#"{"response": {"docs": []}}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_missing_identifier_skipped() {
        let source = InternetArchiveImageSource::new();
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "title": "No identifier"
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
