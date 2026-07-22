//! Foodiesfeed image search source.
//!
//! Uses the Foodiesfeed hybrid-photos API to search for food-related images.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde::Deserialize;

use crate::client::ImageSource;
use crate::error::Result;
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Foodiesfeed image search source.
pub struct FoodiesfeedImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl FoodiesfeedImageSource {
    /// Create a new Foodiesfeed image source.
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

impl Default for FoodiesfeedImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for FoodiesfeedImageSource {
    fn source_name(&self) -> &str {
        "foodiesfeed"
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
        _filters: &Filters,
    ) -> Vec<SearchUrl> {
        let page_size = 24;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let page = pn + 1;
                let istock_offset = pn * page_size;
                let total_loaded = (pn + 1) * page_size;
                SearchUrl::new(format!(
                    "https://www.foodiesfeed.com/api/hybrid-photos?page={}&limit={}&locale=zh&sort=relevance&requireTagMatch=false&apiLocation=hybrid-search&localExhausted=true&istockOffset={}&totalLoaded={}&searchQuery={}",
                    page,
                    page_size,
                    istock_offset,
                    total_loaded,
                    urlencoding::encode(keyword),
                ))
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let response: FoodiesfeedResponse = serde_json::from_str(body).unwrap_or(FoodiesfeedResponse {
            photos: None,
        });

        let photos = response.photos.unwrap_or_default();

        let image_infos: Vec<ImageInfo> = photos
            .into_iter()
            .filter_map(|photo| {
                let mut candidates = Vec::new();
                if let Some(url) = &photo.master_url {
                    if url.starts_with("http") {
                        candidates.push(url.clone());
                    }
                }
                if let Some(url) = &photo.webp_url {
                    if url.starts_with("http") && !candidates.contains(&url.clone()) {
                        candidates.push(url.clone());
                    }
                }
                if let Some(url) = &photo.thumbnail_url {
                    if url.starts_with("http") && !candidates.contains(&url.clone()) {
                        candidates.push(url.clone());
                    }
                }

                if candidates.is_empty() {
                    return None;
                }

                let identifier = photo
                    .id
                    .unwrap_or_else(|| candidates[0].clone());

                Some(ImageInfo::with_identifier(
                    self.source_name(),
                    candidates,
                    identifier,
                ))
            })
            .collect();

        Ok(image_infos)
    }
}

/// Foodiesfeed API response.
#[derive(Debug, Deserialize)]
struct FoodiesfeedResponse {
    #[serde(default)]
    photos: Option<Vec<FoodiesfeedPhoto>>,
}

/// A single photo from the Foodiesfeed API.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FoodiesfeedPhoto {
    id: Option<String>,
    master_url: Option<String>,
    webp_url: Option<String>,
    thumbnail_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = FoodiesfeedImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("pasta", &params, &Filters::new());
        assert_eq!(urls.len(), 3); // 50 * 1.2 / 24 ≈ 3 pages
        assert!(urls[0].url.contains("searchQuery=pasta"));
        assert!(urls[0].url.contains("page=1"));
        assert!(urls[0].url.contains("limit=24"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = FoodiesfeedImageSource::new();
        let json = r#"{
            "photos": [
                {
                    "id": "istock-123",
                    "master_url": "https://example.com/photo1.jpg",
                    "webp_url": "https://example.com/photo1.webp",
                    "thumbnail_url": "https://example.com/photo1_thumb.jpg"
                },
                {
                    "id": "istock-456",
                    "master_url": "https://example.com/photo2.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://example.com/photo1.jpg"
        );
    }

    #[test]
    fn test_parse_empty_results() {
        let source = FoodiesfeedImageSource::new();
        let json = r#"{"photos": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
