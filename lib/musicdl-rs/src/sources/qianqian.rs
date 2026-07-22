//! Qianqian music source: http://music.taihe.com/
//!
//! Replaces Python's `QianqianMusicClient`. Uses Qianqian's search API
//! for search and official APIs for download URL resolution.
//!
//! The API requires request signing: parameters are sorted alphabetically,
//! concatenated as `key1=value1&key2=value2...`, appended with the secret key,
//! and then MD5-hashed to produce the `sign` parameter.

use async_trait::async_trait;
use reqwest::header::{HeaderMap, USER_AGENT};

use crate::client::http::HttpClient;
use crate::client::MusicSource;
use crate::detect::is_valid_audio_ext;
use crate::error::{MusicDlError, Result};
use crate::types::{Filters, SearchParams, SearchUrl, SongInfo};
use crate::utils;

/// Qianqian API secret key (extracted from the web player JS bundle).
const QIANQIAN_SECRET: &str = "0b50b02fd0d73a9c4c8c3a781c30845f";
/// Qianqian API app ID.
const QIANQIAN_APPID: &str = "16073360";

/// Qianqian (Baidu) music source.
pub struct QianqianMusicSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl QianqianMusicSource {
    /// Create a new Qianqian music source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert("Referer", "https://music.91q.com/player".parse().unwrap());
        search_headers.insert("from", "web".parse().unwrap());

        let mut download_headers = HeaderMap::new();
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

    /// Create the signed query string for a Qianqian API request.
    ///
    /// Algorithm (from the web player JS):
    /// 1. Add `timestamp = current_unix_time` to the params
    /// 2. Sort keys alphabetically
    /// 3. Concatenate as `key1=value1&key2=value2...` (raw values, NOT URL-encoded)
    /// 4. Append the secret key
    /// 5. `sign = MD5(concatenated_string)`
    fn create_sign(params: &mut Vec<(&str, String)>) -> (u64, String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        params.push(("timestamp", timestamp.to_string()));

        // Sort by key
        params.sort_by(|a, b| a.0.cmp(b.0));

        // Build concatenated string: key1=value1&key2=value2...
        let concatenated: String = params
            .iter()
            .enumerate()
            .map(|(i, (k, v))| {
                if i == 0 {
                    format!("{}={}", k, v)
                } else {
                    format!("&{}={}", k, v)
                }
            })
            .collect();

        // Append secret and MD5
        let sign_input = format!("{}{}", concatenated, QIANQIAN_SECRET);
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(sign_input.as_bytes());
        let result = hasher.finalize();
        let sign = result.iter().map(|b| format!("{:02x}", b)).collect::<String>();

        (timestamp, sign)
    }
}

impl Default for QianqianMusicSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicSource for QianqianMusicSource {
    fn source_name(&self) -> &str {
        "qianqian"
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
        let page_size = params.search_size_per_page;
        let base_url = "https://music.91q.com/v1/search";
        let mut urls = Vec::new();
        let mut count = 0;

        while params.search_limits > count {
            let page_no = count / page_size + 1;

            let mut sign_params: Vec<(&str, String)> = vec![
                ("appid", QIANQIAN_APPID.to_string()),
                ("pageNo", page_no.to_string()),
                ("pageSize", page_size.to_string()),
                ("type", "1".to_string()),
                ("word", keyword.to_string()),
            ];
            let (timestamp, sign) = Self::create_sign(&mut sign_params);

            let url = format!(
                "{}?word={}&type=1&pageNo={}&pageSize={}&appid={}&timestamp={}&sign={}",
                base_url,
                urlencoding::encode(keyword),
                page_no,
                page_size,
                QIANQIAN_APPID,
                timestamp,
                sign,
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

        // Check for API errors
        if data.get("state").and_then(|v| v.as_bool()) == Some(false) {
            let errmsg = data
                .get("errmsg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            let errno = data
                .get("errno")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            return Err(MusicDlError::Parse {
                origin: self.source_name().to_string(),
                reason: format!("API error {}: {}", errno, errmsg),
            });
        }

        let results = data
            .get("data")
            .and_then(|d| d.get("typeTrack"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| MusicDlError::Parse {
                origin: self.source_name().to_string(),
                reason: "Missing data.typeTrack".to_string(),
            })?;

        let songs = results
            .iter()
            .map(|item| {
                let identifier = item
                    .get("id")
                    .or(item.get("TSID"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let song_name = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let singers = item
                    .get("artist")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .or_else(|| {
                        item.get("artistName")
                            .and_then(|v| v.as_str())
                            .map(utils::legalize_string)
                    });

                let album = item
                    .get("albumTitle")
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let cover_url = item
                    .get("pic")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let lyric_url = item
                    .get("lyric")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                let duration_s = item
                    .get("duration")
                    .and_then(|v| v.as_u64())
                    .map(|ms| ms / 1000);

                SongInfo {
                    source: self.source_name().to_string(),
                    song_name,
                    singers,
                    album,
                    cover_url,
                    identifier,
                    duration_s,
                    duration: duration_s.map(utils::seconds_to_hms),
                    raw_data: serde_json::json!({"search": item, "lyric_url": lyric_url}),
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
        let song_id = song_info.identifier.clone();

        // Try Qianqian's play URL API with signed requests
        let qualities = ["3000", "320", "128"];

        for quality in qualities {
            let mut sign_params: Vec<(&str, String)> = vec![
                ("appid", QIANQIAN_APPID.to_string()),
                ("bitrate", quality.to_string()),
                ("TSID", song_id.clone()),
            ];
            let (timestamp, sign) = Self::create_sign(&mut sign_params);

            let url = format!(
                "https://music.91q.com/v1/song/tracklink?TSID={}&appid={}&bitrate={}&timestamp={}&sign={}",
                urlencoding::encode(&song_id),
                QIANQIAN_APPID,
                quality,
                timestamp,
                sign,
            );

            match http
                .get_json::<serde_json::Value>(&url, self.search_headers.clone())
                .await
            {
                Ok(result) => {
                    // Check for API errors
                    if result.get("state").and_then(|v| v.as_bool()) == Some(false) {
                        continue;
                    }

                    let download_url = utils::extract_str(&result, &["data", "path"])
                        .unwrap_or("")
                        .to_string();

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

                        // Extract cover from tracklink response if not already set
                        if song_info.cover_url.is_none() {
                            let pic = result
                                .get("data")
                                .and_then(|d| d.get("pic"))
                                .and_then(|v| v.as_str());
                            if let Some(pic_url) = pic {
                                song_info.cover_url = Some(pic_url.to_string());
                            }
                        }
                        break;
                    }
                }
                Err(e) => {
                    log::debug!(
                        "[qianqian] Tracklink API failed for {} quality {}: {}",
                        song_id,
                        quality,
                        e
                    );
                    continue;
                }
            }
        }

        if song_info.download_url.is_none() {
            return Err(MusicDlError::DownloadUrlResolution {
                identifier: song_id,
            });
        }

        // Fetch lyrics from lyric URL (plain text, not LRC format)
        if song_info.lyric.is_none() {
            let lyric_url = song_info.raw_data
                .get("lyric_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !lyric_url.is_empty() && lyric_url.starts_with("http") {
                if let Ok(text) = http.get_text(&lyric_url, self.search_headers.clone()).await {
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        song_info.lyric = Some(text);
                    }
                }
            }
        }

        Ok(())
    }
}
