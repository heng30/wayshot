//! Wellcome Collection image search source.
//!
//! Replaces Python's `WellcomeImageClient`. Uses the Wellcome Collection
//! Catalogue API v2 with JSON response parsing.
//!
//! Image URLs are constructed from IIIF info.json URLs by converting them
//! to full-resolution image URLs.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Wellcome API requests.
const DEFAULT_PAGE_SIZE: usize = 25;

/// Maximum image dimension for IIIF image URL construction.
const IIIF_MAX_SIZE: usize = 1200;

/// Wellcome Collection image search source.
pub struct WellcomeImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl WellcomeImageSource {
    /// Create a new Wellcome image source.
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

    /// Convert a IIIF info.json URL to a full image URL.
    ///
    /// For example:
    /// - `https://example.org/image/info.json` becomes
    ///   `https://example.org/image/full/!1200,1200/0/default.jpg`
    /// - `https://example.org/image/infojson` (no dot before "info.json") becomes
    ///   `https://example.org/image/full/!1200,1200/0/default.jpg`
    /// - URLs not ending with "info.json" are returned as-is.
    fn info_url_to_image_url(info_url: &str) -> String {
        if info_url.is_empty() {
            return String::new();
        }
        if info_url.ends_with("/info.json") {
            let base = &info_url[..info_url.len() - "/info.json".len()];
            format!(
                "{}/full/!{},{}0/default.jpg",
                base, IIIF_MAX_SIZE, IIIF_MAX_SIZE
            )
        } else if info_url.ends_with("info.json") {
            // Handle case without trailing slash: "somethinginfo.json"
            let base = info_url[..info_url.len() - "info.json".len()]
                .trim_end_matches('/');
            format!(
                "{}/full/!{},{}0/default.jpg",
                base, IIIF_MAX_SIZE, IIIF_MAX_SIZE
            )
        } else {
            info_url.to_string()
        }
    }
}

impl Default for WellcomeImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for WellcomeImageSource {
    fn source_name(&self) -> &str {
        "wellcome"
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
        let base_url = "https://api.wellcomecollection.org/catalogue/v2/images?";
        let page_size = filters
            .get("pageSize")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}query={}&pageSize={}&page={}",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    pn + 1,
                );
                for (key, value) in filters {
                    if key != "pageSize" && key != "query" && key != "page"
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

                let image_id = match item.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => continue,
                };

                // Extract thumbnail URL and convert to image URL
                let mut candidate_urls = Vec::new();

                if let Some(thumb_url) = item
                    .get("thumbnail")
                    .and_then(|t| t.get("url"))
                    .and_then(|v| v.as_str())
                {
                    let image_url = Self::info_url_to_image_url(thumb_url);
                    if image_url.starts_with("http") {
                        candidate_urls.push(image_url);
                    }
                }

                // Also try locations[].url
                if let Some(locations) = item.get("locations").and_then(|v| v.as_array()) {
                    for loc in locations {
                        if let Some(url) = loc.get("url").and_then(|v| v.as_str()) {
                            let image_url = Self::info_url_to_image_url(url);
                            if image_url.starts_with("http")
                                && !candidate_urls.contains(&image_url)
                            {
                                candidate_urls.push(image_url);
                            }
                        }
                    }
                }

                if candidate_urls.is_empty() {
                    continue;
                }

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
                    description: item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    identifier: image_id.to_string(),
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
        let source = WellcomeImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("medicine", &params, &Filters::new());
        // 50 * 1.2 / 25 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("query=medicine"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("pageSize=25"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = WellcomeImageSource::new();
        let json = r#"{
            "results": [
                {
                    "id": "abc123",
                    "title": "Medical illustration",
                    "thumbnail": {
                        "url": "https://iiif.wellcomecollection.org/image/abc123/info.json"
                    }
                },
                {
                    "id": "def456",
                    "title": "Another image",
                    "thumbnail": {
                        "url": "https://iiif.wellcomecollection.org/image/def456/info.json"
                    },
                    "locations": [
                        {
                            "url": "https://iiif.wellcomecollection.org/image/def456_loc/info.json"
                        }
                    ]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "abc123");
        assert_eq!(results[0].description, "Medical illustration");
        assert!(results[0].candidate_download_urls[0].contains("/full/!1200,12000/default.jpg"));
        assert_eq!(results[1].candidate_download_urls.len(), 2);
    }

    #[test]
    fn test_parse_empty_results() {
        let source = WellcomeImageSource::new();
        let json = r#"{"results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_info_url_to_image_url() {
        // Standard info.json URL
        let result =
            WellcomeImageSource::info_url_to_image_url("https://example.org/image/info.json");
        assert_eq!(
            result,
            "https://example.org/image/full/!1200,12000/default.jpg"
        );

        // URL without trailing slash before info.json
        let result =
            WellcomeImageSource::info_url_to_image_url("https://example.org/abcinfo.json");
        assert_eq!(
            result,
            "https://example.org/abc/full/!1200,12000/default.jpg"
        );

        // Non-info.json URL returned as-is
        let result =
            WellcomeImageSource::info_url_to_image_url("https://example.org/image.jpg");
        assert_eq!(result, "https://example.org/image.jpg");

        // Empty string
        let result = WellcomeImageSource::info_url_to_image_url("");
        assert!(result.is_empty());
    }
}
