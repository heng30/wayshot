//! Netease music source: https://music.163.com/
//!
//! Replaces Python's `NeteaseMusicClient`. Uses Netease's cloud search API
//! for search and the official player URL API for download URL resolution.

use async_trait::async_trait;
use reqwest::header::{HeaderMap, REFERER, USER_AGENT};

use crate::client::http::HttpClient;
use crate::client::MusicSource;
use crate::detect::is_valid_audio_ext;
use crate::error::{MusicDlError, Result};
use crate::types::{Filters, SearchParams, SearchUrl, SongInfo};
use crate::utils;

/// Netease music source.
pub struct NeteaseMusicSource {
    search_headers: HeaderMap,
    parse_headers: HeaderMap,
    download_headers: HeaderMap,
}

impl NeteaseMusicSource {
    /// Create a new Netease music source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );
        search_headers.insert(
            REFERER,
            "https://music.163.com/".parse().unwrap(),
        );

        let download_headers = HeaderMap::new();

        Self {
            search_headers,
            parse_headers: HeaderMap::new(),
            download_headers,
        }
    }

    /// Parse download URL using Netease's official player URL API.
    ///
    /// Tries quality levels from highest to lowest until a valid URL is found.
    async fn parse_with_official_api(
        &self,
        search_result: &serde_json::Value,
        http: &HttpClient,
    ) -> Result<SongInfo> {
        let song_id = search_result
            .get("id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| MusicDlError::DownloadUrlResolution {
                identifier: "unknown".to_string(),
            })?;

        // Try different bitrates: 320kbps, 192kbps, 128kbps
        let bitrates = [320000, 192000, 128000];

        for br in bitrates {
            let url = format!(
                "https://music.163.com/api/song/enhance/player/url?id={}&ids=%5B{}%5D&br={}",
                song_id, song_id, br
            );

            match http
                .get_json::<serde_json::Value>(&url, self.search_headers.clone())
                .await
            {
                Ok(result) => {
                    let download_url = result
                        .get("data")
                        .and_then(|d| d.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|item| item.get("url"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !download_url.starts_with("http") {
                        continue;
                    }

                    // Convert http:// to https:// if possible
                    let download_url = if download_url.starts_with("http://") {
                        format!("https://{}", &download_url[7..])
                    } else {
                        download_url
                    };

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
                        let song_name = search_result
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(utils::legalize_string);

                        let singers = search_result
                            .get("ar")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .map(|s| utils::legalize_string(&s));

                        let album = search_result
                            .get("al")
                            .and_then(|al| al.get("name"))
                            .and_then(|v| v.as_str())
                            .map(utils::legalize_string);

                        let cover_url = search_result
                            .get("al")
                            .and_then(|al| al.get("picUrl"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        let duration_s = search_result
                            .get("dt")
                            .and_then(|v| v.as_u64())
                            .map(|ms| ms / 1000);

                        return Ok(SongInfo {
                            source: self.source_name().to_string(),
                            song_name,
                            singers,
                            album,
                            ext: status.ext.clone(),
                            file_size_bytes: status.file_size_bytes,
                            file_size: status.file_size.clone(),
                            download_url: status.download_url.clone(),
                            download_url_status: status,
                            identifier: song_id.to_string(),
                            duration_s,
                            duration: duration_s.map(utils::seconds_to_hms),
                            cover_url,
                            raw_data: serde_json::json!({
                                "search": search_result,
                                "download": result,
                                "bitrate": br,
                            }),
                            ..Default::default()
                        });
                    }
                }
                Err(e) => {
                    log::debug!(
                        "[netease] Player URL API failed for {} bitrate {}: {}",
                        song_id,
                        br,
                        e
                    );
                    continue;
                }
            }
        }

        Err(MusicDlError::DownloadUrlResolution {
            identifier: song_id.to_string(),
        })
    }
}

impl Default for NeteaseMusicSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicSource for NeteaseMusicSource {
    fn source_name(&self) -> &str {
        "netease"
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
        let base_url = "https://music.163.com/api/cloudsearch/pc";
        let mut urls = Vec::new();
        let mut count = 0;

        while params.search_limits > count {
            let offset = (count / page_size) * page_size;
            let mut form = std::collections::HashMap::new();
            form.insert("s".to_string(), keyword.to_string());
            form.insert("type".to_string(), "1".to_string());
            form.insert("limit".to_string(), page_size.to_string());
            form.insert("offset".to_string(), offset.to_string());
            urls.push(SearchUrl::post_form(base_url, form));
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
            .get("result")
            .and_then(|r| r.get("songs"))
            .and_then(|s| s.as_array())
            .ok_or_else(|| MusicDlError::Parse {
                origin: self.source_name().to_string(),
                reason: "Missing result.songs".to_string(),
            })?;

        let songs = results
            .iter()
            .map(|item| {
                let identifier = item
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .map(|id| id.to_string())
                    .unwrap_or_default();

                let song_name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let singers = item
                    .get("ar")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .map(|s| utils::legalize_string(&s));

                let album = item
                    .get("al")
                    .and_then(|al| al.get("name"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let cover_url = item
                    .get("al")
                    .and_then(|al| al.get("picUrl"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let duration_s = item
                    .get("dt")
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
        let search_result = &song_info.raw_data["search"];

        match self.parse_with_official_api(search_result, http).await {
            Ok(resolved) => {
                song_info.download_url = resolved.download_url;
                song_info.download_url_status = resolved.download_url_status;
                song_info.ext = resolved.ext;
                song_info.duration_s = resolved.duration_s;
                song_info.duration = resolved.duration;
                if resolved.cover_url.is_some() {
                    song_info.cover_url = resolved.cover_url;
                }
                song_info.lyric = resolved.lyric;
                song_info.file_size_bytes = resolved.file_size_bytes;
                song_info.file_size = resolved.file_size;
                if resolved.song_name.is_some() {
                    song_info.song_name = resolved.song_name;
                }
                if resolved.singers.is_some() {
                    song_info.singers = resolved.singers;
                }
                if resolved.album.is_some() {
                    song_info.album = resolved.album;
                }
            }
            Err(e) => return Err(e),
        }

        // Fetch lyrics if not already present
        if song_info.lyric.is_none() {
            let song_id = &song_info.identifier;
            let lyric_url = format!(
                "https://music.163.com/api/song/lyric?id={}&lv=1&tv=-1",
                song_id
            );
            if let Ok(result) = http
                .get_json::<serde_json::Value>(&lyric_url, self.search_headers.clone())
                .await
            {
                let lrc_text = result
                    .get("lrc")
                    .and_then(|l| l.get("lyric"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if let Some(cleaned) = utils::clean_lrc(lrc_text) {
                    song_info.lyric = Some(cleaned);
                }
            }
        }

        Ok(())
    }
}
