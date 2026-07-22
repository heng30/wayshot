//! Progress reporting types for scanning and exporting.

use std::{path::PathBuf, time::Duration};

/// Progress update during video scanning.
#[derive(Debug, Clone)]
pub struct ScanProgress {
    /// Index of the video currently being scanned (0-based).
    pub video_index: usize,
    /// Total number of videos to scan.
    pub total_videos: usize,
    /// Path of the current video.
    pub current_video: PathBuf,
    /// Number of frames processed so far in the current video.
    pub frames_processed: u64,
    /// Estimated total frames in the current video.
    pub total_frames: u64,
    /// Highest cosine similarity seen so far in the current video.
    pub best_similarity: f32,
    /// Elapsed time since scan started.
    pub elapsed: Duration,
}

impl ScanProgress {
    /// Fraction of the current video that has been scanned (0.0–1.0).
    pub fn fraction(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.frames_processed as f32 / self.total_frames as f32
    }

    /// Overall fraction across all videos (0.0–1.0).
    pub fn overall_fraction(&self) -> f32 {
        if self.total_videos == 0 {
            return 0.0;
        }
        let per_video = 1.0 / self.total_videos as f32;
        (self.video_index as f32 + self.fraction()) * per_video
    }
}

/// Progress update during video segment export.
#[derive(Debug, Clone)]
pub struct ExportProgress {
    /// Index of the segment currently being exported (0-based).
    pub segment_index: usize,
    /// Total number of segments to export.
    pub total_segments: usize,
    /// Number of frames encoded so far for the current segment.
    pub frames_processed: u64,
    /// Total frames to encode for the current segment.
    pub total_frames: u64,
}

impl ExportProgress {
    /// Fraction of the current segment that has been exported (0.0–1.0).
    pub fn fraction(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.frames_processed as f32 / self.total_frames as f32
    }

    /// Overall fraction across all segments (0.0–1.0).
    pub fn overall_fraction(&self) -> f32 {
        if self.total_segments == 0 {
            return 0.0;
        }
        let per_segment = 1.0 / self.total_segments as f32;
        (self.segment_index as f32 + self.fraction()) * per_segment
    }
}
