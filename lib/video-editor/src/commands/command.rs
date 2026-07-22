use crate::{Result, tracks::manager::Manager};

#[derive(Clone, Debug, Default, derive_setters::Setters)]
#[setters(prefix = "with_")]
pub struct AffectedSegment {
    pub track_index: usize,
    pub segment_index: usize,
    // (left_thumbnail, right_thumbnail) - which thumbnails need refresh
    pub update_thumbnail: (bool, bool),
    // Whether audio sample preview needs refresh
    pub update_audio_sample: bool,
}

impl AffectedSegment {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            update_thumbnail: (false, false),
            update_audio_sample: false,
        }
    }

    pub fn with_left_thumbnail(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            update_thumbnail: (true, false),
            update_audio_sample: true,
        }
    }

    pub fn with_right_thumbnail(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            update_thumbnail: (false, true),
            update_audio_sample: true,
        }
    }

    pub fn with_both_thumbnails(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            update_thumbnail: (true, true),
            update_audio_sample: true,
        }
    }

    pub fn with_audio_only(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            update_thumbnail: (false, false),
            update_audio_sample: true,
        }
    }

    pub fn should_update(&self) -> bool {
        !matches!(self.update_thumbnail, (false, false)) || self.update_audio_sample
    }
}

// Information about segments that need preview refresh after a command
#[derive(Clone, Debug, Default)]
pub struct AffectedSegments {
    pub segments: Vec<AffectedSegment>,

    // Indicates that tracks were added/removed/moved, requiring preview refresh
    pub tracks_changed: bool,
}

impl AffectedSegments {
    pub fn new() -> Self {
        Self {
            segments: vec![],
            tracks_changed: false,
        }
    }

    pub fn add(&mut self, track_index: usize, segment_index: usize) {
        self.segments
            .push(AffectedSegment::new(track_index, segment_index));
    }

    pub fn add_left_thumbnail(&mut self, track_index: usize, segment_index: usize) {
        self.segments.push(AffectedSegment::with_left_thumbnail(
            track_index,
            segment_index,
        ));
    }

    pub fn add_right_thumbnail(&mut self, track_index: usize, segment_index: usize) {
        self.segments.push(AffectedSegment::with_right_thumbnail(
            track_index,
            segment_index,
        ));
    }

    pub fn add_both_thumbnails(&mut self, track_index: usize, segment_index: usize) {
        self.segments.push(AffectedSegment::with_both_thumbnails(
            track_index,
            segment_index,
        ));
    }

    pub fn add_audio_only(&mut self, track_index: usize, segment_index: usize) {
        self.segments
            .push(AffectedSegment::with_audio_only(track_index, segment_index));
    }

    pub fn merge(&mut self, other: AffectedSegments) {
        self.segments.extend(other.segments);
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

pub trait Command: Send + Sync {
    fn describe(&self) -> String;
    fn execute(&mut self, manager: &mut Manager) -> Result<()>;
    fn undo(&mut self, manager: &mut Manager) -> Result<()>;
    fn can_undo(&self) -> bool {
        true
    }

    // Optional: Merge with another command (for optimization)
    //
    // If this command can be merged with another (e.g., multiple small
    // adjustments of the same property), return true and modify self.
    // By default, commands don't merge.
    fn merge(&mut self, _other: Box<dyn Command>) -> Result<bool> {
        Ok(false)
    }

    // Returns segments that need preview refresh after execute()
    // Used by UI layer to reload thumbnails/audio previews after undo/redo
    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments::default()
    }

    // Returns segments that need preview refresh after undo()
    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments::default()
    }
}
