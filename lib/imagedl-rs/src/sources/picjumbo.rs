//! Picjumbo image search source.
//!
//! Replaces Python's `PicJumboImageClient`. Uses HTML scraping
//! with CSS selectors to extract image URLs from search result pages.
//! Parses srcset attributes and sorts by width (largest first).

use reqwest::header::{HeaderMap, USER_AGENT};
use scraper::{Html, Selector};

use crate::client::ImageSource;
use crate::error::Result;
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Picjumbo search page.
const PAGE_SIZE: usize = 20;

/// Picjumbo image search source.
pub struct PicjumboImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl PicjumboImageSource {
    /// Create a new Picjumbo image source.
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

    /// Parse a srcset attribute value into (url, width) pairs, sorted by width descending.
    fn parse_srcset(srcset: &str) -> Vec<(String, usize)> {
        let mut entries = Vec::new();
        for part in srcset.split(',') {
            let tokens: Vec<&str> = part.trim().split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }
            let mut url = tokens[0].to_string();
            // Fix protocol-relative URLs
            if url.starts_with("//") {
                url = format!("https:{}", url);
            }
            let width = if tokens.len() > 1 && tokens[1].ends_with('w') {
                tokens[1][..tokens[1].len() - 1]
                    .parse::<usize>()
                    .unwrap_or(0)
            } else {
                0
            };
            entries.push((url, width));
        }
        // Sort by width descending (largest images first)
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries
    }
}

impl Default for PicjumboImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for PicjumboImageSource {
    fn source_name(&self) -> &str {
        "picjumbo"
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
                    "https://picjumbo.com/search/{}/page/{}/",
                    urlencoding::encode(keyword),
                    pn + 1,
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let document = Html::parse_document(body);
        let item_selector = Selector::parse("div.photo_item").unwrap();
        let img_selector = Selector::parse("img").unwrap();
        let h3_selector = Selector::parse("h3").unwrap();

        let mut image_infos = Vec::new();

        for item in document.select(&item_selector) {
            // Extract srcset from the img element
            let img = match item.select(&img_selector).next() {
                Some(img) => img,
                None => continue,
            };

            let srcset = match img.value().attr("srcset") {
                Some(s) => s,
                None => continue,
            };

            let parsed = Self::parse_srcset(srcset);
            let candidate_urls: Vec<String> = parsed
                .into_iter()
                .map(|(url, _)| url)
                .filter(|url| url.starts_with("http"))
                .collect();

            if candidate_urls.is_empty() {
                continue;
            }

            // Extract description from h3 if available
            let description = item
                .select(&h3_selector)
                .next()
                .and_then(|h3| h3.text().next())
                .map(|t| t.trim().to_string())
                .unwrap_or_default();

            let identifier = candidate_urls[0].clone();

            let mut info = ImageInfo::with_identifier(
                self.source_name(),
                candidate_urls,
                identifier,
            );
            info.description = description;
            image_infos.push(info);
        }

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = PicjumboImageSource::new();
        let params = SearchParams {
            search_limits: 40,
            ..Default::default()
        };
        let urls = source.construct_search_urls("nature", &params, &Filters::new());
        // 40 * 1.2 / 20 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("search/nature"));
        assert!(urls[0].url.contains("page/1"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = PicjumboImageSource::new();
        let html = r#"
        <div class="photo_query">
            <div class="photo_item">
                <img srcset="https://picjumbo.com/wp-content/uploads/photo1-1920x1280.jpg 1920w, https://picjumbo.com/wp-content/uploads/photo1-800x533.jpg 800w" />
                <h3>Sunset over mountains</h3>
            </div>
            <div class="photo_item">
                <img srcset="//picjumbo.com/wp-content/uploads/photo2-1600x1067.jpg 1600w, //picjumbo.com/wp-content/uploads/photo2-400x267.jpg 400w" />
                <h3>Ocean waves</h3>
            </div>
        </div>
        "#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "picjumbo");
        // Largest image should be first
        assert!(results[0].candidate_download_urls[0].contains("1920x1280"));
        assert_eq!(results[0].description, "Sunset over mountains");
        // Protocol-relative URL should be fixed
        assert!(results[1].candidate_download_urls[0].starts_with("https://"));
    }

    #[test]
    fn test_parse_empty_results() {
        let source = PicjumboImageSource::new();
        let html = r#"<div class="photo_query"></div>"#;
        let results = source.parse_search_result(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_srcset() {
        let parsed = PicjumboImageSource::parse_srcset(
            "https://picjumbo.com/photo-1920x1280.jpg 1920w, https://picjumbo.com/photo-800x533.jpg 800w",
        );
        assert_eq!(parsed.len(), 2);
        // Should be sorted by width descending
        assert_eq!(parsed[0].1, 1920);
        assert_eq!(parsed[1].1, 800);
    }

    #[test]
    fn test_parse_srcset_protocol_relative() {
        let parsed = PicjumboImageSource::parse_srcset(
            "//picjumbo.com/photo.jpg 1920w",
        );
        assert_eq!(parsed[0].0, "https://picjumbo.com/photo.jpg");
    }
}
