//! Pixabay image search source.
//!
//! Replaces Python's `PixabayImageClient`. Uses the Pixabay REST API
//! with JSON response parsing. Multiple API keys are rotated across
//! pages to distribute load.

use reqwest::header::{HeaderMap, ACCEPT_LANGUAGE, REFERER, UPGRADE_INSECURE_REQUESTS, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::filter::{Filter, FilterRule};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Pixabay API keys for rotation across pages.
const CANDIDATE_API_KEYS: &[&str] = &[
    "52293499-20edab5a9aeb2872ddc6cf68d",
    "51464414-1d83eb06bfdf3164b71156c0d",
    "50096047-8bb459140d4c19e045f4f2381",
    "35428194-f806941a429b19ee5838722ec",
    "43843784-ca8a7d4eb022dffa63faad957",
    "34748321-56ec586673804760cca13f7f6",
    "22850428-9964a4ca16315545d67c15abc",
    "20524560-a948ec896d1e8c0b8ba1135a6",
    "20583871-24538aa0638807f136238470d",
    "34787804-1aefa27f7d66275b11fe28ff3",
    "15089766-5bf9896a3416c7dcc335047dc",
    "47820586-1bbcb8dfd700ccd5c12e5d9e1",
];

/// Number of results per Pixabay API request.
const PAGE_SIZE: usize = 20;

/// Pixabay image search source.
pub struct PixabayImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl PixabayImageSource {
    /// Create a new Pixabay image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            ACCEPT_LANGUAGE,
            "zh-CN,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2"
                .parse()
                .unwrap(),
        );
        search_headers.insert(REFERER, "keep-alive".parse().unwrap());
        search_headers.insert(UPGRADE_INSECURE_REQUESTS, "1".parse().unwrap());
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

impl Default for PixabayImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for PixabayImageSource {
    fn source_name(&self) -> &str {
        "pixabay"
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
        let num_pages = ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        let filter_str = self
            .build_filter()
            .apply(filters, "&")
            .unwrap_or_default();

        (0..num_pages)
            .map(|pn| {
                let api_key = CANDIDATE_API_KEYS[pn % CANDIDATE_API_KEYS.len()];
                let mut url = format!(
                    "https://pixabay.com/api/?key={}&q={}&per_page={}&page={}",
                    api_key,
                    urlencoding::encode(keyword),
                    PAGE_SIZE,
                    pn + 1,
                );
                if !filter_str.is_empty() {
                    url.push_str(&format!("&{}", filter_str));
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

        let hits = search_result.get("hits").and_then(|v| v.as_array());
        if let Some(items) = hits {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                // Try URLs in order of quality: fullHDURL > imageURL > largeImageURL > webformatURL > previewURL
                let mut candidate_urls = Vec::new();
                for key in &["fullHDURL", "imageURL", "largeImageURL", "webformatURL", "previewURL"] {
                    if let Some(url) = item.get(*key).and_then(|v| v.as_str()) {
                        if url.starts_with("http") {
                            candidate_urls.push(url.to_string());
                        }
                    }
                }

                if candidate_urls.is_empty() {
                    continue;
                }

                let identifier = item
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| candidate_urls[0].clone());

                image_infos.push(ImageInfo::with_identifier(
                    self.source_name(),
                    candidate_urls,
                    identifier,
                ));
            }
        }

        Ok(image_infos)
    }

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // Language filter
        f.add_rule(FilterRule::with_string_choices(
            "lang",
            vec![
                "cs", "da", "de", "en", "es", "fr", "id", "it", "hu", "nl",
                "no", "pl", "pt", "ro", "sk", "fi", "sv", "tr", "vi", "th",
                "bg", "ru", "el", "ja", "ko", "zh",
            ],
            |v| format!("lang={}", v),
        ));

        // Image type filter
        f.add_rule(FilterRule::with_string_choices(
            "image_type",
            vec!["all", "photo", "illustration", "vector"],
            |v| format!("image_type={}", v),
        ));

        // Orientation filter
        f.add_rule(FilterRule::with_string_choices(
            "orientation",
            vec!["all", "horizontal", "vertical"],
            |v| format!("orientation={}", v),
        ));

        // Category filter
        f.add_rule(FilterRule::with_string_choices(
            "category",
            vec![
                "backgrounds", "fashion", "nature", "science", "education",
                "feelings", "health", "people", "religion", "places",
                "animals", "industry", "computer", "food", "sports",
                "transportation", "travel", "buildings", "business", "music",
            ],
            |v| format!("category={}", v),
        ));

        // Order filter
        f.add_rule(FilterRule::with_string_choices(
            "order",
            vec!["popular", "latest"],
            |v| format!("order={}", v),
        ));

        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FilterValue;

    #[test]
    fn test_construct_search_urls() {
        let source = PixabayImageSource::new();
        let params = SearchParams {
            search_limits: 40,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 40 * 1.2 / 20 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[1].url.contains("page=2"));
        // API key rotation
        assert!(urls[0].url.contains(CANDIDATE_API_KEYS[0]));
        assert!(urls[1].url.contains(CANDIDATE_API_KEYS[1]));
    }

    #[test]
    fn test_parse_search_result() {
        let source = PixabayImageSource::new();
        let json = r#"{
            "hits": [
                {
                    "id": 123,
                    "fullHDURL": "https://pixabay.com/full/123.jpg",
                    "largeImageURL": "https://pixabay.com/large/123.jpg",
                    "webformatURL": "https://pixabay.com/web/123.jpg",
                    "previewURL": "https://pixabay.com/preview/123.jpg"
                },
                {
                    "id": 456,
                    "largeImageURL": "https://pixabay.com/large/456.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "123");
        // fullHDURL should be first candidate
        assert_eq!(results[0].candidate_download_urls[0], "https://pixabay.com/full/123.jpg");
        assert_eq!(results[0].candidate_download_urls.len(), 4);
        assert_eq!(results[1].identifier, "456");
        assert_eq!(results[1].candidate_download_urls.len(), 1);
    }

    #[test]
    fn test_parse_empty_results() {
        let source = PixabayImageSource::new();
        let json = r#"{"hits": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_pixabay_filter_image_type() {
        let source = PixabayImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("image_type".to_string(), FilterValue::from("photo"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "image_type=photo");
    }

    #[test]
    fn test_pixabay_filter_order() {
        let source = PixabayImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("order".to_string(), FilterValue::from("latest"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "order=latest");
    }
}
