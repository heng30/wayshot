pub mod downloader;

pub use downloader::{DownloadStatus, Downloader};

pub type Result<T> = std::result::Result<T, DownloadError>;

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("HTTP request {url} failed. Error: {error}")]
    RequestError { error: reqwest::Error, url: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to get content length from response")]
    ContentLengthError,

    #[error("Download operation cancelled")]
    Cancelled,

    #[error("Download incomplete: {downloaded}/{total} bytes. Error: {error}")]
    IncompleteDownload {
        error: String,
        downloaded: u64,
        total: u64,
    },

    #[error("Failed to create file: {path}. Error: {error}")]
    FileCreateError { error: std::io::Error, path: String },
}
