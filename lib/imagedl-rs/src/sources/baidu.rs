//! Baidu image search source.
//!
//! Replaces Python's `BaiduImageClient`. Uses JSON API parsing with
//! URL obfuscation decoding for the `objurl` field.

use reqwest::header::{
    ACCEPT_LANGUAGE, CONNECTION, HeaderMap, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::filter::{Filter, FilterRule};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Baidu image search source.
pub struct BaiduImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl BaiduImageSource {
    /// Create a new Baidu image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            ACCEPT_LANGUAGE,
            "zh-CN,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2"
                .parse()
                .unwrap(),
        );
        search_headers.insert(CONNECTION, "keep-alive".parse().unwrap());
        search_headers.insert(UPGRADE_INSECURE_REQUESTS, "1".parse().unwrap());
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
        }
    }

    /// Decode Baidu's obfuscated URL.
    ///
    /// Baidu encodes image URLs by replacing characters and applying a
    /// substitution cipher. This reverses the encoding.
    fn decode_url(url: &str) -> String {
        // Character substitution table
        let in_table = "0123456789abcdefghijklmnopqrstuvw";
        let out_table = "7dgjmoru140852vsnkheb963wtqplifca";

        let mut result = url.to_string();

        // Replace encoded sequences
        result = result.replace("_z2C$q", ":");
        result = result.replace("_z&e3B", ".");
        result = result.replace("AzdH3F", "/");

        // Apply character translation
        let translate: std::collections::HashMap<char, char> =
            in_table.chars().zip(out_table.chars()).collect();

        result
            .chars()
            .map(|c| *translate.get(&c).unwrap_or(&c))
            .collect()
    }

    /// Extract all candidate image URLs from a Baidu search result item.
    fn pick_all_image_urls(item: &Value) -> Vec<String> {
        let mut candidate_urls = Vec::new();

        // Helper to get a string field with lowercase key matching
        let get_str_field = |item: &Value, key: &str| -> Option<String> {
            // Try exact key first, then lowercase
            if let Some(v) = item.get(key).and_then(|v| v.as_str())
                && !v.trim().is_empty()
            {
                return Some(v.trim().to_string());
            }
            // Try lowercase version
            let lower_key = key.to_lowercase();
            if let Some(v) = item.get(&lower_key).and_then(|v| v.as_str())
                && !v.trim().is_empty()
            {
                return Some(v.trim().to_string());
            }
            None
        };

        // Try 1: objURL (obfuscated, needs decoding)
        if let Some(objurl) = get_str_field(item, "objURL") {
            let decoded = Self::decode_url(&objurl);
            if decoded.starts_with("http") {
                candidate_urls.push(decoded);
            } else if objurl.starts_with("http") {
                // If the URL is already a valid HTTP URL, use it as-is
                candidate_urls.push(objurl);
            }
        }

        // Try 2: hoverURL
        if let Some(url) = get_str_field(item, "hoverURL")
            && url.starts_with("http")
        {
            candidate_urls.push(url);
        }

        // Try 3: replaceUrl[].objurl
        if let Some(replace_urls) = item.get("replaceUrl").and_then(|v| v.as_array()) {
            for r in replace_urls {
                if let Some(objurl) = r.get("objURL").and_then(|v| v.as_str())
                    && !objurl.trim().is_empty()
                    && objurl.trim().starts_with("http")
                {
                    candidate_urls.push(objurl.trim().to_string());
                }
            }
        }

        // Try 4: middleURL
        if let Some(url) = get_str_field(item, "middleURL")
            && url.starts_with("http")
        {
            candidate_urls.push(url);
        }

        // Try 5: thumbURL
        if let Some(url) = get_str_field(item, "thumbURL")
            && url.starts_with("http")
        {
            candidate_urls.push(url);
        }

        // Fix escaped slashes
        candidate_urls
            .into_iter()
            .map(|url| url.replace("\\/", "/"))
            .collect()
    }
}

