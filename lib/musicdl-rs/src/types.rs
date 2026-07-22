//! Core shared types for the musicdl-rs library.
use crate::utils;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

/// Protocol for downloading a song.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadProtocol {
    #[default]
    Http,
    Hls,
}

/// Status of download URL resolution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadUrlStatus {
    /// Whether the URL is reachable and returns valid audio data.
    pub ok: bool,
    /// Detected audio file extension (e.g. "mp3", "flac").
    pub ext: Option<String>,
    /// File size in bytes.
    pub file_size_bytes: Option<u64>,
    /// Human-readable file size (e.g. "4.20 MB").
    pub file_size: Option<String>,
    /// The verified download URL.
    pub download_url: Option<String>,
}

/// Metadata about a single song found through search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongInfo {
    // --- Source identification ---
    /// Which source produced this result (e.g. "netease", "migu").
    pub source: String,
    /// Root source (for third-party API results that proxy another source).
    #[serde(default)]
    pub root_source: Option<String>,

    // --- Song metadata ---
    /// Song title.
    pub song_name: Option<String>,
    /// Artist name(s), comma-separated.
    pub singers: Option<String>,
    /// Album name.
    pub album: Option<String>,
    /// Audio file extension (e.g. "mp3", "flac").
    pub ext: Option<String>,
    /// File size in bytes.
    pub file_size_bytes: Option<u64>,
    /// Human-readable file size (e.g. "4.20 MB").
    pub file_size: Option<String>,
    /// Duration in seconds.
    pub duration_s: Option<u64>,
    /// Human-readable duration (e.g. "3:45").
    pub duration: Option<String>,
    /// Audio bitrate in kbps.
    pub bitrate: Option<u32>,
    /// Audio codec name.
    pub codec: Option<String>,
    /// Sample rate in Hz.
    pub samplerate: Option<u32>,
    /// Number of audio channels.
    pub channels: Option<u32>,

    // --- Lyrics and cover ---
    /// LRC-format lyrics.
    pub lyric: Option<String>,
    /// Cover art image URL.
    pub cover_url: Option<String>,

    // --- Episodes (for audiobook/FM sources like Ximalaya) ---
    /// Episodes list, each item is a SongInfo object.
    #[serde(default)]
    pub episodes: Option<Vec<SongInfo>>,

    // --- Download URL resolution ---
    /// The resolved download URL.
    pub download_url: Option<String>,
    /// Status of download URL resolution.
    #[serde(default)]
    pub download_url_status: DownloadUrlStatus,
    /// Download protocol (HTTP or HLS).
    #[serde(default)]
    pub protocol: DownloadProtocol,
    /// Pre-downloaded content (some APIs return audio data directly).
    /// Skipped during serialization since bytes::Bytes doesn't impl Serialize.
    #[serde(skip)]
    pub downloaded_contents: Option<bytes::Bytes>,
    /// Streaming download chunk size in bytes.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    // --- Source-specific download headers/cookies ---
    /// Override download headers for this specific song.
    /// Skipped during serialization since HeaderMap doesn't impl Serialize.
    #[serde(skip)]
    pub default_download_headers: Option<HeaderMap>,
    /// Override download cookies for this specific song.
    #[serde(default)]
    pub default_download_cookies: Option<HashMap<String, String>>,

    // --- File system ---
    /// Directory where the song file will be saved.
    #[serde(default = "default_work_dir")]
    pub work_dir: PathBuf,
    /// Full path to the saved file. Set after download.
    pub save_path: Option<PathBuf>,
    /// Base filename without extension. Set during search post-processing.
    pub save_name: Option<String>,

    // --- Deduplication ---
    /// Unique identifier within the source (e.g. song ID, content ID, hash).
    pub identifier: String,

    // --- Raw API data for debugging ---
    /// Raw data from the search and download APIs.
    #[serde(default)]
    pub raw_data: serde_json::Value,
}

fn default_work_dir() -> PathBuf {
    PathBuf::from("musicdl_outputs")
}

fn default_chunk_size() -> usize {
    1024 * 1024 // 1MB
}

impl SongInfo {
    /// Create a new SongInfo with the given source and identifier.
    pub fn new(source: impl Into<String>, identifier: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            identifier: identifier.into(),
            work_dir: default_work_dir(),
            chunk_size: default_chunk_size(),
            protocol: DownloadProtocol::Http,
            ..Default::default()
        }
    }

    /// Whether this song has a valid, downloadable URL.
    pub fn with_valid_download_url(&self) -> bool {
        if let Some(episodes) = &self.episodes {
            return episodes.iter().all(|eps| eps.with_valid_download_url());
        }
        let has_valid_url = self
            .download_url
            .as_ref()
            .map(|u| u.starts_with("http"))
            .unwrap_or(false);
        let has_downloaded = self.downloaded_contents.is_some();
        let is_downloadable = self.download_url_status.ok;
        has_downloaded || (has_valid_url && is_downloadable)
    }

    /// Compute the save path from song metadata if not already set.
    pub fn compute_save_path(&mut self) {
        if self.save_path.is_some() {
            return;
        }
        let name = self.song_name.as_deref().unwrap_or("unknown");
        let singers = self.singers.as_deref().unwrap_or("unknown");
        let ext = self
            .ext
            .as_deref()
            .map(|e| e.trim_start_matches('.'))
            .unwrap_or("mp3");
        let filename = format!("{} - {}.{}", name, singers, ext);
        let filename = utils::legalize_string(&filename);
        let path = self.work_dir.join(&filename);
        self.save_path = Some(path);
    }
}

