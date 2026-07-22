//! Art Institute of Chicago image search source.
//!
//! Replaces Python's `AICImageClient`. Uses the Art Institute of Chicago's
//! REST API with JSON response parsing and IIIF image URLs.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Art Institute of Chicago image search source.
pub struct AicImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl AicImageSource {
    /// Create a new AIC image source.
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

        let mut search_headers = HeaderMap::new();
        search_headers.insert(USER_AGENT, ua.parse().unwrap());

        let mut download_headers = HeaderMap::new();
        download_headers.insert(USER_AGENT, ua.parse().unwrap());

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for AicImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for AicImageSource {
    fn source_name(&self) -> &str {
        "aic"
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
        let base_url = "https://api.artic.edu/api/v1/artworks/search?";
        let limit = (params.search_limits as f64 * 1.2).ceil() as usize;

        let mut url = format!(
            "{}q={}&query%5Bterm%5D%5Bis_public_domain%5D=true&fields=id%2Ctitle%2Cimage_id%2Cis_public_domain&limit={}",
            base_url,
            urlencoding::encode(keyword),
            limit,
        );

        // Add extra filter params
        for (key, value) in filters {
            if key != "q" && key != "limit" && key != "fields" {
                if let Some(s) = value.as_str() {
                    url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
                }
            }
        }

        vec![SearchUrl::new(url)]
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let search_result: Value = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        // Extract the IIIF base URL from config
        let iiif_url = search_result
            .get("config")
            .and_then(|c| c.get("iiif_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("https://www.artic.edu/iiif/2");

        let mut image_infos = Vec::new();

        let data = search_result.get("data").and_then(|v| v.as_array());
        if let Some(items) = data {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let image_id = item.get("image_id").and_then(|v| v.as_str());
                let image_id = match image_id {
                    Some(id) if !id.is_empty() => id,
                    _ => continue,
                };

                let download_url = format!(
                    "{}/{}/full/843,/0/default.jpg",
                    iiif_url.trim_end_matches('/'),
                    image_id
                );

                let identifier = image_id.to_string();

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: vec![download_url],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = AicImageSource::new();
        let params = SearchParams {
            search_limits: 100,
            ..Default::default()
        };
        let urls = source.construct_search_urls("monet", &params, &Filters::new());
        assert_eq!(urls.len(), 1);
        assert!(urls[0].url.contains("q=monet"));
        assert!(urls[0].url.contains("limit=120"));
        assert!(urls[0].url.contains("api.artic.edu"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = AicImageSource::new();
        let json = r#"{
            "config": {
                "iiif_url": "https://www.artic.edu/iiif/2"
            },
            "data": [
                {
                    "id": 123,
                    "title": "Starry Night",
                    "image_id": "abc-123-def",
                    "is_public_domain": true
                },
                {
                    "id": 456,
                    "title": "Sunflowers",
                    "image_id": "xyz-789-uvw",
                    "is_public_domain": true
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "abc-123-def");
        assert_eq!(results[0].description, "Starry Night");
        assert!(results[0].candidate_download_urls[0].contains("abc-123-def"));
        assert!(results[0].candidate_download_urls[0].contains("full/843,/0/default.jpg"));
        assert_eq!(results[1].identifier, "xyz-789-uvw");
    }

    #[test]
    fn test_parse_skips_empty_image_id() {
        let source = AicImageSource::new();
        let json = r#"{
            "config": {
                "iiif_url": "https://www.artic.edu/iiif/2"
            },
            "data": [
                {
                    "id": 789,
                    "title": "No Image",
                    "image_id": null
                },
                {
                    "id": 999,
                    "title": "Empty Image ID",
                    "image_id": ""
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_uses_default_iiif_url() {
        let source = AicImageSource::new();
        let json = r#"{
            "data": [
                {
                    "id": 111,
                    "title": "Test",
                    "image_id": "test-id"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].candidate_download_urls[0].starts_with("https://www.artic.edu/iiif/2/"));
    }

    #[test]
    fn test_parse_empty_results() {
        let source = AicImageSource::new();
        let json = r#"{"config": {"iiif_url": "https://www.artic.edu/iiif/2"}, "data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
