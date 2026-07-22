//! Bing image search source.
//!
//! Replaces Python's `BingImageClient`. Uses HTML scraping with CSS selectors
//! to extract image URLs from Bing's async image search endpoint.

use regex::Regex;
use reqwest::header::{HeaderMap, ACCEPT_LANGUAGE, USER_AGENT};
use scraper::{Html, Selector};

use crate::client::ImageSource;
use crate::error::Result;
use crate::filter::{Filter, FilterRule};
use crate::types::{Filters, ImageInfo, SearchParams, SearchUrl};

/// Bing image search source.
pub struct BingImageSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl BingImageSource {
    /// Create a new Bing image source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            ACCEPT_LANGUAGE,
            "zh-CN,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2"
                .parse()
                .unwrap(),
        );
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
}

impl Default for BingImageSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ImageSource for BingImageSource {
    fn source_name(&self) -> &str {
        "bing"
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
        let page_size: usize = 20;
        let num_pages = ((params.search_limits as f64 * 1.2 / page_size as f64).ceil()) as usize;

        let filter_str = self
            .build_filter()
            .apply(filters, "")
            .unwrap_or_default();
        let filter_param = if filter_str.is_empty() {
            String::new()
        } else {
            format!("&qft={}", filter_str)
        };

        (0..num_pages)
            .map(|pn| {
                let url = format!(
                    "https://www.bing.com/images/async?q={}&first={}{}",
                    urlencoding::encode(keyword),
                    pn * page_size,
                    filter_param,
                );
                SearchUrl::new(url)
            })
            .collect()
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<ImageInfo>> {
        let document = Html::parse_document(body);
        let selector = Selector::parse("div.imgpt").unwrap();
        let a_selector = Selector::parse("a").unwrap();
        let re = Regex::new(r#""murl":"(.*?)""#)?;

        let mut results = Vec::new();
        for element in document.select(&selector) {
            if let Some(a) = element.select(&a_selector).next()
                && let Some(m_attr) = a.value().attr("m") {
                    let unescaped = html_escape::decode_html_entities(m_attr);
                    if let Some(caps) = re.captures(&unescaped) {
                        let url = caps[1].trim().to_string();
                        if !url.is_empty() {
                            results.push(ImageInfo::with_identifier(
                                self.source_name(),
                                vec![url.clone()],
                                url,
                            ));
                        }
                    }
                }
        }

        Ok(results)
    }

    fn build_filter(&self) -> Filter {
        let mut f = Filter::new();

        // Type filter
        f.add_rule(FilterRule::with_string_choices(
            "type",
            vec!["photo", "clipart", "linedrawing", "transparent", "animated"],
            |v| {
                let val = if v == "animated" { "animatedgif" } else { v };
                format!("+filterui:photo-{}", val)
            },
        ));

        // Color filter
        f.add_rule(FilterRule::with_string_choices(
            "color",
            vec![
                "color", "blackandwhite", "red", "orange", "yellow", "green", "teal", "blue",
                "purple", "pink", "white", "gray", "black", "brown",
            ],
            |v| match v {
                "color" => "+filterui:color2-color".to_string(),
                "blackandwhite" => "+filterui:color2-bw".to_string(),
                _ => format!("+filterui:color2-FGcls_{}", v.to_uppercase()),
            },
        ));

        // Size filter
        f.add_rule(FilterRule::with_any_string("size", |v| {
            match v {
                "large" | "medium" | "small" => format!("+filterui:imagesize-{}", v),
                "extralarge" => "+filterui:imagesize-wallpaper".to_string(),
                s if s.starts_with('=') => {
                    let dims: Vec<&str> = s[1..].split('x').collect();
                    if dims.len() == 2 {
                        format!("+filterui:imagesize-custom_{}_{}", dims[0], dims[1])
                    } else {
                        format!(
                            "+filterui:imagesize-{}",
                            s // fallback
                        )
                    }
                }
                _ => format!("+filterui:imagesize-{}", v),
            }
        }));

        // License filter
        let license_code = [
            ("creativecommons", "licenseType-Any"),
            ("publicdomain", "license-L1"),
            ("noncommercial", "license-L2_L3_L4_L5_L6_L7"),
            ("commercial", "license-L2_L3_L4"),
            ("noncommercial,modify", "license-L2_L3_L5_L6"),
            ("commercial,modify", "license-L2_L3"),
        ];
        let license_choices: Vec<&str> = license_code.iter().map(|(k, _)| *k).collect();
        let license_map: std::collections::HashMap<&str, &str> = license_code.into_iter().collect();
        f.add_rule(FilterRule::with_string_choices("license", license_choices, move |v| {
            format!(
                "+filterui:{}",
                license_map.get(v).copied().unwrap_or("licenseType-Any")
            )
        }));

        // Layout filter
        f.add_rule(FilterRule::with_string_choices(
            "layout",
            vec!["square", "wide", "tall"],
            |v| format!("+filterui:aspect-{}", v),
        ));

        // People filter
        f.add_rule(FilterRule::with_string_choices(
            "people",
            vec!["face", "portrait"],
            |v| format!("+filterui:face-{}", v),
        ));

        // Date filter
        let date_minutes = [
            ("pastday", "1440"),
            ("pastweek", "10080"),
            ("pastmonth", "43200"),
            ("pastyear", "525600"),
        ];
        let date_choices: Vec<&str> = date_minutes.iter().map(|(k, _)| *k).collect();
        let date_map: std::collections::HashMap<&str, &str> = date_minutes.into_iter().collect();
        f.add_rule(FilterRule::with_string_choices("date", date_choices, move |v| {
            format!(
                "+filterui:age-lt{}",
                date_map.get(v).copied().unwrap_or("1440")
            )
        }));

        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FilterValue;

    #[test]
    fn test_construct_search_urls() {
        let source = BingImageSource::new();
        let params = SearchParams {
            search_limits: 40,
            ..Default::default()
        };
        let urls = source.construct_search_urls("cats", &params, &Filters::new());
        // 40 * 1.2 / 20 = 2.4 -> ceil = 3 pages
        assert_eq!(urls.len(), 3);
        assert!(urls[0].url.contains("q=cats"));
        assert!(urls[0].url.contains("first=0"));
        assert!(urls[1].url.contains("first=20"));
    }

    #[test]
    fn test_parse_search_result() {
        let source = BingImageSource::new();
        let html = r#"
        <div class="imgpt">
            <a m="&quot;murl&quot;:&quot;https://example.com/cat1.jpg&quot;,&quot;turl&quot;:&quot;thumb.jpg&quot;"></a>
        </div>
        <div class="imgpt">
            <a m="&quot;murl&quot;:&quot;https://example.com/cat2.jpg&quot;,&quot;turl&quot;:&quot;thumb2.jpg&quot;"></a>
        </div>
        "#;
        let results = source.parse_search_result(html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].candidate_download_urls[0], "https://example.com/cat1.jpg");
        assert_eq!(results[0].source, "bing");
    }

    #[test]
    fn test_bing_filter_type() {
        let source = BingImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("type".to_string(), FilterValue::from("clipart"));
        let result = filter.apply(&options, "").unwrap();
        assert_eq!(result, "+filterui:photo-clipart");
    }

    #[test]
    fn test_bing_filter_color() {
        let source = BingImageSource::new();
        let filter = source.build_filter();
        let mut options = Filters::new();
        options.insert("color".to_string(), FilterValue::from("red"));
        let result = filter.apply(&options, "").unwrap();
        assert_eq!(result, "+filterui:color2-FGcls_RED");
    }
}
