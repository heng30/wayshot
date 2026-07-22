//! Dimtown image search source.
//!
//! Replaces Python's `DimTownImageClient`. Uses a 2-step scraping process:
//! 1. Search result pages are scraped for post links matching `dimtown.com/\d+.html`
//! 2. Each post page is visited to extract image URLs
//!
//! The `search()` method is overridden to implement the 2-step process.

use regex::Regex;
use reqwest::header::{HeaderMap, USER_AGENT};
use scraper::{Html, Selector};

use crate::client::http::HttpClient;
use crate::client::ImageSource;
use crate::error::Result;
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of posts per Dimtown search page.
const PAGE_SIZE: usize = 25;

/// Dimtown image search source (2-step scraping).
pub struct DimtownImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
    post_url_re: Regex,
}

impl DimtownImageSource {
    /// Create a new Dimtown image source.
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

        let post_url_re =
            Regex::new(r"^https?://dimtown\.com/\d+\.html$").expect("Invalid Dimtown post URL regex");

        Self {
            search_headers,
            download_headers,
            post_url_re,
        }
    }

    /// Check if a URL has an image file extension.
    fn is_image_url(url: &str) -> bool {
        let path = url.split('?').next().unwrap_or(url).to_lowercase();
        path.ends_with(".jpg")
            || path.ends_with(".jpeg")
            || path.ends_with(".png")
            || path.ends_with(".gif")
            || path.ends_with(".webp")
            || path.ends_with(".bmp")
    }

    /// Extract post URLs from a search result listing page.
    fn extract_post_urls(&self, body: &str) -> Vec<String> {
        let document = Html::parse_document(body);
        let selector = Selector::parse("ul#index_ajax_list a[target=\"_blank\"][href]")
            .unwrap();

        let mut seen = std::collections::HashSet::new();
        let mut post_urls = Vec::new();

        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                let url = href.trim();
                if self.post_url_re.is_match(url) && !seen.contains(url) {
                    seen.insert(url.to_string());
                    post_urls.push(url.to_string());
                }
            }
        }

        post_urls
    }
}

impl Default for DimtownImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for DimtownImageSource {
    fn source_name(&self) -> &str {
        "dimtown"
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
                    "https://dimtown.com/page/{}?s={}",
                    pn + 1,
                    urlencoding::encode(keyword),
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let document = Html::parse_document(body);

        let content_selector = Selector::parse("#content .content_left").unwrap();
        let mut image_infos = Vec::new();
        let mut seen = std::collections::HashSet::new();

        if let Some(box_el) = document.select(&content_selector).next() {
            // Collect URLs from both <a href> and <img src> elements
            let a_selector = Selector::parse("a[href]").unwrap();
            let img_selector = Selector::parse("img[src]").unwrap();

            let mut urls = Vec::new();

            for a in box_el.select(&a_selector) {
                if let Some(href) = a.value().attr("href") {
                    urls.push(href.to_string());
                }
            }

            for img in box_el.select(&img_selector) {
                if let Some(src) = img.value().attr("src") {
                    urls.push(src.to_string());
                }
            }

            for url in urls {
                if Self::is_image_url(&url) && !seen.contains(&url) {
                    seen.insert(url.clone());
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

    /// Override search for the 2-step process:
    /// 1. Fetch listing pages and extract post URLs
    /// 2. Fetch each post page and extract image URLs
    async fn search(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
        http: &HttpClient,
    ) -> Result<Vec<ImageInfo>> {
        let listing_urls = self.construct_search_urls(keyword, params, filters);
        let search_headers = self.search_headers();

        log::info!(
            "[{}] Starting 2-step search (step 1: find post URLs, {} listing pages)",
            self.source_name(),
            listing_urls.len()
        );

        // Step 1: Fetch listing pages and collect post URLs
        let mut post_urls = Vec::new();
        for search_url in &listing_urls {
            match http.get_text(&search_url.url, search_headers.clone()).await {
                Ok(body) => {
                    let found = self.extract_post_urls(&body);
                    log::debug!(
                        "Extracted {} post URLs from listing page: {}",
                        found.len(),
                        search_url.url
                    );
                    post_urls.extend(found);
                }
                Err(e) => {
                    log::warn!("Failed to fetch listing page: {} - {}", search_url.url, e);
                    break;
                }
            }
        }

        log::info!(
            "[{}] Step 1 complete ({} posts), starting step 2",
            self.source_name(),
            post_urls.len()
        );

        // Step 2: Fetch each post page and extract images
        let mut all_infos = Vec::new();
        for post_url in &post_urls {
            match http.get_text(post_url, search_headers.clone()).await {
                Ok(body) => {
                    match self.parse_search_result(&body) {
                        Ok(infos) => {
                            log::debug!("Extracted {} images from post: {}", infos.len(), post_url);
                            all_infos.extend(infos);
                        }
                        Err(e) => {
                            log::warn!("Failed to parse post page: {} - {}", post_url, e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to fetch post page: {} - {}", post_url, e);
                }
            }
        }

        // Deduplicate by identifier
        let mut seen = std::collections::HashSet::new();
        let infos: Vec<ImageInfo> = all_infos
            .into_iter()
            .filter(|info| seen.insert(info.identifier.clone()))
            .collect();

        // Assign file paths
        let infos = crate::client::assign_file_paths(
            infos,
            self.source_name(),
            &params.work_dir,
            keyword,
        );

        log::info!(
            "[{}] 2-step search completed ({} results)",
            self.source_name(),
            infos.len()
        );

        Ok(infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_search_urls() {
        let source = DimtownImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 50 * 1.2 / 25 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("page/1"));
        assert!(urls[0].url.contains("s=cats"));
    }

    #[test]
    fn test_extract_post_urls() {
        let source = DimtownImageSource::new();
        let html = r#"
        <ul id="index_ajax_list">
            <li><a target="_blank" href="https://dimtown.com/12345.html">Post 1</a></li>
            <li><a target="_blank" href="https://dimtown.com/67890.html">Post 2</a></li>
            <li><a target="_blank" href="https://other.com/not-match.html">Not a match</a></li>
        </ul>
        "#;
        let urls = source.extract_post_urls(html);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://dimtown.com/12345.html");
        assert_eq!(urls[1], "https://dimtown.com/67890.html");
    }

    #[test]
    fn test_parse_search_result() {
        let source = DimtownImageSource::new();
        let html = r#"
        <div id="content">
            <div class="content_left">
                <a href="https://dimtown.com/uploads/image1.jpg">Image 1</a>
                <img src="https://dimtown.com/uploads/image2.png" />
                <a href="https://dimtown.com/page.html">Not an image</a>
            </div>
        </div>
        "#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "dimtown");
    }

    #[test]
    fn test_is_image_url() {
        assert!(DimtownImageSource::is_image_url("https://example.com/photo.jpg"));
        assert!(DimtownImageSource::is_image_url("https://example.com/photo.PNG?size=large"));
        assert!(DimtownImageSource::is_image_url("https://example.com/photo.webp"));
        assert!(!DimtownImageSource::is_image_url("https://example.com/page.html"));
        assert!(!DimtownImageSource::is_image_url("https://example.com/document.pdf"));
    }
}
