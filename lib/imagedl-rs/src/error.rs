//! Error types for the imagedl-rs library.

use thiserror::Error;

/// Convenience Result type alias used throughout the library.
pub type Result<T> = std::result::Result<T, ImageDlError>;

/// The main error type for all imagedl operations.
#[derive(Error, Debug)]
pub enum ImageDlError {
    /// An HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Failed to parse a search result from a source.
    #[error("Failed to parse search result from {origin}: {reason}")]
    Parse {
        /// The source name that produced the parse error.
        origin: String,
        /// The reason the parse failed.
        reason: String,
    },

    /// Image format detection failed for a downloaded image.
    #[error("Image format detection failed for URL: {url}")]
    FormatDetection {
        /// The URL that was downloaded.
        url: String,
    },

    /// An I/O error occurred (e.g., writing a file to disk).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The requested source name is not registered.
    #[error("Unknown source: {0}")]
    UnknownSource(String),

    /// A filter validation or formatting error.
    #[error("Filter error: {0}")]
    Filter(String),

    /// All candidate download URLs failed for an image.
    #[error("All candidate URLs failed for image: {identifier}")]
    AllCandidatesFailed {
        /// The image identifier.
        identifier: String,
    },

    /// A JSON parsing error.
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// A URL parsing error.
    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    /// A regex error.
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    /// A generic error with a descriptive message.
    #[error("{0}")]
    Other(String),
}
