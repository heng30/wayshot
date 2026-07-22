//! Google image search source.
//!
//! Uses the Google Custom Search JSON API to search for images.
//! This avoids the need for JavaScript rendering that the HTML-based
//! Google Images interface requires.
//!
//! # API Keys
//!
//! This source uses Google Custom Search API keys (same approach as the
//! Python `imagedl` project). The free tier allows 100 queries/day.
//! Multiple API key/cx pairs are rotated to increase the daily quota.
//!
//! # Proxy
//!
//! If `google.com` is unreachable from your network, configure a proxy
//! via `ImageClientBuilder::proxy()` (supports socks5, http, https).

use reqwest::header::{HeaderMap, USER_AGENT};
use serde::Deserialize;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::filter::{Filter, FilterRule};
use crate::types::{FilterValue, Filters, ImageInfo, SearchParams, SearchUrl};

/// Google Custom Search API key and cx pairs.
///
/// These are the same keys used by the Python `imagedl` project.
/// Multiple pairs are provided for rotation to increase daily quota.
const CSE_CREDENTIALS: &[(&str, &str)] = &[
    ("AIzaSyCGyqf36D5k3QghaZLhAqb1R2OUtRFraF8", "0d386b282da5209ea"),
    ("AIzaSyD4dFGSan50nEmXh2Jnm4l6JHCAgEATWJc", "495179597de2e4ab6"),
    ("AIzaSyBRlama1N7tiW0yVq45CrqCx9hyFrESmIs", "144af1a5b59944a2b"),
    ("AIzaSyB7CcF4xiZqKE3yAmjBDZct4_HHs27gL7Y", "d7e74b48d90e7441c"),
];

/// Maximum results per CSE API request (API limit is 10).
const CSE_PAGE_SIZE: usize = 10;

/// Google image search source using the Custom Search JSON API.
pub struct GoogleImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl GoogleImageSource {
    /// Create a new Google image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
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
}

impl Default for GoogleImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for GoogleImageSource {
    fn source_name(&self) -> &str {
        "google"
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
        let num_pages = ((params.search_limits as f64 / CSE_PAGE_SIZE as f64).ceil()) as usize;

        let mut filters_copy = filters.clone();
        let language = filters_copy.remove("language");

        // Extract filter values and map to CSE API parameters
        let filter_type = filters_copy.remove("type");
        let filter_color = filters_copy.remove("color");
        let filter_size = filters_copy.remove("size");

        (0..num_pages)
            .map(|pn| {
                // Rotate API credentials across pages
                let cred_idx = pn % CSE_CREDENTIALS.len();
                let (api_key, cx) = CSE_CREDENTIALS[cred_idx];

                let mut url_params = vec![
                    format!("q={}", urlencoding::encode(keyword)),
                    format!("key={}", api_key),
                    format!("cx={}", cx),
                    "searchType=image".to_string(),
                    format!("num={}", CSE_PAGE_SIZE),
                    format!("start={}", pn * CSE_PAGE_SIZE + 1),
                ];

                // Apply CSE-specific filter parameters
                if let Some(FilterValue::String(v)) = &filter_type {
                    let img_type = match v.as_str() {
                        "photo" => "photo",
                        "face" => "face",
                        "clipart" => "clipart",
                        "linedrawing" => "lineart",
                        "animated" => "animated",
                        _ => "photo",
                    };
                    url_params.push(format!("imgType={}", img_type));
                }

                if let Some(FilterValue::String(v)) = &filter_color {
                    let color_type = match v.as_str() {
                        "color" => "color",
                        "blackandwhite" => "gray",
                        _ => "color",
                    };
                    url_params.push(format!("imgColorType={}", color_type));
                }

                if let Some(FilterValue::String(v)) = &filter_size {
                    let img_size = match v.as_str() {
                        "large" => "xlarge",
                        "medium" => "medium",
                        "small" => "icon",
                        _ => "medium",
                    };
                    url_params.push(format!("imgSize={}", img_size));
                }

                if let Some(lang) = &language
                    && let Some(s) = lang.as_str()
                {
                    url_params.push(format!("lr=lang_{}", s));
                }

                SearchUrl::new(format!(
                    "https://www.googleapis.com/customsearch/v1?{}",
                    url_params.join("&")
                ))
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        // The CSE API returns JSON directly
        let response: CseResponse = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("Failed to parse Google CSE response: {}", e),
            }
        })?;

