//! Kugou music source: http://www.kugou.com/
//!
//! Replaces Python's `KugouMusicClient`. Uses Kugou's search API for search
//! and the mobile play info API for download URL resolution.

use async_trait::async_trait;
use reqwest::header::{HeaderMap, USER_AGENT};

use crate::client::http::HttpClient;
use crate::client::MusicSource;
use crate::detect::is_valid_audio_ext;
use crate::error::{MusicDlError, Result};
use crate::types::{Filters, SearchParams, SearchUrl, SongInfo};
use crate::utils;

/// Kugou music source.
pub struct KugouMusicSource {
    search_headers: HeaderMap,
    download_headers: HeaderMap,
    parse_headers: HeaderMap,
}

impl KugouMusicSource {
    /// Create a new Kugou music source.
    pub fn new() -> Self {
        let mut search_headers = HeaderMap::new();
        search_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                .parse()
                .unwrap(),
        );

        let download_headers = search_headers.clone();

        let mut parse_headers = HeaderMap::new();
        parse_headers.insert(
            USER_AGENT,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/537.36"
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

impl Default for KugouMusicSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicSource for KugouMusicSource {
    fn source_name(&self) -> &str {
        "kugou"
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
        let base_url = "https://songsearch.kugou.com/song_search_v2";
        let mut urls = Vec::new();
        let mut count = 0;

        while params.search_limits > count {
            let page = count / page_size + 1;
            let url = format!(
                "{}?format=json&keyword={}&platform=WebFilter&page={}&pagesize={}",
                base_url,
                urlencoding::encode(keyword),
                page,
                page_size,
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
            .get("data")
            .and_then(|d| d.get("lists"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| MusicDlError::Parse {
                origin: self.source_name().to_string(),
                reason: "Missing data.lists".to_string(),
            })?;

        let songs = results
            .iter()
            .map(|item| {
                let identifier = item
                    .get("hash")
                    .or(item.get("FileHash"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let song_name = item
                    .get("songname")
                    .or(item.get("SongName"))
                    .or(item.get("songname_original"))
                    .or(item.get("OriSongName"))
                    .or(item.get("filename"))
                    .or(item.get("FileName"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let singers = item
                    .get("singername")
                    .or(item.get("SingerName"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string)
                    .or_else(|| {
                        item.get("singerinfo")
                            .or(item.get("Singers"))
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .map(|s| utils::legalize_string(&s))
                    });

                let album = item
                    .get("album_name")
                    .or(item.get("AlbumName"))
                    .and_then(|v| v.as_str())
                    .map(utils::legalize_string);

                let album_id = item
                    .get("AlbumID")
                    .and_then(|v| v.as_u64())
                    .map(|id| id.to_string());

                let cover_url = item
                    .get("Image")
                    .and_then(|v| v.as_str())
                    .map(|s| s.replace("{size}", "300"))
                    .or_else(|| {
                        item.get("trans_param")
                            .and_then(|tp| tp.get("union_cover"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.replace("{size}", "300"))
                    });

                let filename = item
                    .get("filename")
                    .or(item.get("FileName"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let duration_s = item
                    .get("duration")
                    .or(item.get("Duration"))
                    .and_then(|v| v.as_f64())
                    .map(|d| d as u64)
                    .or_else(|| {
                        item.get("timelen")
                            .and_then(|v| v.as_f64())
                            .map(|d| (d / 1000.0) as u64)
                    });

                SongInfo {
                    source: self.source_name().to_string(),
                    song_name,
                    singers,
                    album,
                    cover_url,
                    identifier,
                    duration_s,
                    duration: duration_s.map(utils::seconds_to_hms),
                    raw_data: serde_json::json!({"search": item, "album_id": album_id, "filename": filename}),
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
        let file_hash = song_info.identifier.clone();
        let album_id = song_info.raw_data
            .get("album_id")
            .and_then(|v| v.as_str())
            .unwrap_or("0");

        // Try Kugou's mobile play info API
        let url = format!(
            "http://m.kugou.com/app/i/getSongInfo.php?cmd=playInfo&hash={}&album_id={}",
            file_hash, album_id
        );

        match http.get_json::<serde_json::Value>(&url, self.parse_headers.clone()).await {
            Ok(result) => {
                let download_url = result
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !download_url.starts_with("http") {
                    let error_msg = result
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("no URL returned");
                    return Err(MusicDlError::DownloadUrlResolution {
                        identifier: format!("{} ({})", file_hash, error_msg),
                    });
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
                    // Update song info from play info response
                    let song_name = result
                        .get("songName")
                        .and_then(|v| v.as_str())
                        .map(utils::legalize_string)
                        .or(song_info.song_name.clone());

                    let singers = result
                        .get("singerName")
                        .or(result.get("author_name"))
                        .and_then(|v| v.as_str())
                        .map(utils::legalize_string)
                        .or(song_info.singers.clone());

                    let album = result
                        .get("album_name")
                        .and_then(|v| v.as_str())
                        .map(utils::legalize_string)
                        .or(song_info.album.clone());

                    let cover_url = result
                        .get("imgUrl")
                        .and_then(|v| v.as_str())
                        .map(|s| s.replace("{size}", "480"))
                        .or_else(|| {
                            result
                                .get("album_img")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        });

                    let duration_s = result
                        .get("timeLength")
                        .and_then(|v| v.as_u64())
                        .or(song_info.duration_s);

                    song_info.download_url = status.download_url.clone();
                    song_info.download_url_status = status;
                    song_info.ext = song_info.download_url_status.ext.clone();
                    song_info.file_size_bytes = song_info.download_url_status.file_size_bytes;
                    song_info.file_size = song_info.download_url_status.file_size.clone();
                    song_info.song_name = song_name;
                    song_info.singers = singers;
                    song_info.album = album;
                    song_info.cover_url = cover_url;
                    song_info.duration_s = duration_s;
                    song_info.duration = duration_s.map(utils::seconds_to_hms);
                }
            }
            Err(e) => {
                log::debug!(
                    "[kugou] Mobile play info API failed for {}: {}",
                    file_hash,
                    e
                );
            }
        }

        // Fetch lyrics if download URL was resolved
        if song_info.download_url.is_some() && song_info.lyric.is_none() {
            let filename = song_info.raw_data
                .get("filename")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let duration = song_info.duration_s.unwrap_or(0);

            // Step 1: Search for lyrics
            let search_url = format!(
                "http://lyrics.kugou.com/search?keyword={}&duration={}&hash={}",
                urlencoding::encode(filename),
                duration * 1000,
                &song_info.identifier,
            );

            if let Ok(search_result) = http.get_json::<serde_json::Value>(&search_url, self.search_headers.clone()).await {
                if let Some(candidates) = search_result.get("candidates").and_then(|c| c.as_array()) {
                    if let Some(first) = candidates.first() {
                        let lyric_id = first.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let accesskey = first.get("accesskey").and_then(|v| v.as_str()).unwrap_or("");

                        // Step 2: Download lyrics
                        let download_url = format!(
                            "http://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=lrc&charset=utf8",
                            lyric_id, accesskey,
                        );

                        if let Ok(dl_result) = http.get_json::<serde_json::Value>(&download_url, self.search_headers.clone()).await {
                            if let Some(content) = dl_result.get("content").and_then(|v| v.as_str()) {
                                if let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content) {
                                    if let Ok(lrc_text) = String::from_utf8(decoded) {
                                        if let Some(cleaned) = utils::clean_lrc(&lrc_text) {
                                            song_info.lyric = Some(cleaned);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if song_info.download_url.is_none() {
            return Err(MusicDlError::DownloadUrlResolution {
                identifier: file_hash,
            });
        }

        Ok(())
    }
}
