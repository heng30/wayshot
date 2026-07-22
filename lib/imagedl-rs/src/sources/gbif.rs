//! GBIF (Global Biodiversity Information Facility) image search source.
//!
//! Replaces Python's `GBIFImageClient`. Uses the GBIF occurrence search API
//! at `https://api.gbif.org/v1/occurrence/search` with JSON response parsing.
//!
//! The GBIF API returns occurrence records, each of which may have media entries.
//! Only media entries of type "StillImage" with an "identifier" field are used
//! as image sources.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// GBIF image search source.
pub struct GbifImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

/// GBIF default page size.
const PAGE_SIZE: usize = 20;

impl GbifImageSource {
    /// Create a new GBIF image source.
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

impl Default for GbifImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for GbifImageSource {
    fn source_name(&self) -> &str {
        "gbif"
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
        let base_url = "https://api.gbif.org/v1/occurrence/search?";
        let page_size = filters
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(PAGE_SIZE as i64) as usize;
        let num_pages =
            ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url_params = vec![
                    format!("q={}", urlencoding::encode(keyword)),
                    "mediaType=STILL_IMAGE".to_string(),
                    format!("limit={}", page_size),
                    format!("offset={}", pn * page_size),
                ];

                // Add extra filter params (excluding limit which we already handled)
                for (key, value) in filters {
                    if key != "limit" {
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
        let search_result: Value = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        let mut image_infos = Vec::new();

        let results = search_result.get("results").and_then(|v| v.as_array());
        if let Some(occurrences) = results {
            for occurrence in occurrences {
                if !occurrence.is_object() {
                    continue;
                }

                // Get the gbifID and key for identifiers
                let gbif_id = occurrence
                    .get("gbifID")
                    .and_then(|v| v.as_i64())
                    .map(|id| id.to_string());
                let key = occurrence
                    .get("key")
                    .and_then(|v| v.as_i64())
                    .map(|k| k.to_string());

                // Get scientific name for description
                let scientific_name = occurrence
                    .get("scientificName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Extract StillImage media entries
                let media = occurrence.get("media").and_then(|v| v.as_array());
                if let Some(media_list) = media {
                    for media_item in media_list {
                        if !media_item.is_object() {
                            continue;
                        }

                        // Only process StillImage type media
                        let media_type = media_item
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if media_type != "StillImage" {
                            continue;
                        }

                        let identifier_url = media_item
                            .get("identifier")
                            .and_then(|v| v.as_str())
                            .filter(|s| s.starts_with("http"))
                            .map(|s| s.to_string());

                        if identifier_url.is_none() {
                            continue;
                        }

                        let url = identifier_url.unwrap();

                        // Use gbifID as identifier, fallback to key, then to URL
                        let identifier = gbif_id
                            .clone()
                            .or_else(|| key.clone())
                            .unwrap_or_else(|| url.clone());

                        image_infos.push(ImageInfo {
                            source: self.source_name().to_string(),
                            download_url: None,
                            candidate_download_urls: vec![url],
                            description: scientific_name.to_string(),
                            identifier,
                            work_dir: Default::default(),
                            ext: None,
                            save_name: None,
                            save_path: None,
                            extra: media_item.clone(),
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
        let source = GbifImageSource::new();
        let params = SearchParams {
            search_limits: 40,
            ..Default::default()
        };
        let urls = source.construct_search_urls("butterfly", &params, &Filters::new());
        // 40 * 1.2 / 20 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("api.gbif.org/v1/occurrence/search"));
        assert!(urls[0].url.contains("q=butterfly"));
        assert!(urls[0].url.contains("mediaType=STILL_IMAGE"));
        assert!(urls[0].url.contains("offset=0"));
        assert!(urls[1].url.contains("offset=20"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = GbifImageSource::new();
        let json = r#"{
            "results": [
                {
                    "gbifID": 1234567890,
                    "key": 1234567890,
                    "scientificName": "Danaus plexippus",
                    "media": [
                        {
                            "type": "StillImage",
                            "identifier": "https://example.com/butterfly1.jpg"
                        },
                        {
                            "type": "Sound",
                            "identifier": "https://example.com/sound1.mp3"
                        }
                    ]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        // Only StillImage media should be included
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "1234567890");
        assert_eq!(results[0].description, "Danaus plexippus");
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://example.com/butterfly1.jpg"
        );
    }

    #[test]
    fn test_parse_empty_results() {
        let source = GbifImageSource::new();
        let json = r#"{"results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_no_media() {
        let source = GbifImageSource::new();
        let json = r#"{
            "results": [
                {
                    "gbifID": 999,
                    "scientificName": "No media species"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_non_http_identifier() {
        let source = GbifImageSource::new();
        let json = r#"{
            "results": [
                {
                    "gbifID": 888,
                    "scientificName": "Bad URL species",
                    "media": [
                        {
                            "type": "StillImage",
                            "identifier": "notaurl"
                        }
                    ]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_custom_limit_filter() {
        let source = GbifImageSource::new();
        let mut filters = Filters::new();
        filters.insert("limit".to_string(), crate::types::FilterValue::from(50));
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("butterfly", &params, &filters);
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("limit=50"));
    }
}