        // Check for API error
        if let Some(err) = &response.error {
            return Err(ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!(
                    "Google CSE API error: {} (code: {})",
                    err.message, err.code
                ),
            });
        }

        let items = response.items.unwrap_or_default();

        let image_infos: Vec<ImageInfo> = items
            .into_iter()
            .filter_map(|item| {
                let link = item.link?;
                if !link.starts_with("http://") && !link.starts_with("https://") {
                    return None;
                }

                let mut candidates = vec![link.clone()];
                // Add thumbnail as fallback candidate
                if let Some(ref thumb) = item.image.thumbnail_link {
                    if thumb.starts_with("http://") || thumb.starts_with("https://") {
                        candidates.push(thumb.clone());
                    }
                }

                let mut info = ImageInfo::with_identifier(
                    self.source_name(),
                    candidates,
                    link.clone(),
                );

                // Store metadata in description
                if let Some(title) = item.title {
                    info.description = title;
                }

                Some(info)
            })
            .collect();

        Ok(image_infos)
    }

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // Type filter — maps to CSE imgType parameter
        f.add_rule(FilterRule::with_string_choices(
            "type",
            vec!["photo", "face", "clipart", "linedrawing", "animated"],
            |v| match v {
                "photo" => "photo".to_string(),
                "face" => "face".to_string(),
                "clipart" => "clipart".to_string(),
                "linedrawing" => "lineart".to_string(),
                "animated" => "animated".to_string(),
                _ => v.to_string(),
            },
        ));

        // Color filter — maps to CSE imgColorType parameter
        f.add_rule(FilterRule::with_string_choices(
            "color",
            vec!["color", "blackandwhite"],
            |v| match v {
                "color" => "color".to_string(),
                "blackandwhite" => "gray".to_string(),
                _ => "color".to_string(),
            },
        ));

        // Size filter — maps to CSE imgSize parameter
        f.add_rule(FilterRule::with_string_choices(
            "size",
            vec!["large", "medium", "small"],
            |v| match v {
                "large" => "xlarge".to_string(),
                "medium" => "medium".to_string(),
                "small" => "icon".to_string(),
                _ => "medium".to_string(),
            },
        ));

        // License filter
        let license_code = [
            ("noncommercial", "f"),
            ("commercial", "fc"),
            ("noncommercial,modify", "fm"),
            ("commercial,modify", "fmc"),
        ];
        let license_choices: Vec<&str> = license_code.iter().map(|(k, _)| *k).collect();
        let license_map: std::collections::HashMap<&str, &str> =
            license_code.into_iter().collect();
        f.add_rule(FilterRule::with_string_choices("license", license_choices, move |v| {
            format!("sur:{}", license_map.get(v).copied().unwrap_or("f"))
        }));

        // Date filter
        f.add_rule(FilterRule::with_string_choices(
            "date",
            vec!["anytime", "pastday", "pastweek", "pastmonth", "pastyear"],
            |v| match v {
                "anytime" => String::new(),
                "pastday" => "qdr:d".to_string(),
                "pastweek" => "qdr:w".to_string(),
                "pastmonth" => "qdr:m".to_string(),
                "pastyear" => "qdr:y".to_string(),
                _ => String::new(),
            },
        ));

        f
    }
}

/// Google Custom Search JSON API response structure.
#[derive(Debug, Deserialize)]
struct CseResponse {
    #[serde(default)]
    items: Option<Vec<CseItem>>,
    /// If present, the API returned an error.
    #[serde(default)]
    error: Option<CseError>,
}

/// A single image result from the CSE API.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct CseItem {
    title: Option<String>,
    link: Option<String>,
    snippet: Option<String>,
    mime: Option<String>,
    image: CseImage,
}

/// Image metadata from the CSE API.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct CseImage {
    #[serde(default)]
    thumbnail_link: Option<String>,
    #[serde(default)]
    height: Option<u64>,
    #[serde(default)]
    width: Option<u64>,
    #[serde(default)]
    byte_size: Option<u64>,
}

/// Google CSE API error details.
#[derive(Debug, Deserialize)]
struct CseError {
    code: u64,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FilterValue;

    #[test]
    fn test_construct_search_urls() {
        let source = GoogleImageSource::new();
        let params = SearchParams {
            search_limits: 20,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        assert_eq!(urls.len(), 2); // 20 results / 10 per page = 2 pages
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("searchType=image"));
        assert!(urls[0].url.contains("customsearch/v1"));
    }

    #[test]
    fn test_parse_search_result_with_json() {
        let source = GoogleImageSource::new();
        let json = r#"{
            "items": [
                {
                    "title": "Cat photo",
                    "link": "https://example.com/cat1.jpg",
                    "snippet": "A cute cat",
                    "mime": "image/jpeg",
                    "image": {
                        "thumbnailLink": "https://example.com/thumb1.jpg",
                        "height": 800,
                        "width": 600,
                        "byteSize": 123456
                    }
                },
                {
                    "title": "Another cat",
                    "link": "https://example.com/cat2.png",
                    "snippet": "Another cute cat",
                    "mime": "image/png",
                    "image": {
                        "thumbnailLink": "https://example.com/thumb2.png",
                        "height": 1024,
                        "width": 768,
                        "byteSize": 234567
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://example.com/cat1.jpg"
        );
        assert_eq!(results[0].description, "Cat photo");
        // Thumbnail should be second candidate
        assert_eq!(
            results[0].candidate_download_urls[1],
            "https://example.com/thumb1.jpg"
        );
    }

    #[test]
    fn test_parse_search_result_empty() {
        let source = GoogleImageSource::new();
        let json = r#"{}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_result_api_error() {
        let source = GoogleImageSource::new();
        let json = r#"{
            "error": {
                "code": 403,
                "message": "Daily limit exceeded"
            }
        }"#;
        let result = source.parse_search_result(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ImageDlError::Parse { origin, reason } => {
                assert_eq!(origin, "google");
                assert!(reason.contains("Daily limit exceeded"));
            }
            _ => panic!("Expected Parse error, got {:?}", err),
        }
    }

    #[test]
    fn test_google_filter_type() {
        let source = GoogleImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("type".to_string(), FilterValue::from("clipart"));
        let result = filter.apply(&options, ",").unwrap();
        assert_eq!(result, "clipart");
    }

    #[test]
    fn test_google_filter_color() {
        let source = GoogleImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("color".to_string(), FilterValue::from("blackandwhite"));
        let result = filter.apply(&options, ",").unwrap();
        assert_eq!(result, "gray");
    }

    #[test]
    fn test_google_filter_size() {
        let source = GoogleImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("size".to_string(), FilterValue::from("large"));
        let result = filter.apply(&options, ",").unwrap();
        assert_eq!(result, "xlarge");
    }
}
