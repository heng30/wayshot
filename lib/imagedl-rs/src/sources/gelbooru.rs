//! Gelbooru image search source.
//!
//! Replaces Python's `GelbooruImageClient`. Uses a 2-step scraping process:
//! 1. Search result pages are scraped for thumbnail links
//! 2. Each post page is visited to extract the full-size image URL from `img#image`
//!
//! The `search()` method is overridden to implement the 2-step process.

use reqwest::header::{HeaderMap, REFERER, USER_AGENT};
use scraper::{Html, Selector};

use crate::client::http::HttpClient;
use crate::client::ImageSource;
use crate::error::Result;
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Gelbooru search page.
const PAGE_SIZE: usize = 42;

/// Gelbooru image search source (2-step scraping).
pub struct GelbooruImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl GelbooruImageSource {
    /// Create a new Gelbooru image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            REFERER,
            "https://gelbooru.com/index.php".parse().unwrap(),
        );
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            REFERER,
            "https://gelbooru.com/index.php".parse().unwrap(),
        );
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }

    /// Resolve a potentially relative URL against the Gelbooru base URL.
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

    /// Extract post URLs and thumbnail data from a search listing page.
    /// Returns a list of (post_url, thumbnail_src) tuples.
    fn extract_post_links(&self, body: &str) -> Vec<(String, Option<String>)> {
        let document = Html::parse_document(body);
        let container_selector =
            Selector::parse("div.thumbnail-container article.thumbnail-preview").unwrap();
        let a_selector = Selector::parse("a[href][id]").unwrap();
        let img_selector = Selector::parse("img").unwrap();

        let base_url = "https://gelbooru.com/";
        let mut posts = Vec::new();

        for container in document.select(&container_selector) {
            let a = match container.select(&a_selector).next() {
                Some(a) => a,
                None => continue,
            };

            let href = match a.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            let post_url = html_escape::decode_html_entities(href);
            let post_url = Self::resolve_url(base_url, &post_url);

            // Extract thumbnail src for fallback
            let thumb_src = container.select(&img_selector).next().and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("data-original"))
                    .or_else(|| img.value().attr("src"))
                    .map(|s| {
                        let decoded = html_escape::decode_html_entities(s);
                        Self::resolve_url(base_url, &decoded)
                    })
            });

            posts.push((post_url, thumb_src));
        }

        posts
    }

    /// Fetch a post page and extract the full-size image URL from `img#image`.
    async fn fetch_post_image(
        &self,
        post_url: &str,
        http: &HttpClient,
        headers: &HeaderMap,
    ) -> Option<String> {
        match http.get_text(post_url, headers.clone()).await {
            Ok(body) => {
                let document = Html::parse_document(&body);
                let img_selector = Selector::parse("img#image").unwrap();
                document
                    .select(&img_selector)
                    .next()
                    .and_then(|img| img.value().attr("src").map(|s| s.to_string()))
                    .filter(|url| url.starts_with("http"))
            }
            Err(e) => {
                log::warn!("Failed to fetch Gelbooru post page: {} - {}", post_url, e);
                None
            }
        }
    }
}

impl Default for GelbooruImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for GelbooruImageSource {
    fn source_name(&self) -> &str {
        "gelbooru"
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
                    "https://gelbooru.com/index.php?page=post&s=list&tags={}&pid={}",
                    urlencoding::encode(keyword),
                    pn * PAGE_SIZE,
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, _body: &str) -> Result<Vec<ImageInfo>> {
        // Not used in the 2-step process; the overridden search() handles everything.
        Ok(vec![])
    }

    /// Override search for the 2-step process:
    /// 1. Fetch listing pages and extract post URLs with thumbnails
    /// 2. Fetch each post page and extract the full-size image URL
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

        // Step 1: Fetch listing pages and collect post links
        let mut all_posts = Vec::new();
        for search_url in &listing_urls {
            match http.get_text(&search_url.url, search_headers.clone()).await {
                Ok(body) => {
                    let posts = self.extract_post_links(&body);
                    log::debug!(
                        "Extracted {} post links from listing page: {}",
                        posts.len(),
                        search_url.url
                    );
                    all_posts.extend(posts);
                }
                Err(e) => {
                    log::warn!("Failed to fetch listing page: {} - {}", search_url.url, e);
                }
            }
        }

        log::info!(
            "[{}] Step 1 complete ({} posts), starting step 2",
            self.source_name(),
            all_posts.len()
        );

        // Step 2: Fetch each post page and extract full-size images
        let mut all_infos = Vec::new();
        for (post_url, thumb_src) in &all_posts {
            let mut candidate_urls = Vec::new();

            // Try to get the full-size image from the post page
            if let Some(image_url) = self.fetch_post_image(post_url, http, &search_headers).await {
                candidate_urls.push(image_url);
            }

            // Add thumbnail as fallback
            if let Some(thumb) = thumb_src {
                if !candidate_urls.contains(thumb) {
                    candidate_urls.push(thumb.clone());
                }
            }

            if candidate_urls.is_empty() {
                continue;
            }

            let identifier = candidate_urls[0].clone();
            all_infos.push(ImageInfo::with_identifier(
                self.source_name(),
                candidate_urls,
                identifier,
            ));
        }

        // Deduplicate by identifier
        let infos = crate::client::dedup_by_identifier(all_infos);

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
        let source = GelbooruImageSource::new();
        let params = SearchParams {
            search_limits: 84,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cat_ears", &params, &Filters::new());
        // 84 * 1.2 / 42 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("tags=cat_ears"));
        assert!(urls[0].url.contains("pid=0"));
        assert!(urls[1].url.contains("pid=42"));
    }

    #[test]
    fn test_extract_post_links() {
        let source = GelbooruImageSource::new();
        let html = r#"
        <div class="thumbnail-container">
            <article class="thumbnail-preview">
                <a href="index.php?page=post&s=view&id=12345" id="p12345">
                    <img data-src="https://img.gelbooru.com/thumbnails/12345.jpg" />
                </a>
            </article>
            <article class="thumbnail-preview">
                <a href="index.php?page=post&s=view&id=67890" id="p67890">
                    <img src="https://img.gelbooru.com/thumbnails/67890.jpg" />
                </a>
            </article>
        </div>
        "#;
        let posts = source.extract_post_links(html);
        assert_eq!(posts.len(), 2);
        assert!(posts[0].0.contains("id=12345"));
        assert!(posts[1].0.contains("id=67890"));
    }

    #[test]
    fn test_resolve_url() {
        assert_eq!(
            GelbooruImageSource::resolve_url("https://gelbooru.com/", "https://img.gelbooru.com/img.jpg"),
            "https://img.gelbooru.com/img.jpg"
        );
        assert_eq!(
            GelbooruImageSource::resolve_url("https://gelbooru.com/", "//img.gelbooru.com/img.jpg"),
            "https://img.gelbooru.com/img.jpg"
        );
        assert_eq!(
            GelbooruImageSource::resolve_url("https://gelbooru.com/", "/index.php?page=post"),
            "https://gelbooru.com/index.php?page=post"
        );
    }
}
