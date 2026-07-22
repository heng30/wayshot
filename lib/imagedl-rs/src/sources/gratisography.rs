//! Gratisography image search source.
//!
//! Replaces Python's `GratisoGraphyImageClient`. Uses HTML scraping
//! with CSS selectors to extract image URLs from search result pages.
//! Supports multiple image URL sources: src, data-src, data-lazy-src,
//! srcset, and data-srcset.

use regex::Regex;
use reqwest::header::{HeaderMap, USER_AGENT};
use scraper::{Html, Selector};

use crate::client::ImageSource;
use crate::error::Result;
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Gratisography search page.
const PAGE_SIZE: usize = 10;

/// Gratisography image search source.
pub struct GratisographyImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
    size_re: Regex,
}

impl GratisographyImageSource {
    /// Create a new Gratisography image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let size_re = Regex::new(r"-(\d+)x(\d+)\.\w+")
            .expect("Invalid Gratisography size regex");

        Self {
            search_headers,
            download_headers,
            size_re,
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

    /// Score a URL by its image dimensions. Higher scores indicate larger images.
    fn score_url(&self, url: &str) -> usize {
        // Try to extract dimensions from the URL (e.g., -1920x1080.jpg)
        if let Some(caps) = self.size_re.captures(url) {
            if let (Some(w), Some(h)) = (caps.get(1), caps.get(2)) {
                if let (Ok(width), Ok(height)) = (w.as_str().parse::<usize>(), h.as_str().parse::<usize>()) {
                    return width * height;
                }
            }
        }
        // Try to extract width from query parameter (e.g., ?w=1920 or ?width=1920)
        let width_re = Regex::new(r"[?&](?:w|width)=(\d+)").unwrap();
        if let Some(caps) = width_re.captures(url) {
            if let Some(w) = caps.get(1) {
                if let Ok(width) = w.as_str().parse::<usize>() {
                    return width;
                }
            }
        }
        0
    }

    /// Parse a srcset attribute value into (url, width) pairs.
    fn parse_srcset(base_url: &str, srcset: &str) -> Vec<(String, usize)> {
        let mut entries = Vec::new();
        for part in srcset.split(',') {
            let tokens: Vec<&str> = part.trim().split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }
            let url = Self::resolve_url(base_url, tokens[0]);
            let width = if tokens.len() > 1 && tokens[1].ends_with('w') {
                tokens[1][..tokens[1].len() - 1]
                    .parse::<usize>()
                    .unwrap_or(0)
            } else {
                0
            };
            entries.push((url, width));
        }
        entries
    }
}

impl Default for GratisographyImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for GratisographyImageSource {
    fn source_name(&self) -> &str {
        "gratisography"
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
                    "https://gratisography.com/page/{}/?s={}",
                    pn + 1,
                    urlencoding::encode(keyword),
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let document = Html::parse_document(body);
        let article_selector =
            Selector::parse(".search-grid article[id^=\"single-photo-\"]").unwrap();
        let img_selector = Selector::parse("img").unwrap();

        let base_url = "https://gratisography.com";
        let mut image_infos = Vec::new();

        for article in document.select(&article_selector) {
            let mut url_scores: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();

            for img in article.select(&img_selector) {
                // Collect URLs from single-source attributes
                for attr in &["src", "data-src", "data-lazy-src"] {
                    if let Some(val) = img.value().attr(attr) {
                        let url = Self::resolve_url(base_url, val);
                        let score = self.score_url(&url);
                        url_scores
                            .entry(url)
                            .and_modify(|existing| {
                                if score > *existing {
                                    *existing = score;
                                }
                            })
                            .or_insert(score);
                    }
                }

                // Collect URLs from srcset attributes
                for attr in &["srcset", "data-srcset"] {
                    if let Some(srcset) = img.value().attr(attr) {
                        for (url, width) in Self::parse_srcset(base_url, srcset) {
                            let score = if width > 0 {
                                width
                            } else {
                                self.score_url(&url)
                            };
                            url_scores
                                .entry(url)
                                .and_modify(|existing| {
                                    if score > *existing {
                                        *existing = score;
                                    }
                                })
                                .or_insert(score);
                        }
                    }
                }
            }

            if url_scores.is_empty() {
                continue;
            }

            // Sort URLs by score descending (largest images first)
            let mut sorted: Vec<(String, usize)> = url_scores.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));

            let candidate_urls: Vec<String> = sorted.into_iter().map(|(url, _)| url).collect();
            let identifier = candidate_urls[0].clone();

            image_infos.push(ImageInfo::with_identifier(
                self.source_name(),
                candidate_urls,
                identifier,
            ));
        }

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = GratisographyImageSource::new();
        let params = SearchParams {
            search_limits: 20,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 20 * 1.2 / 10 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("page/1"));
        assert!(urls[0].url.contains("s=cats"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = GratisographyImageSource::new();
        let html = r#"
        <div class="search-grid">
            <article id="single-photo-1">
                <img src="https://gratisography.com/wp-content/uploads/2024/photo1-800x600.jpg"
                     srcset="https://gratisography.com/wp-content/uploads/2024/photo1-1920x1080.jpg 1920w,
                             https://gratisography.com/wp-content/uploads/2024/photo1-800x600.jpg 800w" />
            </article>
            <article id="single-photo-2">
                <img data-src="https://gratisography.com/wp-content/uploads/2024/photo2.jpg" />
            </article>
        </div>
        "#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "gratisography");
        // Should have candidate URLs from srcset
        assert!(!results[0].candidate_download_urls.is_empty());
    }

    #[test]
    fn test_parse_empty_results() {
        let source = GratisographyImageSource::new();
        let html = r#"<div class="search-grid"></div>"#;
        let results = source.parse_search_result(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_score_url() {
        let source = GratisographyImageSource::new();
        assert_eq!(source.score_url("https://example.com/photo-1920x1080.jpg"), 1920 * 1080);
        assert_eq!(source.score_url("https://example.com/photo-800x600.jpg"), 800 * 600);
        assert_eq!(source.score_url("https://example.com/photo.jpg?w=1920"), 1920);
        assert_eq!(source.score_url("https://example.com/photo.jpg"), 0);
    }

    #[test]
    fn test_parse_srcset() {
        let entries = GratisographyImageSource::parse_srcset(
            "https://gratisography.com",
            "https://gratisography.com/photo-1920x1080.jpg 1920w, https://gratisography.com/photo-800x600.jpg 800w",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "https://gratisography.com/photo-1920x1080.jpg");
        assert_eq!(entries[0].1, 1920);
        assert_eq!(entries[1].1, 800);
    }
}
