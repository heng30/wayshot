use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Segment reference used in many tool parameters
#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentRef {
    /// Track index
    pub track_index: usize,
    /// Segment index within the track
    pub segment_index: usize,
}

/// Filter reference used in filter tools
#[derive(Deserialize, JsonSchema, Default)]
pub struct FilterRef {
    /// Track index
    pub track_index: usize,
    /// Segment index within the track
    pub segment_index: usize,
    /// Filter index within the segment's filter list
    pub filter_index: usize,
}

/// Track info returned by list operations
#[derive(Serialize, JsonSchema)]
pub struct TrackInfo {
    pub index: usize,
    pub name: String,
    pub track_type: String,
    pub locked: bool,
    pub hidden: bool,
    pub muted: bool,
    pub segment_count: usize,
    pub duration_ms: u64,
}

/// Segment info returned by list operations
#[derive(Serialize, JsonSchema)]
pub struct SegmentInfo {
    pub index: usize,
    pub timeline_offset_ms: u64,
    pub duration_ms: u64,
    pub source_offset_ms: u64,
    pub original_duration_ms: u64,
    pub visible: bool,
    pub audio_muted: bool,
    pub playback_speed: f32,
    pub source_path: Option<String>,
}

/// Filter info returned by list operations
#[derive(Serialize, JsonSchema)]
pub struct FilterInfo {
    pub index: usize,
    pub filter_type: String,
    pub name: String,
    pub enabled: bool,
    pub detail: String,
}

/// Project status info
#[derive(Serialize, JsonSchema)]
pub struct ProjectStatus {
    pub is_open: bool,
    pub project_path: Option<String>,
    pub is_unsaved: bool,
    pub track_count: usize,
    pub total_segments: usize,
    pub duration_ms: u64,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// Task info for long-running operations
#[derive(Serialize, JsonSchema)]
pub struct TaskInfo {
    pub task_id: String,
    pub description: String,
    pub status: String,
    pub progress: f32,
}
