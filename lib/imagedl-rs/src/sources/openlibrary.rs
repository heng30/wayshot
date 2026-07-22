//! Open Library image search source.
//!
//! Replaces Python's `OpenLibraryImageClient`. Uses the Open Library
//! Search API with JSON response parsing.
//!
//! Cover images are constructed from cover IDs, edition keys, and ISBNs
//! using the Open Library Covers API.

use reqwest::header::{HeaderMap, ACCEPT, REFERER, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Open Library API requests.
const DEFAULT_PAGE_SIZE: usize = 100;

/// Open Library image search source.
pub struct OpenLibraryImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl OpenLibraryImageSource {
    /// Create a new Open Library image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            ACCEPT,
            "application/json".parse().unwrap(),
        );
        search_headers.insert(
            REFERER,
            "https://openlibrary.org/".parse().unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        download_headers.insert(
            REFERER,
            "https://openlibrary.org/".parse().unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }

    /// Build cover URLs for a given kind (id, olid, isbn) and value.
    ///
    /// Returns large and medium cover URLs.
    fn cover_urls(kind: &str, value: &str) -> Vec<String> {
        let value = value.trim();
        if value.is_empty() {
            return Vec::new();
        }
        vec![
            format!("https://covers.openlibrary.org/b/{}/{}-L.jpg?default=false", kind, value),
            format!("https://covers.openlibrary.org/b/{}/{}-M.jpg?default=false", kind, value),
        ]
    }
}

impl Default for OpenLibraryImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for OpenLibraryImageSource {
    fn source_name(&self) -> &str {
        "openlibrary"
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
        let base_url = "https://openlibrary.org/search.json?";
        let page_size = filters
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_PAGE_SIZE as i64) as usize;
        let page_size = page_size.clamp(1, 100);
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "{}q={}&limit={}&offset={}&fields=key,title,author_name,first_publish_year,cover_i,cover_edition_key,isbn,edition_key",
                    base_url,
                    urlencoding::encode(keyword),
                    page_size,
                    pn * page_size,
                );
                for (key, value) in filters {
                    if key != "limit" && key != "q" && key != "offset" && key != "fields"
                        && let Some(s) = value.as_str()
                    {
                        url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
                    }
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

        let docs = search_result.get("docs").and_then(|v| v.as_array());
        if let Some(items) = docs {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let mut candidate_urls = Vec::new();

                // Cover ID
                if let Some(cover_i) = item.get("cover_i").and_then(|v| v.as_i64()) {
                    candidate_urls.extend(Self::cover_urls("id", &cover_i.to_string()));
                }

                // Cover edition key (OLID)
                if let Some(olid) = item
                    .get("cover_edition_key")
                    .and_then(|v| v.as_str())
                {
                    candidate_urls.extend(Self::cover_urls("olid", olid));
                }

                // ISBN (up to 3)
                if let Some(isbns) = item.get("isbn").and_then(|v| v.as_array()) {
                    for isbn in isbns.iter().take(3) {
                        if let Some(isbn_str) = isbn.as_str() {
                            candidate_urls.extend(Self::cover_urls("isbn", isbn_str));
                        }
                    }
                }

                // Deduplicate while preserving order
                let mut seen = std::collections::HashSet::new();
                candidate_urls.retain(|url| seen.insert(url.clone()));

                if candidate_urls.is_empty() {
                    continue;
                }

                let identifier = item
                    .get("key")
                    .or_else(|| item.get("cover_edition_key"))
                    .or_else(|| item.get("cover_i"))
                    .and_then(|v| {
                        v.as_str()
                            .map(|s| s.to_string())
                            .or_else(|| v.as_i64().map(|n| n.to_string()))
                    })
                    .unwrap_or_else(|| candidate_urls[0].clone());

                image_infos.push(ImageInfo {
                    source: self.source_name().to_string(),
                    download_url: None,
                    candidate_download_urls: candidate_urls,
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
        let source = OpenLibraryImageSource::new();
        let params = SearchParams {
            search_limits: 200,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 200 * 1.2 / 100 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("offset=0"));
        assert!(urls[1].url.contains("offset=100"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = OpenLibraryImageSource::new();
        let json = r#"{
            "docs": [
                {
                    "key": "/works/OL123W",
                    "title": "The Cat in the Hat",
                    "cover_i": 12345,
                    "cover_edition_key": "OL678M",
                    "isbn": ["9780394800011", "039480001X"]
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "/works/OL123W");
        assert_eq!(results[0].description, "The Cat in the Hat");
        // Should have cover_i URLs (2) + olid URLs (2) + isbn URLs (4) = 8
        assert_eq!(results[0].candidate_download_urls.len(), 8);
        assert!(results[0].candidate_download_urls[0].contains("/b/id/12345-L.jpg"));
    }

    #[test]
    fn test_parse_empty_results() {
        let source = OpenLibraryImageSource::new();
        let json = r#"{"docs": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_no_cover_skipped() {
        let source = OpenLibraryImageSource::new();
        let json = r#"{
            "docs": [
                {
                    "key": "/works/OL999W",
                    "title": "Book without cover"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_cover_urls() {
        let urls = OpenLibraryImageSource::cover_urls("id", "12345");
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://covers.openlibrary.org/b/id/12345-L.jpg?default=false");
        assert_eq!(urls[1], "https://covers.openlibrary.org/b/id/12345-M.jpg?default=false");
    }

    #[test]
    fn test_cover_urls_empty_value() {
        let urls = OpenLibraryImageSource::cover_urls("id", "");
        assert!(urls.is_empty());
    }
}
