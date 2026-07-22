//! Weibo image search source.
//!
//! Replaces Python's `WeiboImageClient`. Uses the Weibo mobile API
//! with JSON response parsing. Image URLs are upgraded from thumbnail
//! variants to the full-size `/large/` variant.

use regex::Regex;
use reqwest::header::{HeaderMap, ACCEPT, ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Regex to match Weibo image URL size variants (thumbnail, thumb150, etc.)
/// and allow replacement with `/large/`.
const SINAIMG_PATTERN: &str = r"(https?://wx\d\.sinaimg\.cn/)(?:thumbnail|thumb150|thumb180|small|square|bmiddle|mw\d+|orj\d+)(/)";

/// Number of results per Weibo API request.
const PAGE_SIZE: usize = 20;

/// Weibo image search source.
pub struct WeiboImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
    sinaimg_re: Regex,
}

impl WeiboImageSource {
    /// Create a new Weibo image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            "Cookie",
            "SUB=_2AkMeQriDf8NxqwFRmv0Rz2LgbI9-zA_EieKoHklYJRM3HRl-yT9yqmE5tRB6NcKWbCbMDwPRXM1ooQJ1pNNWP8ZEg0Ev; WEIBOCN_FROM=1110006030; MLOGIN=0; _T_WM=91941672904; XSRF-TOKEN=72dc30; mweibo_short_token=664a2add81; M_WEIBOCN_PARAMS=luicode%3D10000011%26lfid%3D100103type%253D1%2526q%253D%25E7%258C%25AB%25E5%2592%25AA%26fid%3D100103type%253D1%2526q%253D%25E7%258C%25AB%25E5%2592%25AA%26uicode%3D10000011"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1"
                .parse()
                .unwrap(),
        );
        search_headers.insert(ACCEPT, "application/json, text/plain, */*".parse().unwrap());
        search_headers.insert(
            ACCEPT_LANGUAGE,
            "zh-CN,zh;q=0.9,en;q=0.8".parse().unwrap(),
        );
        search_headers.insert(REFERER, "https://m.weibo.cn/".parse().unwrap());
        search_headers.insert(
            "X-Requested-With",
            "XMLHttpRequest".parse().unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        download_headers.insert(REFERER, "https://weibo.com/".parse().unwrap());

        let sinaimg_re = Regex::new(SINAIMG_PATTERN).expect("Invalid Weibo URL regex");

        Self {
            search_headers,
            download_headers,
            sinaimg_re,
        }
    }

    /// Upgrade a Weibo image URL from a thumbnail variant to the full-size `/large/` variant.
    fn upgrade_image_url(&self, url: &str) -> String {
        if url.is_empty() {
            return url.to_string();
        }
        self.sinaimg_re
            .replace(url, "${1}large${2}")
            .to_string()
    }

    /// Extract image info from a single mblog (microblog post).
    fn extract_pics_from_mblog(&self, mblog: &Value) -> Vec<ImageInfo> {
        let mut image_infos = Vec::new();

        if !mblog.is_object() {
            return image_infos;
        }

        // Strategy 1: pics array (most common in mobile API)
        if let Some(pics) = mblog.get("pics").and_then(|v| v.as_array()) {
            for pic in pics {
                if !pic.is_object() {
                    continue;
                }

                let mut candidate_urls = Vec::new();

                // Try large.url first
                if let Some(large) = pic.get("large").and_then(|v| v.as_object()) {
                    if let Some(url) = large.get("url").and_then(|v| v.as_str()) {
                        if url.starts_with("http") {
                            candidate_urls.push(url.trim().to_string());
                        }
                    }
                }

                // Try pic.url and upgrade it
                if let Some(pic_url) = pic.get("url").and_then(|v| v.as_str()) {
                    let pic_url = pic_url.trim();
                    if pic_url.starts_with("http") {
                        let upgraded = self.upgrade_image_url(pic_url);
                        if !candidate_urls.contains(&upgraded) {
                            candidate_urls.push(upgraded);
                        }
                        if !candidate_urls.contains(&pic_url.to_string()) {
                            candidate_urls.push(pic_url.to_string());
                        }
                    }
                }

                if candidate_urls.is_empty() {
                    continue;
                }

                let identifier = candidate_urls[0].clone();
                image_infos.push(ImageInfo::with_identifier(
                    "weibo",
                    candidate_urls,
                    identifier,
                ));
            }
        }

        // Strategy 2: fallback to single pic fields (when pics array is absent)
        if image_infos.is_empty() {
            for key in &["original_pic", "bmiddle_pic", "thumbnail_pic"] {
                if let Some(pic_url) = mblog.get(*key).and_then(|v| v.as_str()) {
                    let pic_url = pic_url.trim();
                    if pic_url.starts_with("http") {
                        let upgraded = self.upgrade_image_url(pic_url);
                        let candidate_urls = if upgraded != pic_url {
                            vec![upgraded, pic_url.to_string()]
                        } else {
                            vec![pic_url.to_string()]
                        };
                        let identifier = candidate_urls[0].clone();
                        image_infos.push(ImageInfo::with_identifier(
                            "weibo",
                            candidate_urls,
                            identifier,
                        ));
                        break;
                    }
                }
            }
        }

        image_infos
    }
}

