//! Configuration for similar-video-segment.

use std::{path::PathBuf, time::Duration};

/// Configuration for video scanning and segment export.
#[derive(Debug, Clone)]
pub struct SimilarVideoConfig {
    /// Sample every N-th frame when scanning video.
    /// Lower values give better accuracy but slower scanning.
    /// Default: 10
    pub sample_interval: u32,

    /// Cosine similarity threshold for a match.
    /// Values range from -1.0 to 1.0; higher means more similar.
    /// Default: 0.75
    pub similarity_threshold: f32,

    /// Time gap for merging consecutive matches.
    /// When multiple matching frames are within this duration of each other,
    /// they are treated as the same visual region and only the best match is kept.
    /// This prevents a single static scene from producing many duplicate segments.
    /// Default: 5 seconds
    pub merge_gap_duration: Duration,

    /// Duration to keep before each match point when exporting.
    /// Default: 5 seconds
    pub before_duration: Duration,

    /// Duration to keep after each match point when exporting.
    /// Default: 5 seconds
    pub after_duration: Duration,

    /// Output directory for exported video segments.
    /// Default: current directory
    pub output_dir: PathBuf,
}

impl Default for SimilarVideoConfig {
    fn default() -> Self {
        Self {
            sample_interval: 10,
            similarity_threshold: 0.75,
            merge_gap_duration: Duration::from_secs(5),
            before_duration: Duration::from_secs(5),
            after_duration: Duration::from_secs(5),
            output_dir: PathBuf::from("."),
        }
    }
}
