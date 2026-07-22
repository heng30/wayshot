//! DuckDuckGo image search source.
//!
//! Replaces Python's `DuckduckgoImageClient`. Uses DuckDuckGo's internal
//! image search API (`i.js`) which requires a `vqd` token obtained from
//! an initial HTML page fetch.
//!
//! # Flow
//!
//! 1. Fetch `https://duckduckgo.com/?q={keyword}` to extract the `vqd` token
//! 2. Use the `vqd` token in requests to `https://duckduckgo.com/i.js?o=json&...`
//!
//! Because the `vqd` token must be obtained before constructing search URLs,
//! this source overrides the `search()` method.

use futures::stream::{self, StreamExt};
use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;
use std::collections::HashSet;

use crate::client::http::HttpClient;
use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::filter::{Filter, FilterRule};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// DuckDuckGo image search source.
pub struct DuckduckgoImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

/// Valid safesearch modes for DuckDuckGo.
const VALID_SAFE_MODES: &[(&str, &str)] = &[
    ("on", "1"),
    ("moderate", "1"),
    ("off", "-1"),
];

/// DuckDuckGo image search page size.
const PAGE_SIZE: usize = 100;

impl DuckduckgoImageSource {
    /// Create a new DuckDuckGo image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert("accept", "*/*".parse().unwrap());
        search_headers.insert(
            "accept-language",
            "en-US,en;q=0.5".parse().unwrap(),
        );
        search_headers.insert(
            "referer",
            "https://duckduckgo.com/".parse().unwrap(),
        );
        search_headers.insert("sec-gpc", "1".parse().unwrap());
        search_headers.insert("connection", "keep-alive".parse().unwrap());
        search_headers.insert("sec-fetch-dest", "empty".parse().unwrap());
        search_headers.insert("sec-fetch-mode", "cors".parse().unwrap());
        search_headers.insert(
            "sec-fetch-site",
            "same-origin".parse().unwrap(),
        );
        search_headers.insert("priority", "u=4".parse().unwrap());

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

    /// Fetch the `vqd` token from DuckDuckGo HTML page.
    ///
    /// The `vqd` token is required for image search API requests.
    /// It is extracted from the HTML response by looking for patterns
    /// like `vqd="..."`, `vqd=...&`, or `vqd='...'`.
    async fn get_vqd(
        keyword: &str,
        http: &HttpClient,
        headers: HeaderMap,
    ) -> Result<String> {
        let url = format!(
            "https://duckduckgo.com/?q={}",
            urlencoding::encode(keyword)
        );
        let html = http.get_text(&url, headers).await?;

        // Try patterns: vqd="...", vqd=...&, vqd='...'
        let patterns: &[(&[u8], &[u8])] = &[
            (b"vqd=\"", b"\""),
            (b"vqd=", b"&"),
            (b"vqd='", b"'"),
        ];

        for (prefix, suffix) in patterns {
            if let Some(pos) = html.as_bytes().windows(prefix.len()).position(|w| w == *prefix)
            {
                let start = pos + prefix.len();
                if let Some(end) = html.as_bytes()[start..].iter().position(|&b| b == suffix[0]) {
                    let vqd = &html[start..start + end];
                    if !vqd.is_empty() {
                        return Ok(vqd.to_string());
                    }
                }
            }
        }

        Err(ImageDlError::Parse {
            origin: "duckduckgo".to_string(),
            reason: "Failed to extract vqd token from DuckDuckGo page".to_string(),
        })
    }
}

