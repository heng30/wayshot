//! Metropolitan Museum of Art image search source.
//!
//! Replaces Python's `MetropolitanImageClient`. Uses the Met's open access API
//! with a two-step search process: first retrieve object IDs, then fetch
//! individual object details.

use futures::StreamExt;
use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::http::HttpClient;
use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Metropolitan Museum of Art image search source.
///
/// This source requires a two-step search process:
/// 1. Search for object IDs matching the keyword
/// 2. Fetch details for each object individually
///
/// Because of this, the `search()` method is overridden instead of relying on
/// the default `construct_search_urls` / `parse_search_result` flow.
pub struct MetropolitanImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

/// Maximum number of object details to fetch to avoid excessive HTTP requests.
const MAX_OBJECT_FETCH: usize = 200;

impl MetropolitanImageSource {
    /// Create a new Metropolitan image source.
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

        let mut search_headers = HeaderMap::new();
        search_headers.insert(USER_AGENT, ua.parse().unwrap());

        let mut download_headers = HeaderMap::new();
        download_headers.insert(USER_AGENT, ua.parse().unwrap());

        Self {
            search_headers,
            download_headers,
        }
    }
}

impl Default for MetropolitanImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for MetropolitanImageSource {
    fn source_name(&self) -> &str {
        "metropolitan"
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
        // Step 1 URL: search for object IDs
        let limit = ((params.search_limits as f64 * 1.2).ceil() as usize).min(MAX_OBJECT_FETCH);
        let mut url = format!(
            "https://collectionapi.metmuseum.org/public/collection/v1/search?q={}&hasImages=true",
            urlencoding::encode(keyword),
        );
        // Add extra filter params
        for (key, value) in filters {
            if key != "q" && key != "hasImages" {
                if let Some(s) = value.as_str() {
                    url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
                }
            }
        }
        let _ = limit; // limit is used in the overridden search()
        vec![SearchUrl::new(url)]
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        // This parses a single object detail response (from step 2).
        // It is also used by the default search flow as a fallback.
        let obj: Value = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        let primary_image = obj.get("primaryImage").and_then(|v| v.as_str());
        let primary_image_small = obj.get("primaryImageSmall").and_then(|v| v.as_str());

        let mut candidate_urls = Vec::new();
        if let Some(url) = primary_image.filter(|s| !s.is_empty() && s.starts_with("http")) {
            candidate_urls.push(url.to_string());
        }
        if let Some(url) = primary_image_small.filter(|s| !s.is_empty() && s.starts_with("http")) {
            candidate_urls.push(url.to_string());
        }

        if candidate_urls.is_empty() {
            return Ok(vec![]);
        }

        let identifier = obj
            .get("objectID")
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| candidate_urls[0].clone());

