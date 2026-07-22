//! Core shared types for the imagedl-rs library.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Metadata about a single image found through search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Which source produced this result (e.g. "bing", "baidu").
    pub source: String,
    /// The URL that was ultimately used for download (set after download succeeds).
    pub download_url: Option<String>,
    /// Ordered list of candidate URLs to try for download, highest quality first.
    pub candidate_download_urls: Vec<String>,
    /// Human-readable description of the image.
    #[serde(default)]
    pub description: String,
    /// Unique identifier within the source (used for deduplication).
    pub identifier: String,
    /// Directory where the image file will be saved.
    #[serde(default = "default_work_dir")]
    pub work_dir: PathBuf,
    /// Detected file extension (e.g. "jpg", "png"). Set after download.
    pub ext: Option<String>,
    /// Base filename without extension (e.g. "00000001"). Set during search post-processing.
    pub save_name: Option<String>,
    /// Full path to the saved file. Set after download.
    pub save_path: Option<PathBuf>,
    /// Source-specific extra metadata (replaces Python's dynamic `extra` dict).
    #[serde(default)]
    pub extra: serde_json::Value,
}

fn default_work_dir() -> PathBuf {
    PathBuf::from("imagedl_outputs")
}

impl ImageInfo {
    /// Create a new ImageInfo with the given source and candidate URLs.
    /// The identifier defaults to the first candidate URL.
    pub fn new(source: impl Into<String>, candidate_download_urls: Vec<String>) -> Self {
        let source = source.into();
        let identifier = candidate_download_urls.first().cloned().unwrap_or_default();
        Self {
            source,
            download_url: None,
            candidate_download_urls,
            description: String::new(),
            identifier,
            work_dir: default_work_dir(),
            ext: None,
            save_name: None,
            save_path: None,
            extra: serde_json::Value::Null,
        }
    }

    /// Create an ImageInfo with a specific identifier (for deduplication).
    pub fn with_identifier(
        source: impl Into<String>,
        candidate_download_urls: Vec<String>,
        identifier: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            ..Self::new(source, candidate_download_urls)
        }
    }
}

/// Parameters controlling search behavior.
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Approximate maximum number of images desired from this source.
    pub search_limits: usize,
    /// Base directory for saving search results and downloads.
    pub work_dir: PathBuf,
    /// Maximum number of concurrent HTTP requests for searching.
    pub concurrency: usize,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            search_limits: 1000,
            work_dir: PathBuf::from("imagedl_outputs"),
            concurrency: 5,
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
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            concurrency: 5,
            work_dir: None,
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
        }
    }

    /// Create a POST search URL with a body.
    pub fn post(url: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Post,
            body: Some(body.into()),
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
pub struct DownloadedInfo {
    /// The image metadata.
    pub image_info: ImageInfo,
    /// The downloaded image data.
    pub data: bytes::Bytes,
    /// The detected image format.
    pub format: ImageFormat,
}

/// Detected image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
    Bmp,
    Tiff,
    Ico,
    Avif,
    Heif,
}

impl ImageFormat {
    /// Get the common file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::WebP => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tif",
            Self::Ico => "ico",
            Self::Avif => "avif",
            Self::Heif => "heif",
        }
    }
}

/// Source-specific filter options, passed as an untyped map.
/// Each source interprets the keys according to its own filter rules.
pub type Filters = std::collections::HashMap<String, FilterValue>;

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
