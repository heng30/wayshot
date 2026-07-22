//! Freenaturestock image search source.
//!
//! Replaces Python's `FreeNatureStockImageClient`. Uses HTML scraping
//! with CSS selectors to extract image URLs from search result pages.

use reqwest::header::{HeaderMap, USER_AGENT};
use scraper::{Html, Selector};

use crate::client::ImageSource;
use crate::error::Result;
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Freenaturestock search page.
const PAGE_SIZE: usize = 40;

/// Freenaturestock image search source.
pub struct FreenaturestockImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl FreenaturestockImageSource {
    /// Create a new Freenaturestock image source.
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

    /// Resolve a potentially relative URL against the base URL.
    fn resolve_url(base_url: &str, url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if url.starts_with("//") {
            format!("https:{}", url)
        } else if url.starts_with('/') {
            format!("{}{}", base_url.trim_end_matches('/'), url)
        } else {
            format!("{}/{}", base_url.trim_end_matches('/'), url)
        }
    }
}

impl Default for FreenaturestockImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for FreenaturestockImageSource {
    fn source_name(&self) -> &str {
        "freenaturestock"
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
        let num_pages = ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let url = format!(
                    "https://freenaturestock.com/page/{}/?s={}",
                    pn,
                    urlencoding::encode(keyword),
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let document = Html::parse_document(body);
        let selector = Selector::parse("#photo-previews img[src]").unwrap();

        let base_url = "https://freenaturestock.com/";
        let mut image_infos = Vec::new();

        for element in document.select(&selector) {
            if let Some(src) = element.value().attr("src") {
                let url = Self::resolve_url(base_url, src);
                if url.starts_with("http") {
                    image_infos.push(ImageInfo::with_identifier(
                        self.source_name(),
                        vec![url.clone()],
                        url,
                    ));
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
        let source = FreenaturestockImageSource::new();
        let params = SearchParams {
            search_limits: 80,
            ..Default::default()
        };
        let urls = source.construct_search_urls("forest", &params, &Filters::new());
        // 80 * 1.2 / 40 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("page/0"));
        assert!(urls[0].url.contains("s=forest"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = FreenaturestockImageSource::new();
        let html = r#"
        <div id="photo-previews">
            <img src="https://freenaturestock.com/wp-content/uploads/photo1.jpg" />
            <img src="https://freenaturestock.com/wp-content/uploads/photo2.jpg" />
        </div>
        "#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "freenaturestock");
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://freenaturestock.com/wp-content/uploads/photo1.jpg"
        );
    }

    #[test]
    fn test_parse_search_result_relative_url() {
        let source = FreenaturestockImageSource::new();
        let html = r#"
        <div id="photo-previews">
            <img src="/wp-content/uploads/photo1.jpg" />
        </div>
        "#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://freenaturestock.com/wp-content/uploads/photo1.jpg"
        );
    }

    #[test]
    fn test_parse_empty_results() {
        let source = FreenaturestockImageSource::new();
        let html = r#"<div id="photo-previews"></div>"#;
        let results = source.parse_search_result(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_resolve_url() {
        assert_eq!(
            FreenaturestockImageSource::resolve_url("https://freenaturestock.com/", "https://example.com/img.jpg"),
            "https://example.com/img.jpg"
        );
        assert_eq!(
            FreenaturestockImageSource::resolve_url("https://freenaturestock.com/", "//cdn.example.com/img.jpg"),
            "https://cdn.example.com/img.jpg"
        );
        assert_eq!(
            FreenaturestockImageSource::resolve_url("https://freenaturestock.com/", "/uploads/img.jpg"),
            "https://freenaturestock.com/uploads/img.jpg"
        );
    }
}
