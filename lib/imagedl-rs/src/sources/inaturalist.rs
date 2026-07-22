//! iNaturalist image search source.
//!
//! Replaces Python's `INaturalistImageClient`. Uses the iNaturalist API
//! at `https://api.inaturalist.org/v1/observations` with JSON response parsing.
//!
//! Each observation may have multiple photos. This source flattens all photos
//! from all observations into individual `ImageInfo` entries. Photo URLs are
//! upgraded from "square" size to "large" or "medium" when possible.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// iNaturalist image search source.
pub struct INaturalistImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

/// iNaturalist page size.
const PAGE_SIZE: usize = 50;

impl INaturalistImageSource {
    /// Create a new iNaturalist image source.
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

    /// Upgrade an iNaturalist photo URL by replacing "square" with a larger size.
    ///
    /// iNaturalist photo URLs contain a size segment like "square" that can be
    /// replaced with "large", "medium", or "small" to get different resolutions.
    fn upgrade_url(url: &str, target_size: &str) -> Option<String> {
        if url.is_empty() || !url.starts_with("http") {
            return None;
        }
        Some(url.replace("square", target_size))
    }
}

impl Default for INaturalistImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for INaturalistImageSource {
    fn source_name(&self) -> &str {
        "inaturalist"
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
        let base_url = "https://api.inaturalist.org/v1/observations?";
        let num_pages =
            ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url_params = vec![
                    format!("q={}", urlencoding::encode(keyword)),
                    "photos=true".to_string(),
                    "quality_grade=research".to_string(),
                    format!("per_page={}", PAGE_SIZE),
                    "order=desc".to_string(),
                    "order_by=votes".to_string(),
                    format!("page={}", pn + 1),
                ];

                // Add extra filter params
                for (key, value) in filters {
                    if let Some(s) = value.as_str() {
                        url_params.push(format!(
                            "{}={}",
                            urlencoding::encode(key),
                            urlencoding::encode(s)
                        ));
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
        if let Some(observations) = results {
            for observation in observations {
                if !observation.is_object() {
                    continue;
                }

                let obs_id = observation
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                let photos = observation.get("photos").and_then(|v| v.as_array());
                if let Some(photo_list) = photos {
                    for (photo_idx, photo) in photo_list.iter().enumerate() {
                        if !photo.is_object() {
                            continue;
                        }

                        let original_url = photo
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        // Build candidate URLs: large > medium > original (square)
                        let large_url = Self::upgrade_url(original_url, "large");
                        let medium_url = Self::upgrade_url(original_url, "medium");
                        let square_url = if original_url.starts_with("http") {
                            Some(original_url.to_string())
                        } else {
                            None
                        };

                        let candidate_urls: Vec<String> =
                            [large_url, medium_url, square_url]
                                .into_iter()
                                .flatten()
                                .collect();

                        if candidate_urls.is_empty() {
                            continue;
                        }

                        // Use photo id as identifier, fallback to observation id + photo index
                        let identifier = photo
                            .get("id")
                            .and_then(|v| v.as_i64())
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| format!("{}_{}", obs_id, photo_idx));

                        image_infos.push(ImageInfo {
                            source: self.source_name().to_string(),
                            download_url: None,
                            candidate_download_urls: candidate_urls,
                            description: observation
                                .get("species_guess")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            identifier,
                            work_dir: Default::default(),
                            ext: None,
                            save_name: None,
                            save_path: None,
                            extra: photo.clone(),
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
        let source = INaturalistImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("butterfly", &params, &Filters::new());
        // 100 * 1.2 / 50 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("api.inaturalist.org/v1/observations"));
        assert!(urls[0].url.contains("q=butterfly"));
        assert!(urls[0].url.contains("photos=true"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = INaturalistImageSource::new();
        let json = r#"{
            "results": [
                {
                    "id": 12345,
                    "species_guess": "Monarch Butterfly",
                    "photos": [
                        {
                            "id": 111,
                            "url": "https://inaturalist-open-data.s3.amazonaws.com/photos/111/square.jpg"
                        },
                        {
                            "id": 222,
                            "url": "https://inaturalist-open-data.s3.amazonaws.com/photos/222/square.jpg"
                        }
                    ]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "111");
        assert_eq!(results[0].description, "Monarch Butterfly");
        // Should have large, medium, and square URLs
        assert_eq!(results[0].candidate_download_urls.len(), 3);
        assert!(results[0].candidate_download_urls[0].contains("/large.jpg"));
        assert!(results[0].candidate_download_urls[1].contains("/medium.jpg"));
        assert!(results[0].candidate_download_urls[2].contains("/square.jpg"));
        assert_eq!(results[1].identifier, "222");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = INaturalistImageSource::new();
        let json = r#"{"results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_observation_no_photos() {
        let source = INaturalistImageSource::new();
        let json = r#"{
            "results": [
                {
                    "id": 999,
                    "species_guess": "No photo obs"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_upgrade_url() {
        assert_eq!(
            INaturalistImageSource::upgrade_url(
                "https://example.com/photos/111/square.jpg",
                "large"
            ),
            Some("https://example.com/photos/111/large.jpg".to_string())
        );
        assert_eq!(
            INaturalistImageSource::upgrade_url(
                "https://example.com/photos/111/square.jpg",
                "medium"
            ),
            Some("https://example.com/photos/111/medium.jpg".to_string())
        );
        assert_eq!(INaturalistImageSource::upgrade_url("", "large"), None);
        assert_eq!(
            INaturalistImageSource::upgrade_url("notaurl", "large"),
            None
        );
    }
}