impl Default for SongInfo {
    fn default() -> Self {
        Self {
            source: String::new(),
            root_source: None,
            song_name: None,
            singers: None,
            album: None,
            ext: None,
            file_size_bytes: None,
            file_size: None,
            duration_s: None,
            duration: None,
            bitrate: None,
            codec: None,
            samplerate: None,
            channels: None,
            lyric: None,
            cover_url: None,
            episodes: None,
            download_url: None,
            download_url_status: DownloadUrlStatus::default(),
            protocol: DownloadProtocol::Http,
            downloaded_contents: None,
            chunk_size: default_chunk_size(),
            default_download_headers: None,
            default_download_cookies: None,
            work_dir: default_work_dir(),
            save_path: None,
            save_name: None,
            identifier: String::new(),
            raw_data: serde_json::Value::Null,
        }
    }
}

/// Parameters controlling search behavior.
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Approximate maximum number of songs desired from this source.
    pub search_limits: usize,
    /// Number of results per search page.
    pub search_size_per_page: usize,
    /// Whether to strictly limit results to search_size_per_page per page.
    pub strict_limit_search_size_per_page: bool,
    /// Base directory for saving search results and downloads.
    pub work_dir: PathBuf,
    /// Maximum number of concurrent HTTP requests for searching.
    pub concurrency: usize,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            search_limits: 5,
            search_size_per_page: 10,
            strict_limit_search_size_per_page: true,
            work_dir: PathBuf::from("musicdl_outputs"),
            concurrency: 5,
        }
    }
}

/// Flags controlling what content to download.
#[derive(Debug, Clone, Copy)]
pub struct DownloadContent {
    /// Download the audio file.
    pub audio: bool,
    /// Download the cover image.
    pub cover: bool,
    /// Include the lyrics text.
    pub lyric: bool,
}

impl Default for DownloadContent {
    fn default() -> Self {
        Self {
            audio: true,
            cover: true,
            lyric: true,
        }
    }
}

impl DownloadContent {
    /// Create a new DownloadContent with all flags enabled.
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a new DownloadContent with only audio enabled.
    pub fn audio_only() -> Self {
        Self {
            audio: true,
            cover: false,
            lyric: false,
        }
    }
}

/// Parameters controlling download behavior.
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// Maximum number of concurrent downloads.
    pub concurrency: usize,
    /// Base directory override (if different from search-time work_dir).
    pub work_dir: Option<PathBuf>,
    /// Whether to auto-supplement song metadata (write tags, lyrics) after download.
    pub auto_supplement_song: bool,
    /// Streaming download chunk size in bytes.
    pub chunk_size: usize,
    /// What content to download (audio, cover, lyric).
    pub content: DownloadContent,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            concurrency: 5,
            work_dir: None,
            auto_supplement_song: true,
            chunk_size: 1024 * 1024,
            content: DownloadContent::default(),
        }
    }
}

/// A URL to fetch during search, with optional metadata about HTTP method.
#[derive(Debug, Clone)]
pub struct SearchUrl {
    /// The URL to request.
    pub url: String,
    /// HTTP method to use.
    pub method: HttpMethod,
    /// Request body for POST requests.
    pub body: Option<String>,
    /// POST form data (key-value pairs).
    pub form: Option<HashMap<String, String>>,
    /// POST JSON body.
    pub json: Option<serde_json::Value>,
}

/// HTTP method for search requests.
#[derive(Debug, Clone, Copy, Default)]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
}

impl SearchUrl {
    /// Create a simple GET search URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Get,
            body: None,
            form: None,
            json: None,
        }
    }

    /// Create a POST search URL with form data.
    pub fn post_form(url: impl Into<String>, form: HashMap<String, String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Post,
            body: None,
            form: Some(form),
            json: None,
        }
    }

    /// Create a POST search URL with a JSON body.
    pub fn post_json(url: impl Into<String>, json: serde_json::Value) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Post,
            body: None,
            form: None,
            json: Some(json),
        }
    }
}

impl From<String> for SearchUrl {
    fn from(url: String) -> Self {
        Self::new(url)
    }
}

impl From<&str> for SearchUrl {
    fn from(url: &str) -> Self {
        Self::new(url)
    }
}

/// The result of a successful download.
#[derive(Debug, Clone)]
pub struct DownloadedSongInfo {
    /// The song metadata.
    pub song_info: SongInfo,
    /// The downloaded audio data.
    pub data: bytes::Bytes,
    /// The detected audio format.
    pub format: AudioFormat,
    /// Downloaded cover image bytes (if cover_url was present and cover was requested).
    pub cover_data: Option<bytes::Bytes>,
    /// Lyric text (if lyric was present and requested).
    pub lyric_data: Option<String>,
}

/// Detected audio format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Mp3,
    Flac,
    Aac,
    M4a,
    Ogg,
    Opus,
    Wav,
    Wma,
    Ape,
    Unknown,
}

impl AudioFormat {
    /// Get the common file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::M4a => "m4a",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Wav => "wav",
            Self::Wma => "wma",
            Self::Ape => "ape",
            Self::Unknown => "bin",
        }
    }
}

/// Source-specific filter options, passed as an untyped map.
/// Each source interprets the keys according to its own filter rules.
pub type Filters = HashMap<String, FilterValue>;

/// A value that can be passed as a filter option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    /// A string filter value.
    String(String),
    /// An integer filter value.
    Int(i64),
    /// A boolean filter value.
    Bool(bool),
}

impl FilterValue {
    /// Get the value as a string slice, if it is a String variant.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get the value as an i64, if it is an Int variant.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }
}

impl From<String> for FilterValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for FilterValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<i64> for FilterValue {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<bool> for FilterValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}
