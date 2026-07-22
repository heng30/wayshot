//! Statens Museum for Kunst (SMK) / Danish National Gallery image search source.
//!
//! Replaces Python's `SMKImageClient`. Uses the SMK open access API
//! with JSON response parsing and IIIF image URL construction.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Statens Museum for Kunst (SMK) image search source.
pub struct SmkImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl SmkImageSource {
    /// Create a new SMK image source.
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            "accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        search_headers.insert("referer", "https://open.smk.dk/".parse().unwrap());
        search_headers.insert(USER_AGENT, ua.parse().unwrap());

        let mut download_headers = HeaderMap::new();
        download_headers.insert("referer", "https://open.smk.dk/".parse().unwrap());
        download_headers.insert(USER_AGENT, ua.parse().unwrap());

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for SmkImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for SmkImageSource {
    fn source_name(&self) -> &str {
        "smk"
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
        let base_url = "https://api.smk.dk/api/v1/art/search/?";
        let page_size = filters
            .get("rows")
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as usize;
        let page_size = page_size.clamp(1, 100);

        let num_pages = (params.search_limits as f64 * 1.2 / page_size as f64).ceil() as usize;

        (0..num_pages)
            .map(|page_idx| {
                let offset = page_idx * page_size;
                let mut url = format!(
                    "{}keys={}&filters=%5Bpublic_domain%3Atrue%5D%2C%5Bhas_image%3Atrue%5D&offset={}&rows={}",
                    base_url,
                    urlencoding::encode(keyword),
                    offset,
                    page_size,
                );
                // Add extra filter params
                for (key, value) in filters {
                    if key != "keys" && key != "filters" && key != "offset" && key != "rows" {
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

        let items = search_result.get("items").and_then(|v| v.as_array());
        if let Some(items) = items {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let mut candidate_urls = Vec::new();

                // Primary IIIF image
                if let Some(iiif_id) = item
                    .get("image_iiif_id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    let iiif_url = format!(
                        "{}/full/!800,800/0/default.jpg",
                        iiif_id.trim_end_matches('/')
                    );
                    candidate_urls.push(iiif_url);
                }

                // Native image URL
                if let Some(url) = item
                    .get("image_native")
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                {
                    candidate_urls.push(url.to_string());
                }

                // Thumbnail image URL
                if let Some(url) = item
                    .get("image_thumbnail")
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                {
                    candidate_urls.push(url.to_string());
                }

                // Alternative images
                if let Some(alts) = item.get("alternative_images").and_then(|v| v.as_array()) {
                    for alt in alts {
                        if !alt.is_object() {
                            continue;
                        }
                        if let Some(url) = alt
                            .get("native")
                            .and_then(|v| v.as_str())
                            .filter(|s| s.starts_with("http"))
                        {
                            candidate_urls.push(url.to_string());
                        }
                        if let Some(url) = alt
                            .get("thumbnail")
                            .and_then(|v| v.as_str())
                            .filter(|s| s.starts_with("http"))
                        {
                            candidate_urls.push(url.to_string());
                        }
                    }
                }

                // Deduplicate while preserving order
                let mut seen = std::collections::HashSet::new();
                candidate_urls.retain(|url| seen.insert(url.clone()));

                if candidate_urls.is_empty() {
                    continue;
                }

                // Use id or object_number as identifier, fallback to first URL
                let identifier = item
                    .get("id")
                    .and_then(|v| v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(|s| s.to_string())))
                    .or_else(|| {
                        item.get("object_number")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| candidate_urls[0].clone());

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
                    description: item
                        .get("titles")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("title"))
                        .and_then(|v| v.as_str())
                        .or_else(|| item.get("title").and_then(|v| v.as_str()))
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
        let source = SmkImageSource::new();
        let params = SearchParams {
            search_limits: 200,
            ..Default::default()
        };
        let urls = source.construct_search_urls("rembrandt", &params, &Filters::new());
        // 200 * 1.2 / 100 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("keys=rembrandt"));
        assert!(urls[0].url.contains("offset=0"));
        assert!(urls[1].url.contains("offset=100"));
        assert!(urls[0].url.contains("rows=100"));
    }

    #[test]
    fn test_parse_search_result_with_iiif() {
        let source = SmkImageSource::new();
        let json = r#"{
            "items": [
                {
                    "id": 12345,
                    "object_number": "KKS1234",
                    "image_iiif_id": "https://api.smk.dk/iiif/abc123",
                    "image_native": "https://api.smk.dk/native/abc123.jpg",
                    "image_thumbnail": "https://api.smk.dk/thumb/abc123.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "12345");
        // IIIF URL should be constructed from image_iiif_id
        assert!(results[0].candidate_download_urls[0].contains("full/!800,800/0/default.jpg"));
        assert!(results[0].candidate_download_urls[1].contains("native/abc123.jpg"));
    }

    #[test]
    fn test_parse_search_result_with_alternative_images() {
        let source = SmkImageSource::new();
        let json = r#"{
            "items": [
                {
                    "id": 67890,
                    "image_native": "https://api.smk.dk/native/def456.jpg",
                    "alternative_images": [
                        {
                            "native": "https://api.smk.dk/alt/native1.jpg",
                            "thumbnail": "https://api.smk.dk/alt/thumb1.jpg"
                        }
                    ]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // native, alt native, alt thumbnail
        assert_eq!(results[0].candidate_download_urls.len(), 3);
    }

    #[test]
    fn test_parse_deduplicates_urls() {
        let source = SmkImageSource::new();
        let json = r#"{
            "items": [
                {
                    "id": 111,
                    "image_native": "https://api.smk.dk/same.jpg",
                    "image_thumbnail": "https://api.smk.dk/same.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // Same URL should appear only once
        assert_eq!(results[0].candidate_download_urls.len(), 1);
    }

    #[test]
    fn test_parse_skips_items_without_images() {
        let source = SmkImageSource::new();
        let json = r#"{
            "items": [
                {
                    "id": 999,
                    "image_iiif_id": null,
                    "image_native": null
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_empty_results() {
        let source = SmkImageSource::new();
        let json = r#"{"items": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
