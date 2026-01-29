//! Video editing operations
//!
//! This module provides core video editing functionality including:
//! - Trimming/cutting videos
//! - Concatenating/merging videos
//! - Splitting videos
//! - Speed control
//! - Crossfading

pub mod trim;
pub mod concat;
pub mod split;
pub mod speed;

pub use trim::{trim_video, TrimConfig, extract_segment};
pub use concat::{concat_videos, ConcatConfig, concat_videos_simple};
pub use split::{split_video, SplitConfig, split_equal, split_by_duration, split_at_points};
pub use speed::{change_speed, SpeedConfig, speed_up, slow_down, reverse_video, SpeedFactor};
