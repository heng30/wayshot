//! Kuwo music source: http://www.kuwo.cn/
//!
//! Replaces Python's `KuwoMusicClient`. Uses Kuwo's search API for search
//! and the antiserver API for download URL resolution.

use async_trait::async_trait;
use reqwest::header::{HeaderMap, USER_AGENT};

use crate::client::http::HttpClient;
use crate::client::MusicSource;
use crate::detect::is_valid_audio_ext;
use crate::error::{MusicDlError, Result};
use crate::types::{Filters, SearchParams, SearchUrl, SongInfo};
use crate::utils;

/// Kuwo music source.
pub struct KuwoMusicSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
    parse_headers: HeaderMap,
}

impl KuwoMusicSource {
    /// Create a new Kuwo music source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert("Referer", "http://www.kuwo.cn/".parse().unwrap());
        search_headers.insert("csrf", "123456".parse().unwrap());
        search_headers.insert("Cookie", "kw_token=123456".parse().unwrap());

        let download_headers = HeaderMap::new();

        let mut parse_headers = HeaderMap::new();
        parse_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1"
                .parse()
                .unwrap(),
        );

        Self {
            search_headers,
            download_headers,
            parse_headers,
        }
    }
}

impl Default for KuwoMusicSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicSource for KuwoMusicSource {
    fn source_name(&self) -> &str {
        "kuwo"
    }

    fn search_headers(&self) -> HeaderMap {
        self.search_headers.clone()
    }

    fn download_headers(&self) -> HeaderMap {
        self.download_headers.clone()
    }

    fn parse_headers(&self) -> HeaderMap {
        self.parse_headers.clone()
    }

    fn construct_search_urls(
        &self,
        keyword: &str,
        params: &SearchParams,
        _filters: &Filters,
    ) -> Vec<SearchUrl> {
        let page_size = params.search_size_per_page;
        let base_url = "http://www.kuwo.cn/search/searchMusicBykeyWord";
        let mut urls = Vec::new();
        let mut count = 0;

        while params.search_limits > count {
            let pn = count / page_size;
            let url = format!(
                "{}?vipver=1&client=kt&ft=music&cluster=0&strategy=2012&encoding=utf8&rformat=json&mobi=1&issubtitle=1&show_copyright_off=1&pn={}&rn={}&all={}",
                base_url, pn, page_size, urlencoding::encode(keyword),
            );
            urls.push(SearchUrl::new(url));
            count += page_size;
        }

        urls
    }

    fn parse_search_result(&self, body: &str) -> Result<Vec<SongInfo>> {
        let data: serde_json::Value = serde_json::from_str(body).map_err(|e| {
            MusicDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("JSON parse error: {}", e),
            }
        })?;

        let results = data
            .get("abslist")
            .and_then(|l| l.as_array())
            .ok_or_else(|| MusicDlError::Parse {
                origin: self.source_name().to_string(),
                reason: "Missing abslist".to_string(),
            })?;

        let songs = results
            .iter()
            .map(|item| {
                let identifier = item
                    .get("rid")
                    .or(item.get("MUSICRID"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim_start_matches("MUSIC_")
                    .to_string();

                let song_name = item
                    .get("name")
                    .or(item.get("SONGNAME"))
                    .or(item.get("songname"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let singers = item
                    .get("artist")
                    .or(item.get("ARTIST"))
                    .or(item.get("singer"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let album = item
                    .get("album")
                    .or(item.get("ALBUM"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let cover_url = item
                    .get("hts_MVPIC")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        item.get("albumpic")
                            .or(item.get("pic"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    });

                let duration_s = item
                    .get("duration")
                    .or(item.get("songTimeMinutes"))
                    .and_then(|v| v.as_str())
                    .and_then(utils::parse_duration_to_seconds);

                SongInfo {
                    source: self.source_name().to_string(),
                    song_name,
                    singers,
                    album,
                    cover_url,
                    identifier,
                    duration_s,
                    duration: duration_s.map(utils::seconds_to_hms),
                    raw_data: serde_json::json!({"search": item}),
                    ..Default::default()
                }
            })
            .collect();

        Ok(songs)
    }

    async fn parse_download_url(
        &self,
        song_info: &mut SongInfo,
        http: &HttpClient,
    ) -> Result<()> {
        let rid = &song_info.identifier;

        // Try Kuwo's antiserver API (older but reliable)
        let qualities = [
            ("mp3", "convert_url3"),  // 320kbps (returns JSON)
            ("mp3", "convert_url"),   // 128kbps (returns plain URL)
        ];

        for (format, convert_type) in qualities {
            let url = format!(
                "http://antiserver.kuwo.cn/anti.s?useless=0&rid=MUSIC_{}&format={}&type={}&response=url",
                rid, format, convert_type
            );

            match http.get_text(&url, HeaderMap::new()).await {
                Ok(text) => {
                    let text = text.trim().to_string();

                    // Handle both plain URL and JSON response formats
                    let download_url = if text.starts_with("http") {
                        text
                    } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        // JSON format: {"code": 200, "msg": "success", "url": "https://..."}
                        json.get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string()
                    } else {
                        continue;
                    };

                    if !download_url.starts_with("http") {
                        continue;
                    }

                    let status = http
                        .test_audio_link(&download_url, self.download_headers.clone())
                        .await
                        .unwrap_or_default();

                    if status.ok
                        && status
                            .ext
                            .as_deref()
                            .map(is_valid_audio_ext)
                            .unwrap_or(false)
                    {
                        song_info.download_url = status.download_url.clone();
                        song_info.download_url_status = status;
                        song_info.ext = song_info.download_url_status.ext.clone();
                        song_info.file_size_bytes = song_info.download_url_status.file_size_bytes;
                        song_info.file_size = song_info.download_url_status.file_size.clone();
                        break;
                    }
                }
                Err(e) => {
                    log::debug!("[kuwo] Antiserver API failed for {}: {}", rid, e);
                }
            }
        }

        if song_info.download_url.is_none() {
            return Err(MusicDlError::DownloadUrlResolution {
                identifier: rid.clone(),
            });
        }

        // Fetch lyrics and cover from H5 API
        let h5_url = format!(
            "https://m.kuwo.cn/newh5/singles/songinfoandlrc?musicId={}",
            rid
        );
        let mut h5_headers = self.parse_headers.clone();
        h5_headers.insert(
            "Referer",
            format!("https://m.kuwo.cn/yinyue/{}", rid)
                .parse()
                .unwrap(),
        );
        h5_headers.insert("Accept", "application/json".parse().unwrap());

        if let Ok(h5_result) = http
            .get_json::<serde_json::Value>(&h5_url, h5_headers)
            .await
        {
            // Extract cover from songinfo
            if song_info.cover_url.is_none() {
                let pic = h5_result
                    .get("data")
                    .and_then(|d| d.get("songinfo"))
                    .and_then(|s| s.get("pic"))
                    .and_then(|v| v.as_str());
                if let Some(pic_url) = pic {
                    song_info.cover_url = Some(pic_url.to_string());
                }
            }

            // Extract lyrics from lrclist
            if song_info.lyric.is_none() {
                let lrclist = h5_result
                    .get("data")
                    .and_then(|d| d.get("lrclist"));
                if let Some(lrc) = utils::kuwo_lrclist_to_lrc(lrclist.unwrap_or(&serde_json::Value::Null)) {
                    song_info.lyric = Some(lrc);
                }
            }
        }

        Ok(())
    }
}
