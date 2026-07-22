//! Yandex image search source.
//!
//! Replaces Python's `YandexImageClient`. Uses HTML scraping to extract
//! image URLs from Yandex image search. Parses the `data-state` JSON
//! attribute from the `#ImagesApp-*` element to access structured
//! image data.
//!
//! The `data-state` JSON may contain HTML tags (e.g. `<b>cat</b>`) and
//! HTML entities like `&quot;` inside string values, which break
//! `serde_json` parsing. These are handled by:
//! 1. Replacing `&quot;` with a placeholder before HTML entity decoding
//! 2. Decoding HTML entities and stripping HTML tags
//! 3. Restoring the placeholder as escaped quotes `\"`

use regex::Regex;
use reqwest::header::{HeaderMap, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use serde_json::Value;

use crate::client::ImageSource;
use crate::error::{ImageDlError, Result};
use crate::filter::{Filter, FilterRule};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Number of results per Yandex search page.
const PAGE_SIZE: usize = 30;

/// Placeholder used to preserve `&quot;` through HTML entity decoding.
const QUOTE_PLACEHOLDER: &str = "\x00Q\x00";

/// Yandex image search source.
pub struct YandexImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
    data_state_re: Regex,
    html_tag_re: Regex,
}

impl YandexImageSource {
    /// Create a new Yandex image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            ACCEPT_LANGUAGE,
            "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7"
                .parse()
                .unwrap(),
        );

        let mut download_headers = HeaderMap::new();
        download_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let data_state_re = Regex::new(r#"id="ImagesApp-[^"]*"\s*data-state="(\{.*?})"\s*data-hydrate-priority="#)
            .expect("Invalid Yandex data-state regex");

        let html_tag_re = Regex::new(r"<[^>]+>")
            .expect("Invalid HTML tag regex");

        Self {
            search_headers,
            download_headers,
            data_state_re,
            html_tag_re,
        }
    }

    /// Prepare data-state JSON for parsing by handling HTML entities and tags.
    ///
    /// The JSON inside `data-state` may contain:
    /// - `&quot;` representing actual quote characters inside JSON strings
    /// - HTML tags like `<b>...</b>` inside string values
    /// - Other HTML entities like `&amp;`
    ///
    /// We must decode `&quot;` to escaped JSON quotes (`\"`), not bare `"`,
    /// otherwise the JSON structure breaks.
    fn prepare_json(&self, raw: &str) -> String {
        // Step 1: Replace &quot; with placeholder to preserve it through decoding
        let s = raw.replace("&quot;", QUOTE_PLACEHOLDER);

        // Step 2: Decode HTML entities
        let s = html_escape::decode_html_entities(&s).to_string();

        // Step 3: Strip HTML tags
        let s = self.html_tag_re.replace_all(&s, "").to_string();

        // Step 4: Restore placeholder as escaped JSON quote
        let s = s.replace(QUOTE_PLACEHOLDER, "\\\"");

        // Step 5: Decode any remaining HTML entities
        html_escape::decode_html_entities(&s).to_string()
    }
}