impl Default for WeiboImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for WeiboImageSource {
    fn source_name(&self) -> &str {
        "weibo"
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
        let containerid = format!(
            "100103type%3D1%26q%3D{}",
            urlencoding::encode(keyword),
        );
        let base_url = "https://m.weibo.cn/api/container/getIndex";

        (0..num_pages)
            .map(|pn| {
                let page = pn + 1;
                let url = if page == 1 {
                    format!("{}?containerid={}&page_type=searchall", base_url, containerid)
                } else {
                    format!(
                        "{}?containerid={}&page_type=searchall&page={}",
                        base_url, containerid, page
                    )
                };
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

        // Validate response
        if search_result.get("ok").and_then(|v| v.as_i64()) != Some(1) {
            return Ok(vec![]);
        }

        let data = match search_result.get("data") {
            Some(d) if d.is_object() => d,
            _ => return Ok(vec![]),
        };

        let cards = match data.get("cards").and_then(|v| v.as_array()) {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        let mut image_infos = Vec::new();

        for card in cards {
            if !card.is_object() {
                continue;
            }

            let card_type = card.get("card_type").and_then(|v| v.as_i64());

            match card_type {
                Some(9) => {
                    // Single blog post card
                    if let Some(mblog) = card.get("mblog") {
                        image_infos.extend(self.extract_pics_from_mblog(mblog));
                    }
                }
                Some(11) => {
                    // Card group containing multiple blog posts
                    if let Some(card_group) = card.get("card_group").and_then(|v| v.as_array()) {
                        for sub_card in card_group {
                            if !sub_card.is_object() {
                                continue;
                            }
                            if sub_card.get("card_type").and_then(|v| v.as_i64()) == Some(9) {
                                if let Some(sub_mblog) = sub_card.get("mblog") {
                                    image_infos.extend(self.extract_pics_from_mblog(sub_mblog));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(image_infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_image_url() {
        let source = WeiboImageSource::new();
        let upgraded = source.upgrade_image_url(
            "https://wx1.sinaimg.cn/thumbnail/abc123.jpg",
        );
        assert_eq!(upgraded, "https://wx1.sinaimg.cn/large/abc123.jpg");

        let upgraded2 = source.upgrade_image_url(
            "https://wx3.sinaimg.cn/orj480/def456.jpg",
        );
        assert_eq!(upgraded2, "https://wx3.sinaimg.cn/large/def456.jpg");
    }

    #[test]
    fn test_upgrade_image_url_no_change() {
        let source = WeiboImageSource::new();
        let url = "https://wx1.sinaimg.cn/large/abc123.jpg";
        let upgraded = source.upgrade_image_url(url);
        assert_eq!(upgraded, url);
    }

    #[test]
    fn test_construct_search_urls() {
        let source = WeiboImageSource::new();
        let params = SearchParams {
            search_limits: 40,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 40 * 1.2 / 20 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("containerid="));
        assert!(urls[0].url.contains("page_type=searchall"));
        // First page should not have &page= parameter
        assert!(!urls[0].url.contains("&page="));
        assert!(urls[1].url.contains("&page=2"));
    }

    #[test]
    fn test_parse_search_result_card_type_9() {
        let source = WeiboImageSource::new();
        let json = r#"{
            "ok": 1,
            "data": {
                "cards": [
                    {
                        "card_type": 9,
                        "mblog": {
                            "pics": [
                                {
                                    "large": {"url": "https://wx1.sinaimg.cn/large/abc.jpg"},
                                    "url": "https://wx1.sinaimg.cn/thumbnail/abc.jpg"
                                }
                            ]
                        }
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "weibo");
    }

    #[test]
    fn test_parse_search_result_card_type_11() {
        let source = WeiboImageSource::new();
        let json = r#"{
            "ok": 1,
            "data": {
                "cards": [
                    {
                        "card_type": 11,
                        "card_group": [
                            {
                                "card_type": 9,
                                "mblog": {
                                    "original_pic": "https://wx1.sinaimg.cn/bmiddle/xyz.jpg"
                                }
                            }
                        ]
                    }
                ]
            }
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 1);
        // Upgraded URL should be first candidate
        assert!(results[0].candidate_download_urls[0].contains("/large/"));
    }

    #[test]
    fn test_parse_search_result_not_ok() {
        let source = WeiboImageSource::new();
        let json = r#"{"ok": 0}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }
}