impl Default for DuckduckgoImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for DuckduckgoImageSource {
    fn source_name(&self) -> &str {
        "duckduckgo"
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
        // NOTE: These URLs cannot be used without the vqd token.
        // The overridden search() method handles vqd retrieval.
        // This method is provided for trait compliance but returns
        // placeholder URLs that should not be fetched directly.
        let num_pages =
            ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        let safesearch = filters
            .get("safesearch")
            .and_then(|v| v.as_str())
            .and_then(|s| VALID_SAFE_MODES.iter().find(|(k, _)| *k == s).map(|(_, v)| *v))
            .unwrap_or("-1");

        let region = filters
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("us-en");

        (0..num_pages)
            .map(|pn| {
                SearchUrl::new(format!(
                    "https://duckduckgo.com/i.js?o=json&q={}&l={}&p={}&s={}",
                    urlencoding::encode(keyword),
                    region,
                    safesearch,
                    pn * PAGE_SIZE,
                ))
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

        let results = search_result.get("results").and_then(|v| v.as_array());
        if let Some(items) = results {
            for item in items {
                if !item.is_object() {
                    continue;
                }

                let image_url = item
                    .get("image")
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                    .map(|s| s.to_string());
                let thumbnail_url = item
                    .get("thumbnail")
                    .and_then(|v| v.as_str())
                    .filter(|s| s.starts_with("http"))
                    .map(|s| s.to_string());

                let candidate_urls: Vec<String> =
                    [image_url, thumbnail_url].into_iter().flatten().collect();

                if candidate_urls.is_empty() {
                    continue;
                }

                // Use image_token as identifier, fallback to first URL
                let identifier = item
                    .get("image_token")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
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

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // Time filter
        f.add_rule(FilterRule::with_string_choices(
            "time",
            vec!["Day", "Week", "Month", "Year"],
            |v| format!("time:{}", v),
        ));

        // Size filter
        f.add_rule(FilterRule::with_string_choices(
            "size",
            vec!["Small", "Medium", "Large", "Wallpaper"],
            |v| format!("size:{}", v),
        ));

        // Color filter
        f.add_rule(FilterRule::with_string_choices(
            "color",
            vec![
                "color",
                "Monochrome",
                "Red",
                "Orange",
                "Yellow",
                "Green",
                "Blue",
                "Purple",
                "Pink",
                "Brown",
                "Black",
                "Gray",
                "Teal",
                "White",
            ],
            |v| format!("color:{}", v),
        ));

        // Type filter
        f.add_rule(FilterRule::with_string_choices(
            "type",
            vec!["photo", "clipart", "gif", "transparent", "line"],
            |v| format!("type:{}", v),
        ));

        // Layout filter
        f.add_rule(FilterRule::with_string_choices(
            "layout",
            vec!["Square", "Tall", "Wide"],
            |v| format!("layout:{}", v),
        ));

        // License filter
        f.add_rule(FilterRule::with_string_choices(
            "license",
            vec![
                "any",
                "Public",
                "Share",
                "ShareCommercially",
                "Modify",
                "ModifyCommercially",
            ],
            |v| format!("license:{}", v),
        ));

        f
    }

    /// Override the default search flow to fetch the vqd token first.
    ///
    /// 1. Fetch `https://duckduckgo.com/?q={keyword}` to get the vqd token
    /// 2. Construct search URLs with the vqd token embedded
    /// 3. Fetch and parse each page as normal
    async fn search(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
        http: &HttpClient,
    ) -> Result<Vec<ImageInfo>> {
        // Step 1: Get vqd token
        let vqd_headers = self.search_headers();
        let vqd = Self::get_vqd(keyword, http, vqd_headers).await?;

        log::info!("[{}] Obtained vqd token: {}", self.source_name(), vqd);

        // Step 2: Construct search URLs with vqd
        let safesearch = filters
            .get("safesearch")
            .and_then(|v| v.as_str())
            .and_then(|s| VALID_SAFE_MODES.iter().find(|(k, _)| *k == s).map(|(_, v)| *v))
            .unwrap_or("-1");

        let region = filters
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("us-en");

        let num_pages =
            ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        let filter_str = self.build_filter().apply(filters, ",")?;
        let urls: Vec<SearchUrl> = (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "https://duckduckgo.com/i.js?o=json&q={}&l={}&p={}&vqd={}&s={}",
                    urlencoding::encode(keyword),
                    region,
                    safesearch,
                    urlencoding::encode(&vqd),
                    pn * PAGE_SIZE,
                );
                if !filter_str.is_empty() {
                    url.push_str(&format!("&f={}", urlencoding::encode(&filter_str)));
                }
                SearchUrl::new(url)
            })
            .collect();

        if urls.is_empty() {
            return Ok(vec![]);
        }

        // Step 3: Fetch all pages concurrently
        let search_headers = self.search_headers();
        let concurrency = params.concurrency;

        log::info!(
            "[{}] Starting search ({} URLs)",
            self.source_name(),
            urls.len()
        );

        let fetch_results: Vec<Option<Result<String>>> = stream::iter(urls)
            .map(|search_url| {
                let http = http.clone();
                let headers = search_headers.clone();
                async move {
                    let result = http.get_text(&search_url.url, headers).await;
                    match result {
                        Ok(text) => {
                            log::debug!("Search request succeeded: {}", search_url.url);
                            Some(Ok(text))
                        }
                        Err(e) => {
                            log::warn!("Search request failed: {} - {}", search_url.url, e);
                            Some(Err(e))
                        }
                    }
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        // Separate bodies and errors
        let mut bodies = Vec::new();
        let mut last_error = None;
        for result in fetch_results.into_iter().flatten() {
            match result {
                Ok(text) => bodies.push(text),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        if bodies.is_empty() {
            if let Some(e) = last_error {
                return Err(e);
            }
            return Ok(vec![]);
        }

        // Parse sequentially
        let mut all_infos = Vec::new();
        for body in &bodies {
            match self.parse_search_result(body) {
                Ok(infos) => all_infos.extend(infos),
                Err(e) => {
                    log::warn!(
                        "[{}] Failed to parse search result: {}",
                        self.source_name(),
                        e
                    );
                }
            }
        }

        // Deduplicate by identifier
        let mut seen = HashSet::new();
        let infos: Vec<ImageInfo> = all_infos
            .into_iter()
            .filter(|info| seen.insert(info.identifier.clone()))
            .collect();

        // Assign file paths
        let timestamp = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
        let dir_name = format!("{} {}", timestamp, keyword);
        let dir = params.work_dir.join(self.source_name()).join(dir_name);

        let infos: Vec<ImageInfo> = infos
            .into_iter()
            .enumerate()
            .map(|(idx, mut info)| {
                let save_name = format!("{:08}", idx + 1);
                info.work_dir = dir.clone();
                info.save_name = Some(save_name);
                info.save_path = Some(dir.join(info.save_name.as_ref().unwrap()));
                info
            })
            .collect();

        log::info!(
            "[{}] Search completed ({} results)",
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
        let source = DuckduckgoImageSource::new();
        let params = SearchParams {
            search_limits: 200,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 200 * 1.2 / 100 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("duckduckgo.com/i.js"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = DuckduckgoImageSource::new();
        let json = r#"{
            "results": [
                {
                    "image": "https://example.com/cat1.jpg",
                    "thumbnail": "https://example.com/thumb1.jpg",
                    "title": "A cute cat",
                    "image_token": "token_abc"
                },
                {
                    "image": "https://example.com/cat2.jpg",
                    "title": "Another cat"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "token_abc");
        assert_eq!(results[0].description, "A cute cat");
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        assert_eq!(results[1].identifier, "https://example.com/cat2.jpg");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = DuckduckgoImageSource::new();
        let json = r#"{"results": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_safesearch_filter() {
        let source = DuckduckgoImageSource::new();
        let mut filters = Filters::new();
        filters.insert("safesearch".to_string(), crate::types::FilterValue::from("on"));
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &filters);
        assert!(urls[0].url.contains("p=1"));
    }

    #[test]
    fn test_build_filter() {
        let source = DuckduckgoImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("size".to_string(), crate::types::FilterValue::from("Large"));
        let result = filter.apply(&options, ",").unwrap();
        assert_eq!(result, "size:Large");
    }
}
