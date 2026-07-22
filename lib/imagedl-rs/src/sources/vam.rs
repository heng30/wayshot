//! Victoria and Albert Museum (V&A) image search source.
//!
//! Replaces Python's `VAMImageClient`. Uses the V&A's open access API
//! with JSON response parsing and IIIF image URL construction.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Victoria and Albert Museum (V&A) image search source.
pub struct VamImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl VamImageSource {
    /// Create a new V&A image source.
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            "accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        search_headers.insert(
            "referer",
            "https://collections.vam.ac.uk/".parse().unwrap(),
        );
        search_headers.insert(USER_AGENT, ua.parse().unwrap());

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            "referer",
            "https://collections.vam.ac.uk/".parse().unwrap(),
        );
        download_headers.insert(USER_AGENT, ua.parse().unwrap());

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for VamImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for VamImageSource {
    fn source_name(&self) -> &str {
        "vam"
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
        let base_url = "https://api.vam.ac.uk/v2/objects/search?";
        let page_size = filters
            .get("page_size")
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as usize;
        let page_size = page_size.clamp(1, 100);

        let num_pages = (params.search_limits as f64 * 1.2 / page_size as f64).ceil() as usize;

        (0..num_pages)
            .map(|page_idx| {
                let page = page_idx + 1;
                let mut url = format!(
                    "{}q={}&images_exist=1&page_size={}&page={}",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    page,
                );
                // Add extra filter params
                for (key, value) in filters {
                    if key != "q" && key != "images_exist" && key != "page_size" && key != "page" {
                        if let Some(s) = value.as_str() {
                            url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
                        }
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

        let records = search_result.get("records").and_then(|v| v.as_array());
        if let Some(items) = records {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let mut candidate_urls = Vec::new();

                // IIIF image base URL
                let images = item.get("_images").and_then(|v| v.as_object());
                if let Some(image_data) = images {
                    if let Some(base_url) = image_data
                        .get("_iiif_image_base_url")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        let iiif_url = format!(
                            "{}/full/full/0/default.jpg",
                            base_url.trim_end_matches('/')
                        );
                        candidate_urls.push(iiif_url);
                    }

                    // Primary thumbnail as fallback
                    if let Some(url) = image_data
                        .get("_primary_thumbnail")
                        .and_then(|v| v.as_str())
                        .filter(|s| s.starts_with("http"))
                    {
                        candidate_urls.push(url.to_string());
                    }
                }

                // Deduplicate while preserving order
                let mut seen = std::collections::HashSet::new();
                candidate_urls.retain(|url| seen.insert(url.clone()));

                if candidate_urls.is_empty() {
                    continue;
                }

                // Use systemNumber as identifier, fallback to _primaryImageId or first URL
                let identifier = item
                    .get("systemNumber")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        item.get("_primaryImageId")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| candidate_urls[0].clone());

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
                    description: item
                        .get("_primaryTitle")
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = VamImageSource::new();
        let params = SearchParams {
            search_limits: 150,
            ..Default::default()
        };
        let urls = source.construct_search_urls("ceramics", &params, &Filters::new());
        // 150 * 1.2 / 100 = 1.8 -> ceil = 2 pages
        assert_eq!(urls.len(), 2);
        assert!(urls[0].url.contains("q=ceramics"));
        assert!(urls[0].url.contains("images_exist=1"));
        assert!(urls[0].url.contains("page_size=100"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = VamImageSource::new();
        let json = r#"{
            "records": [
                {
                    "systemNumber": "O123456",
                    "_primaryTitle": "Chinese Vase",
                    "_images": {
                        "_iiif_image_base_url": "https://api.vam.ac.uk/iiif/O123456",
                        "_primary_thumbnail": "https://api.vam.ac.uk/thumb/O123456.jpg"
                    }
                },
                {
                    "systemNumber": "O789012",
                    "_primaryTitle": "English Teapot",
                    "_images": {
                        "_iiif_image_base_url": "https://api.vam.ac.uk/iiif/O789012"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "O123456");
        assert_eq!(results[0].description, "Chinese Vase");
        // IIIF URL first, then thumbnail
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        assert!(results[0].candidate_download_urls[0].contains("full/full/0/default.jpg"));
        assert!(results[0].candidate_download_urls[1].contains("thumb/O123456.jpg"));
        assert_eq!(results[1].candidate_download_urls.len(), 1);
    }

    #[test]
    fn test_parse_iiif_url_construction() {
        let source = VamImageSource::new();
        let json = r#"{
            "records": [
                {
                    "systemNumber": "O999",
                    "_images": {
                        "_iiif_image_base_url": "https://api.vam.ac.uk/iiif/O999/"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // Trailing slash should be handled
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://api.vam.ac.uk/iiif/O999/full/full/0/default.jpg"
        );
    }

    #[test]
    fn test_parse_skips_records_without_images() {
        let source = VamImageSource::new();
        let json = r#"{
            "records": [
                {
                    "systemNumber": "O111",
                    "_images": {}
                },
                {
                    "systemNumber": "O222",
                    "_images": null
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_uses_fallback_identifier() {
        let source = VamImageSource::new();
        let json = r#"{
            "records": [
                {
                    "_primaryImageId": "IMG123",
                    "_images": {
                        "_iiif_image_base_url": "https://api.vam.ac.uk/iiif/test"
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "IMG123");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = VamImageSource::new();
        let json = r#"{"records": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
