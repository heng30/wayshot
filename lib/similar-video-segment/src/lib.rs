//! # similar-video-segment
//!
//! A Rust library for finding similar video frames to a query image
//! and exporting the surrounding video segments.
//!
//! # Example
//!
//! ```rust,ignore
//! use similar_video_segment::{
//!     SimilarVideoConfig, CancellationToken,
//!     scan_videos, export_segments,
//! };
//! use std::path::PathBuf;
//!
//! let config = SimilarVideoConfig::default();
//! let query_image = PathBuf::from("query.png");
//! let videos = vec![PathBuf::from("video1.mp4"), PathBuf::from("video2.mp4")];
//!
//! // Scan for similar frames
//! let matches = scan_videos(&query_image, &videos, &config, None, |_| {})?;
//!
//! // Export segments around each match (with audio preserved)
//! let exported = export_segments(&matches, &config, None, |_| {}, true)?;
//! ```

pub mod cancel;
pub mod config;
pub mod embedder;
pub mod error;
pub mod exporter;
pub mod progress;
pub mod scanner;

pub use cancel::CancellationToken;
pub use config::SimilarVideoConfig;
pub use embedder::ImageEmbedder;
pub use error::{Error, Result};
pub use exporter::export_segments;
pub use progress::{ExportProgress, ScanProgress};
pub use scanner::{MatchResult, scan_videos};
