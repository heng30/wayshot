//! Wikimedia Commons image search source.
//!
//! Replaces Python's `WikipediaImageClient`. Uses the Wikimedia Commons
//! API with JSON response parsing.
//!
//! The API uses the MediaWiki `action=query` endpoint with
//! `generator=search` to find images in the File namespace (6).

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Wikimedia API requests.
const DEFAULT_PAGE_SIZE: usize = 50;

/// Wikimedia Commons image search source.
pub struct WikipediaImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl WikipediaImageSource {
    /// Create a new Wikimedia Commons image source.
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

impl Default for WikipediaImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for WikipediaImageSource {
    fn source_name(&self) -> &str {
        "wikipedia"
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
        let base_url = "https://commons.wikimedia.org/w/api.php?";
        let page_size = filters
            .get("gsrlimit")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}action=query&generator=search&gsrsearch={}&gsrlimit={}&gsroffset={}&prop=imageinfo&iiprop=url&iiurlwidth=800&format=json&gsrnamespace=6",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    pn * page_size,
                );
                for (key, value) in filters {
                    if key != "gsrlimit" && key != "gsrsearch" && key != "gsroffset"
                        && key != "action" && key != "generator" && key != "prop"
                        && key != "iiprop" && key != "iiurlwidth" && key != "format"
                        && key != "gsrnamespace"
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

        let pages = search_result
            .get("query")
            .and_then(|q| q.get("pages"))
            .and_then(|p| p.as_object());

        if let Some(page_map) = pages {
            for (_page_id, page) in page_map {
                if !page.is_object() {
                    continue;
                }

                // Each page can have multiple imageinfo entries; create a
                // separate ImageInfo for each URL.
                let image_infos_arr = page.get("imageinfo").and_then(|v| v.as_array());

                if let Some(info_list) = image_infos_arr {
                    for info in info_list {
                        if !info.is_object() {
                            continue;
                        }

                        // Prefer the full URL, use thumburl as fallback candidate
                        let mut candidate_urls = Vec::new();
                        if let Some(url) = info.get("url").and_then(|v| v.as_str()) {
                            if url.starts_with("http") {
                                candidate_urls.push(url.to_string());
                            }
                        }
                        if let Some(url) = info.get("thumburl").and_then(|v| v.as_str()) {
                            if url.starts_with("http") && !candidate_urls.contains(&url.to_string())
                            {
                                candidate_urls.push(url.to_string());
                            }
                        }

                        if candidate_urls.is_empty() {
                            continue;
                        }

                        let identifier = candidate_urls[0].clone();

                        image_infos.push(ImageInfo {
                            source: self.source_name().to_string(),
                            download_url: None,
                            candidate_download_urls: candidate_urls,
                            description: page
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            identifier,
                            work_dir: Default::default(),
                            ext: None,
                            save_name: None,
                            save_path: None,
                            extra: page.clone(),
                        });
                    }
                }
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
        let source = WikipediaImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("gsrsearch=cats"));
        assert!(urls[0].url.contains("gsroffset=0"));
        assert!(urls[0].url.contains("gsrnamespace=6"));
        assert!(urls[1].url.contains("gsroffset=50"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = WikipediaImageSource::new();
        let json = r#"{
            "query": {
                "pages": {
                    "123": {
                        "title": "File:Cat photo.jpg",
                        "imageinfo": [
                            {
                                "url": "https://upload.wikimedia.org/wikipedia/commons/c/cat_photo.jpg",
                                "thumburl": "https://upload.wikimedia.org/wikipedia/commons/thumb/c/cat_photo/800px-cat_photo.jpg"
                            }
                        ]
                    },
                    "456": {
                        "title": "File:Another cat.png",
                        "imageinfo": [
                            {
                                "url": "https://upload.wikimedia.org/wikipedia/commons/a/another_cat.png"
                            }
                        ]
                    }
                }
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].description, "File:Cat photo.jpg");
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        assert_eq!(results[0].candidate_download_urls[0], "https://upload.wikimedia.org/wikipedia/commons/c/cat_photo.jpg");
        assert_eq!(results[1].description, "File:Another cat.png");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = WikipediaImageSource::new();
        let json = r#"{"query": {"pages": {}}}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_no_imageinfo_skipped() {
        let source = WikipediaImageSource::new();
        let json = r#"{
            "query": {
                "pages": {
                    "789": {
                        "title": "File:No info.jpg"
                    }
                }
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
