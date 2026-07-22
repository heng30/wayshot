//! Bluesky image search source.
//!
//! Replaces Python's `BlueskyImageClient`. Uses the Bluesky AT Protocol
//! search API with JSON response parsing.
//!
//! This source uses cursor-based pagination. The first request returns
//! a cursor value that is used for subsequent pages.

use reqwest::header::{HeaderMap, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Default page size for Bluesky API requests.
#[allow(dead_code)]
const DEFAULT_PAGE_SIZE: usize = 100;

/// Bluesky image search source.
pub struct BlueskyImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl BlueskyImageSource {
    /// Create a new Bluesky image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            "accept",
            "application/json, text/plain, */*".parse().unwrap(),
        );
        search_headers.insert(
            "referer",
            "https://bsky.app/".parse().unwrap(),
        );
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            "referer",
            "https://bsky.app/".parse().unwrap(),
        );
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }

    /// Recursively extract image groups from a Bluesky embed structure.
    ///
    /// Bluesky posts can have images in `embed.images`, `embed.media.images`,
    /// or nested structures. This function walks the tree and collects
    /// groups of candidate URLs (fullsize + thumb) for each image.
    fn extract_image_groups(node: &Value) -> Vec<Vec<String>> {
        let mut image_groups = Vec::new();

        if let Some(arr) = node.as_array() {
            for item in arr {
                image_groups.extend(Self::extract_image_groups(item));
            }
            return image_groups;
        }

        if !node.is_object() {
            return image_groups;
        }

        // Check if this node has fullsize/thumb directly (an image object)
        let mut candidate_urls = Vec::new();
        if let Some(url) = node.get("fullsize").and_then(|v| v.as_str()) {
            if url.starts_with("http") {
                candidate_urls.push(url.to_string());
            }
        }
        if let Some(url) = node.get("thumb").and_then(|v| v.as_str()) {
            if url.starts_with("http") && !candidate_urls.contains(&url.to_string()) {
                candidate_urls.push(url.to_string());
            }
        }

        if !candidate_urls.is_empty() {
            image_groups.push(candidate_urls);
            return image_groups;
        }

        // Recurse into "images" and "media" keys
        for key in &["images", "media"] {
            if let Some(child) = node.get(*key) {
                image_groups.extend(Self::extract_image_groups(child));
            }
        }

        image_groups
    }
}

impl Default for BlueskyImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for BlueskyImageSource {
    fn source_name(&self) -> &str {
        "bluesky"
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
        let base_url = "https://api.bsky.app/xrpc/app.bsky.feed.searchPosts?";
        // Bluesky uses cursor-based pagination, but we construct multiple URLs
        // with estimated page counts. The cursor parameter is not known until
        // a response is received, so we only construct the first URL here.
        // Additional pages would need to be fetched using the cursor from
        // previous responses.
        let page_size = filters
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| {
                let computed = (params.search_limits as f64 * 1.2).min(100.0).max(1.0);
                computed as i64
            }) as usize;
        let page_size = page_size.clamp(1, 100);

        let mut url = format!(
            "{}q={}&limit={}&sort=latest",
            base_url,
            urlencoding::encode(keyword),
            page_size,
        );
        for (key, value) in filters {
            if key != "limit" && key != "q" && key != "sort"
                && let Some(s) = value.as_str()
            {
                url.push_str(&format!("&{}={}", key, urlencoding::encode(s)));
            }
        }

        vec![SearchUrl::new(url)]
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let search_result: Value = serde_json::from_str(body).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        let mut image_infos = Vec::new();

        let posts = search_result.get("posts").and_then(|v| v.as_array());
        if let Some(post_list) = posts {
            for post in post_list {
                if !post.is_object() {
                    continue;
                }

                let embed = post.get("embed").cloned().unwrap_or(Value::Null);
                let image_groups = Self::extract_image_groups(&embed);

                let post_identifier = post
                    .get("uri")
                    .or_else(|| post.get("cid"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                for (image_idx, candidate_urls) in image_groups.into_iter().enumerate() {
                    let identifier = if !post_identifier.is_empty() {
                        format!("{}#{}", post_identifier, image_idx)
                    } else {
                        candidate_urls.first().cloned().unwrap_or_default()
                    };

                    image_infos.push(ImageInfo {
                        source: self.source_name().to_string(),
                        download_url: None,
                        candidate_download_urls: candidate_urls,
                        description: String::new(),
                        identifier,
                        work_dir: Default::default(),
                        ext: None,
                        save_name: None,
                        save_path: None,
                        extra: post.clone(),
                    });
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
        let source = BlueskyImageSource::new();
        let params = SearchParams {
            search_limits: 50,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // Bluesky uses cursor-based pagination, so only one URL is constructed
        assert_eq!(urls.len(), 1);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("sort=latest"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = BlueskyImageSource::new();
        let json = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:abc/app.bsky.feed.post/123",
                    "cid": "cid123",
                    "embed": {
                        "images": [
                            {
                                "fullsize": "https://cdn.bsky.app/img/feed_fullsize/img1.jpg",
                                "thumb": "https://cdn.bsky.app/img/feed_thumbnail/img1.jpg"
                            },
                            {
                                "fullsize": "https://cdn.bsky.app/img/feed_fullsize/img2.jpg",
                                "thumb": "https://cdn.bsky.app/img/feed_thumbnail/img2.jpg"
                            }
                        ]
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "at://did:plc:abc/app.bsky.feed.post/123#0");
        assert_eq!(results[0].candidate_download_urls[0], "https://cdn.bsky.app/img/feed_fullsize/img1.jpg");
        assert_eq!(results[1].identifier, "at://did:plc:abc/app.bsky.feed.post/123#1");
    }

    #[test]
    fn test_parse_embed_with_media() {
        let source = BlueskyImageSource::new();
        let json = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:xyz/app.bsky.feed.post/456",
                    "embed": {
                        "media": {
                            "images": [
                                {
                                    "fullsize": "https://cdn.bsky.app/img/feed_fullsize/img3.jpg",
                                    "thumb": "https://cdn.bsky.app/img/feed_thumbnail/img3.jpg"
                                }
                            ]
                        }
                    }
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].candidate_download_urls[0], "https://cdn.bsky.app/img/feed_fullsize/img3.jpg");
    }

    #[test]
    fn test_parse_empty_results() {
        let source = BlueskyImageSource::new();
        let json = r#"{"posts": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_extract_image_groups() {
        let node = serde_json::json!({
            "images": [
                {"fullsize": "https://example.com/full1.jpg", "thumb": "https://example.com/thumb1.jpg"},
                {"fullsize": "https://example.com/full2.jpg"}
            ]
        });
        let groups = BlueskyImageSource::extract_image_groups(&node);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1].len(), 1);
    }
}