impl Default for BaiduImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for BaiduImageSource {
    fn source_name(&self) -> &str {
        "baidu"
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
        let page_size: usize = 30;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        let filter_str = self.build_filter().apply(filters, "&").unwrap_or_default();
        let filter_param = if filter_str.is_empty() {
            String::new()
        } else {
            format!("&{}", filter_str)
        };

        (0..num_pages)
            .map(|pn| {
                let url = format!(
                    "http://image.baidu.com/search/acjson?tn=resultjson_com&ipn=rj&word={}&pn={}&rn={}{}",
                    urlencoding::encode(keyword),
                    pn * page_size,
                    page_size,
                    filter_param,
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let search_result: Value = serde_json::from_str(body).map_err(|e| ImageDlError::Parse {
            origin: self.source_name().to_string(),
            reason: format!("JSON parse error: {}", e),
        })?;

        let mut image_infos = Vec::new();

        let data = search_result.get("data").and_then(|v| v.as_array());
        if let Some(items) = data {
            for item in items {
                if !item.is_object() {
                    continue;
                }
                let candidate_urls = Self::pick_all_image_urls(item);
                if candidate_urls.is_empty() {
                    continue;
                }
                let identifier = candidate_urls[0].clone();
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
                    extra: item.clone(),
                });
            }
        }

        Ok(image_infos)
    }

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // Type filter
        let type_code = [
            ("portrait", "s=3&lm=0&st=-1&face=0"),
            ("face", "s=0&lm=0&st=-1&face=1"),
            ("clipart", "s=0&lm=0&st=1&face=0"),
            ("linedrawing", "s=0&lm=0&st=2&face=0"),
            ("animated", "s=0&lm=6&st=-1&face=0"),
            ("static", "s=0&lm=7&st=-1&face=0"),
        ];
        let type_choices: Vec<&str> = type_code.iter().map(|(k, _)| *k).collect();
        let type_map: std::collections::HashMap<&str, &str> = type_code.into_iter().collect();
        f.add_rule(FilterRule::with_string_choices(
            "type",
            type_choices,
            move |v| {
                type_map
                    .get(v)
                    .copied()
                    .unwrap_or("s=0&lm=0&st=-1&face=0")
                    .to_string()
            },
        ));

        // Color filter
        let color_code = [
            ("red", "1"),
            ("orange", "256"),
            ("yellow", "2"),
            ("green", "4"),
            ("purple", "32"),
            ("pink", "64"),
            ("teal", "8"),
            ("blue", "16"),
            ("brown", "12"),
            ("white", "1024"),
            ("black", "512"),
            ("blackandwhite", "2048"),
        ];
        let color_choices: Vec<&str> = color_code.iter().map(|(k, _)| *k).collect();
        let color_map: std::collections::HashMap<&str, &str> = color_code.into_iter().collect();
        f.add_rule(FilterRule::with_string_choices(
            "color",
            color_choices,
            move |v| format!("ic={}", color_map.get(v).copied().unwrap_or("0")),
        ));

        // Size filter
        f.add_rule(FilterRule::with_any_string("size", |v| match v {
            "extralarge" => "z=9".to_string(),
            "large" => "z=3".to_string(),
            "medium" => "z=2".to_string(),
            "small" => "z=1".to_string(),
            s if s.starts_with('=') => {
                let dims: Vec<&str> = s[1..].split('x').collect();
                if dims.len() == 2 {
                    format!("width={}&height={}", dims[0], dims[1])
                } else {
                    "z=0".to_string()
                }
            }
            _ => "z=0".to_string(),
        }));

        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FilterValue;

    #[test]
    fn test_decode_url() {
        // Test the basic character substitution
        let decoded = BaiduImageSource::decode_url("AzdH3F_z2C$q_z&e3Bcom");
        assert!(decoded.contains('/'));
        assert!(decoded.contains(':'));
        assert!(decoded.contains('.'));
    }

    #[test]
    fn test_construct_search_urls() {
        let source = BaiduImageSource::new();
        let params = SearchParams {
            search_limits: 60,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 60 * 1.2 / 30 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("word=cats"));
        assert!(urls[0].url.contains("pn=0"));
        assert!(urls[1].url.contains("pn=30"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = BaiduImageSource::new();
        let json = r#"{
            "data": [
                {
                    "objURL": "https://example.com/cat1.jpg",
                    "hoverURL": "https://example.com/cat1_hover.jpg",
                    "middleURL": "https://example.com/cat1_mid.jpg",
                    "thumbURL": "https://example.com/cat1_thumb.jpg"
                },
                {
                    "objURL": "https://example.com/cat2.jpg"
                }
            ]
        }"#;
        let results = source.parse_search_result(json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "baidu");
        // objURL is decoded, so it should be present
        assert!(!results[0].candidate_download_urls.is_empty());
    }

    #[test]
    fn test_parse_empty_data() {
        let source = BaiduImageSource::new();
        let json = r#"{"data": []}"#;
        let results = source.parse_search_result(json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_baidu_filter_type() {
        let source = BaiduImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("type".to_string(), FilterValue::from("clipart"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "s=0&lm=0&st=1&face=0");
    }
}