impl Default for YandexImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for YandexImageSource {
    fn source_name(&self) -> &str {
        "yandex"
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
        let num_pages = ((params.search_limits as f64 * 1.2 / PAGE_SIZE as f64).ceil()) as usize;

        let filter_str = self
            .build_filter()
            .apply(filters, "&")
            .unwrap_or_default();

        (0..num_pages)
            .map(|pn| {
                let mut url = format!(
                    "https://yandex.com/images/search?text={}&p={}",
                    urlencoding::encode(keyword),
                    pn + 1,
                );
                if !filter_str.is_empty() {
                    url.push_str(&format!("&{}", filter_str));
                }
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        // Unescape HTML entities and normalize whitespace
        let cleaned = html_escape::decode_html_entities(body);
        let cleaned = cleaned.replace('\n', "").replace('\r', "");

        // Extract data-state JSON from the ImagesApp element
        let data_state_json = match self.data_state_re.captures(&cleaned) {
            Some(caps) => caps.get(1).unwrap().as_str().to_string(),
            None => return Ok(vec![]),
        };

        // Prepare JSON for parsing (handle &quot;, HTML tags, entities)
        let data_state_json = self.prepare_json(&data_state_json);

        let data_state: Value = serde_json::from_str(&data_state_json).map_err(|e| {
            ImageDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("Failed to parse data-state JSON: {}", e),
            }
        })?;

        // Navigate to entities: initialState.serpList.items.entities
        let entities = data_state
            .get("initialState")
            .and_then(|v| v.get("serpList"))
            .and_then(|v| v.get("items"))
            .and_then(|v| v.get("entities"))
            .and_then(|v| v.as_object());

        let entities = match entities {
            Some(e) => e,
            None => return Ok(vec![]),
        };

        let mut image_infos = Vec::new();

        for item in entities.values() {
            if !item.is_object() {
                continue;
            }

            let mut candidate_urls = Vec::new();

            // origUrl
            if let Some(url) = item.get("origUrl").and_then(|v| v.as_str()) {
                if !url.is_empty() && url.starts_with("http") {
                    candidate_urls.push(url.to_string());
                }
            }

            // image (may be protocol-relative)
            if let Some(img) = item.get("image").and_then(|v| v.as_str()) {
                if !img.is_empty() {
                    let url = if img.starts_with("//") {
                        format!("https:{}", img)
                    } else if img.starts_with("http") {
                        img.to_string()
                    } else {
                        continue;
                    };
                    if !candidate_urls.contains(&url) {
                        candidate_urls.push(url);
                    }
                }
            }

            // viewerData.img_href
            if let Some(href) = item
                .get("viewerData")
                .and_then(|v| v.as_object())
                .and_then(|obj| obj.get("img_href"))
                .and_then(|v| v.as_str())
            {
                if !href.is_empty() && !candidate_urls.contains(&href.to_string()) {
                    candidate_urls.push(href.to_string());
                }
            }

            if candidate_urls.is_empty() {
                continue;
            }

            let identifier = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(&candidate_urls[0])
                .to_string();

            image_infos.push(ImageInfo::with_identifier(
                self.source_name(),
                candidate_urls,
                identifier,
            ));
        }

        Ok(image_infos)
    }

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // isize filter (size)
        f.add_rule(FilterRule::with_string_choices(
            "isize",
            vec!["large", "medium", "small", "wallpaper"],
            |v| format!("isize={}", v),
        ));

        // iorient filter (orientation)
        f.add_rule(FilterRule::with_string_choices(
            "iorient",
            vec!["horizontal", "vertical", "square"],
            |v| format!("iorient={}", v),
        ));

        // itype filter (type)
        f.add_rule(FilterRule::with_string_choices(
            "itype",
            vec!["photo", "clipart", "lineart", "face"],
            |v| format!("itype={}", v),
        ));

        // icolor filter (color)
        f.add_rule(FilterRule::with_string_choices(
            "icolor",
            vec![
                "color", "gray", "red", "orange", "yellow", "green",
                "cyan", "blue", "violet", "white", "black",
            ],
            |v| format!("icolor={}", v),
        ));

        // file_type filter
        f.add_rule(FilterRule::with_string_choices(
            "file_type",
            vec!["jpg", "png", "gifan"],
            |v| format!("file_type={}", v),
        ));

        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FilterValue;

    #[test]
    fn test_construct_search_urls() {
        let source = YandexImageSource::new();
        let params = SearchParams {
            search_limits: 60,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("text=cats"));
        assert!(urls[0].url.contains("p=1"));
        assert!(urls[1].url.contains("p=2"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = YandexImageSource::new();
        let data_state = r#"{"initialState":{"serpList":{"items":{"entities":{"img1":{"id":"img1","origUrl":"https://example.com/cat1.jpg","image":"//example.com/thumb1.jpg"},"img2":{"id":"img2","origUrl":"https://example.com/cat2.jpg","viewerData":{"img_href":"https://example.com/cat2_full.jpg"}}}}}}}"#;
        let html = format!(
            r#"<div id="ImagesApp-123" data-state="{}" data-hydrate-priority="low"></div>"#,
            data_state.replace('"', "&quot;"),
        );
        let results = source.parse_search_result(&html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].identifier, "img1");
        assert_eq!(results[0].candidate_download_urls[0], "https://example.com/cat1.jpg");
    }

    #[test]
    fn test_prepare_json_strips_tags() {
        let source = YandexImageSource::new();
        // Verify that HTML tags are stripped from the JSON
        let input = r#""text":"24 results for <b>real cat</b> in category""#;
        let output = source.prepare_json(input);
        assert!(!output.contains("<b>"));
        assert!(!output.contains("</b>"));
        assert!(output.contains("real cat"));
    }

    #[test]
    fn test_parse_search_result_no_data_state() {
        let source = YandexImageSource::new();
        let html = r#"<html><body>No images app here</body></html>"#;
        let results = source.parse_search_result(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_yandex_filter_isize() {
        let source = YandexImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("isize".to_string(), FilterValue::from("large"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "isize=large");
    }

    #[test]
    fn test_yandex_filter_itype() {
        let source = YandexImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("itype".to_string(), FilterValue::from("photo"));
        let result = filter.apply(&options, "&").unwrap();
        assert_eq!(result, "itype=photo");
    }
}