        Ok(vec![ImageInfo {
            source: self.source_name().to_string(),
            download_url: None,
            candidate_download_urls: candidate_urls,
            description: obj
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            identifier,
            work_dir: Default::default(),
            ext: None,
            save_name: None,
            save_path: None,
            extra: obj,
        }])
    }

    /// Override the default search flow for the two-step Met API.
    ///
    /// Step 1: Fetch object IDs from the search endpoint.
    /// Step 2: Fetch details for each object ID in parallel.
    async fn search(
        &self,
        keyword: &str,
        params: &SearchParams,
        filters: &Filters,
        http: &HttpClient,
    ) -> Result<Vec<ImageInfo>> {
        let limit = ((params.search_limits as f64 * 1.2).ceil() as usize).min(MAX_OBJECT_FETCH);
        let search_headers = self.search_headers();
        let concurrency = params.concurrency;

        log::info!("[{}] Starting two-step search", self.source_name());

        // Step 1: Search for object IDs
        let search_urls = self.construct_search_urls(keyword, params, filters);
        if search_urls.is_empty() {
            return Ok(vec![]);
        }

        let search_body = http
            .get_text(&search_urls[0].url, search_headers.clone())
            .await
            .map_err(|e| {
                log::warn!("[{}] Step 1 search request failed: {}", self.source_name(), e);
                e
            })?;

        let search_result: Value = serde_json::from_str(&search_body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("Failed to parse search response: {}", e),
            }
        })?;

        let object_ids: Vec<i64> = search_result
            .get("objectIDs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64())
                    .take(limit)
                    .collect()
            })
            .unwrap_or_default();

        if object_ids.is_empty() {
            log::info!("[{}] No object IDs found", self.source_name());
            return Ok(vec![]);
        }

        log::info!(
            "[{}] Found {} object IDs, fetching details",
            self.source_name(),
            object_ids.len()
        );

        // Step 2: Fetch details for each object in parallel
        let fetch_results: Vec<Option<Result<String>>> =
            futures::stream::iter(object_ids.iter().cloned())
                .map(|obj_id| {
                    let http = http.clone();
                    let headers = search_headers.clone();
                    async move {
                        let url = format!(
                            "https://collectionapi.metmuseum.org/public/collection/v1/objects/{}",
                            obj_id
                        );
                        match http.get_text(&url, headers).await {
                            Ok(text) => {
                                log::debug!("Object {} fetch succeeded", obj_id);
                                Some(Ok(text))
                            }
                            Err(e) => {
                                log::warn!("Object {} fetch failed: {}", obj_id, e);
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

        // Parse each object detail response
        let mut all_infos = Vec::new();
        for body in &bodies {
            match self.parse_search_result(body) {
                Ok(infos) => all_infos.extend(infos),
                Err(e) => {
                    log::warn!(
                        "[{}] Failed to parse object detail: {}",
                        self.source_name(),
                        e
                    );
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
        let keyword_owned = keyword.to_string();
        let infos: Vec<ImageInfo> = {
            let timestamp = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
            let dir_name = format!("{} {}", timestamp, keyword_owned);
            let dir = params.work_dir.join(self.source_name()).join(dir_name);

            infos
                .into_iter()
                .enumerate()
                .map(|(idx, mut info)| {
                    let save_name = format!("{:08}", idx + 1);
                    info.work_dir = dir.clone();
                    info.save_name = Some(save_name);
                    info.save_path = Some(dir.join(info.save_name.as_ref().unwrap()));
                    info
                })
                .collect()
        };

        log::info!(
            "[{}] Two-step search completed ({} results)",
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
        let source = MetropolitanImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("van gogh", &params, &Filters::new());
        assert_eq!(urls.len(), 1);
        assert!(urls[0].url.contains("q=van%20gogh"));
        assert!(urls[0].url.contains("hasImages=true"));
        assert!(urls[0].url.contains("collectionapi.metmuseum.org"));
    }

    #[test]
    fn test_parse_search_result_single_object() {
        let source = MetropolitanImageSource::new();
        let json = r#"{
            "objectID": 436535,
            "title": "Starry Night",
            "primaryImage": "https://images.metmuseum.org/images/large.jpg",
            "primaryImageSmall": "https://images.metmuseum.org/images/small.jpg"
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier, "436535");
        assert_eq!(results[0].description, "Starry Night");
        assert_eq!(results[0].candidate_download_urls.len(), 2);
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://images.metmuseum.org/images/large.jpg"
        );
    }

    #[test]
    fn test_parse_object_without_images() {
        let source = MetropolitanImageSource::new();
        let json = r#"{
            "objectID": 999,
            "title": "No Image Object",
            "primaryImage": "",
            "primaryImageSmall": ""
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_object_with_only_small_image() {
        let source = MetropolitanImageSource::new();
        let json = r#"{
            "objectID": 888,
            "title": "Small Image Only",
            "primaryImage": "",
            "primaryImageSmall": "https://images.metmuseum.org/images/small.jpg"
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].candidate_download_urls.len(), 1);
        assert_eq!(
            results[0].candidate_download_urls[0],
            "https://images.metmuseum.org/images/small.jpg"
        );
    }

    #[test]
    fn test_max_object_fetch_constant() {
        assert_eq!(MAX_OBJECT_FETCH, 200);
    }

    #[test]
    fn test_parse_empty_object() {
        let source = MetropolitanImageSource::new();
        let json = r#"{"objectID": 1}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
