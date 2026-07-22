use super::{
    command::{AffectedSegments, Command},
    segment::StretchSegmentRightCommand,
};
use crate::{
    Error, Result,
    metadata::Metadata,
    tracks::{
        audio_track::AudioTrack, image_track::ImageTrack, manager::Manager,
        subtitle_track::SubtitleTrack, text_track::TextTrack, track::Track,
        video_track::VideoTrack,
    },
};
use std::{sync::Arc, time::Duration};

pub struct AddTrackCommand {
    track: Track,
    track_index: Option<usize>,
    segment_count: Option<usize>,
}

impl AddTrackCommand {
    pub fn new(track: Track) -> Self {
        Self {
            track,
            track_index: None,
            segment_count: None,
        }
    }
}

impl Command for AddTrackCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let actual_index = manager.add_track(self.track.clone());
        self.track_index = Some(actual_index);
        self.segment_count = manager.get(actual_index).map(|t| t.segments_count());

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let index = self
            .track_index
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;
        manager.remove_track(index)
    }

    fn describe(&self) -> String {
        "Add track".to_string()
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let Some(track_index) = self.track_index else {
            return AffectedSegments::default();
        };
        let Some(segment_count) = self.segment_count else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };

        let is_video_or_image = matches!(self.track, Track::Video(_) | Track::Image(_));
        let mut affected = AffectedSegments::new();
        affected.tracks_changed = true;

        for seg_idx in 0..segment_count {
            if is_video_or_image {
                affected.add_both_thumbnails(track_index, seg_idx);
            } else {
                affected.add_audio_only(track_index, seg_idx);
            }
        }

        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

pub struct InsertTrackCommand {
    track: Track,
    requested_index: usize,
    actual_track_index: Option<usize>,
    segment_count: Option<usize>,
}

impl InsertTrackCommand {
    pub fn new(track: Track, track_index: usize) -> Self {
        Self {
            track,
            requested_index: track_index,
            actual_track_index: None,
            segment_count: None,
        }
    }
}

impl Command for InsertTrackCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let actual_index = manager.insert_track(self.requested_index, self.track.clone())?;
        self.actual_track_index = Some(actual_index);
        self.segment_count = manager.get(actual_index).map(|t| t.segments_count());

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let actual_index = self
            .actual_track_index
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;
        manager.remove_track(actual_index)
    }

    fn describe(&self) -> String {
        let actual = self
            .actual_track_index
            .map_or("pending".to_string(), |i| i.to_string());
        format!(
            "Insert track at requested index {} (actual: {})",
            self.requested_index, actual
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let Some(actual_index) = self.actual_track_index else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };
        let Some(segment_count) = self.segment_count else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };

        let is_video_or_image = matches!(self.track, Track::Video(_) | Track::Image(_));
        let mut affected = AffectedSegments::new();
        affected.tracks_changed = true;

        for seg_idx in 0..segment_count {
            if is_video_or_image {
                affected.add_both_thumbnails(actual_index, seg_idx);
            } else {
                affected.add_audio_only(actual_index, seg_idx);
            }
        }

        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

pub struct RemoveTrackCommand {
    track_index: usize,
    removed_track: Option<Track>,
    segment_count: Option<usize>,
    is_video_or_image: Option<bool>,
}

impl RemoveTrackCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            removed_track: None,
            segment_count: None,
            is_video_or_image: None,
        }
    }
}

impl Command for RemoveTrackCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager.len()))?;

        self.segment_count = Some(track.segments_count());
        self.is_video_or_image = Some(matches!(track, Track::Video(_) | Track::Image(_)));

        let track = track.clone();
        self.removed_track = Some(track);
        manager.remove_track(self.track_index)
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = self
            .removed_track
            .take()
            .ok_or_else(|| Error::InvalidConfig("Track not saved".into()))?;

        manager.insert_track(self.track_index, track)?;
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Remove track {}", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let Some(segment_count) = self.segment_count else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };
        let Some(is_video_or_image) = self.is_video_or_image else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };

        let mut affected = AffectedSegments::new();
        affected.tracks_changed = true;

        for seg_idx in 0..segment_count {
            if is_video_or_image {
                affected.add_both_thumbnails(self.track_index, seg_idx);
            } else {
                affected.add_audio_only(self.track_index, seg_idx);
            }
        }

        affected
    }
}

pub struct MoveTrackCommand {
    from_index: usize,
    to_index: usize,
}

impl MoveTrackCommand {
    pub fn new(from_index: usize, to_index: usize) -> Self {
        Self {
            from_index,
            to_index,
        }
    }
}

impl Command for MoveTrackCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        manager.move_track(self.from_index, self.to_index)
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        manager.move_track(self.to_index, self.from_index)
    }

    fn describe(&self) -> String {
        format!("Move track {} -> {}", self.from_index, self.to_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

pub struct DuplicateTrackCommand {
    track_index: usize,
    new_track_index: Option<usize>,
    segment_count: Option<usize>,
    is_video_or_image: Option<bool>,
}

impl DuplicateTrackCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            new_track_index: None,
            segment_count: None,
            is_video_or_image: None,
        }
    }
}

impl Command for DuplicateTrackCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager.len()))?;

        self.is_video_or_image = Some(matches!(track, Track::Video(_) | Track::Image(_)));

        // 深度克隆轨道
        let duplicated = match track {
            Track::Video(vt) => {
                let vt = vt.as_ref();
                Track::Video(Arc::new(VideoTrack {
                    name: vt.name.clone(),
                    hiding: vt.hiding,
                    muted: vt.muted,
                    locked: vt.locked,
                    track: vt.track.clone(),
                }))
            }
            Track::Audio(at) => {
                let at = at.as_ref();
                Track::Audio(Arc::new(AudioTrack {
                    name: at.name.clone(),
                    hiding: at.hiding,
                    locked: at.locked,
                    track: at.track.clone(),
                }))
            }
            Track::Subtitle(st) => {
                let st = st.as_ref();
                Track::Subtitle(Arc::new(SubtitleTrack {
                    name: st.name.clone(),
                    hiding: st.hiding,
                    locked: st.locked,
                    track: st.track.clone(),
                }))
            }
            Track::Image(ot) => {
                let ot = ot.as_ref();
                Track::Image(Arc::new(ImageTrack {
                    name: ot.name.clone(),
                    hiding: ot.hiding,
                    locked: ot.locked,
                    track: ot.track.clone(),
                }))
            }
            Track::Text(tt) => {
                let tt = tt.as_ref();
                Track::Text(Arc::new(TextTrack {
                    name: tt.name.clone(),
                    hiding: tt.hiding,
                    locked: tt.locked,
                    track: tt.track.clone(),
                }))
            }
        };

        let new_index = manager.add_track(duplicated);
        self.new_track_index = Some(new_index);
        self.segment_count = manager.get(new_index).map(|t| t.segments_count());

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let index = self
            .new_track_index
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;
        manager.remove_track(index)
    }

    fn describe(&self) -> String {
        format!("Duplicate track {}", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let Some(new_track_index) = self.new_track_index else {
            return AffectedSegments::default();
        };
        let Some(segment_count) = self.segment_count else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };
        let Some(is_video_or_image) = self.is_video_or_image else {
            return AffectedSegments {
                segments: vec![],
                tracks_changed: true,
            };
        };

        let mut affected = AffectedSegments::new();
        affected.tracks_changed = true;

        for seg_idx in 0..segment_count {
            if is_video_or_image {
                affected.add_both_thumbnails(new_track_index, seg_idx);
            } else {
                affected.add_audio_only(new_track_index, seg_idx);
            }
        }

        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

pub struct ToggleTrackVisibilityCommand {
    track_index: usize,
    old_visibility: Option<bool>,
}

impl ToggleTrackVisibilityCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            old_visibility: None,
        }
    }
}

impl Command for ToggleTrackVisibilityCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        self.old_visibility = Some(track.is_hiding());
        track.set_hiding(!track.is_hiding());
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_visibility = self
            .old_visibility
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.set_hiding(old_visibility);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Toggle track {} visibility", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

pub struct SetTrackVisibilityCommand {
    track_index: usize,
    new_visibility: bool,
    old_visibility: Option<bool>,
}

impl SetTrackVisibilityCommand {
    pub fn new(track_index: usize, hiding: bool) -> Self {
        Self {
            track_index,
            new_visibility: hiding,
            old_visibility: None,
        }
    }
}

impl Command for SetTrackVisibilityCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        self.old_visibility = Some(track.is_hiding());
        track.set_hiding(self.new_visibility);
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_visibility = self
            .old_visibility
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.set_hiding(old_visibility);
        Ok(())
    }

    fn describe(&self) -> String {
        let action = if self.new_visibility { "Hide" } else { "Show" };
        format!("{} track {}", action, self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

pub struct TrimGapCommand {
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
    start_gap: Option<Duration>,
    end_gap: Option<Duration>,
}

impl TrimGapCommand {
    pub fn new(track_index: usize, segment_index: usize, shift_timeline: bool) -> Self {
        Self {
            track_index,
            segment_index,
            shift_timeline,
            start_gap: None,
            end_gap: None,
        }
    }
}

impl Command for TrimGapCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();

        if self.segment_index == 0 {
            self.start_gap = segments.first().map(|s| s.timeline_offset);
        } else {
            let prev_segment = &segments[self.segment_index - 1];
            let target_segment = &segments[self.segment_index];
            let prev_end = prev_segment.timeline_offset + prev_segment.duration;
            self.start_gap = Some(target_segment.timeline_offset.saturating_sub(prev_end));
        }

        if self.segment_index == segments.len() - 1 {
            let target_segment = &segments[self.segment_index];
            let segment_end = target_segment.timeline_offset + target_segment.duration;
            self.end_gap = Some(track.duration().saturating_sub(segment_end));
        } else {
            let target_segment = &segments[self.segment_index];
            let next_segment = &segments[self.segment_index + 1];
            let segment_end = target_segment.timeline_offset + target_segment.duration;
            self.end_gap = Some(next_segment.timeline_offset.saturating_sub(segment_end));
        }

        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.trim_gap(self.segment_index, self.shift_timeline)?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let start_gap = self.start_gap.unwrap_or(Duration::ZERO);
        let end_gap = self.end_gap.unwrap_or(Duration::ZERO);

        if start_gap.is_zero() && end_gap.is_zero() {
            return Ok(());
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments_count = track.segments_count();

        if start_gap > Duration::ZERO {
            if self.shift_timeline {
                let start_index = self.segment_index;
                for i in start_index..segments_count {
                    track.shift_segment_timeline(i, start_gap)?;
                }
            } else {
                track.shift_segment_timeline(self.segment_index, start_gap)?;
            }
        }

        if end_gap > Duration::ZERO {
            if self.segment_index < segments_count - 1 {
                if self.shift_timeline {
                    for i in (self.segment_index + 1)..segments_count {
                        track.shift_segment_timeline(i, end_gap)?;
                    }
                } else {
                    track.shift_segment_timeline(self.segment_index + 1, end_gap)?;
                }
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Trim gap from segment {} in track {}",
            self.segment_index, self.track_index
        )
    }
}

pub struct TrimStartGapCommand {
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
    removed_gap: Option<Duration>,
}

impl TrimStartGapCommand {
    pub fn new(track_index: usize, segment_index: usize, shift_timeline: bool) -> Self {
        Self {
            track_index,
            segment_index,
            shift_timeline,
            removed_gap: None,
        }
    }
}

impl Command for TrimStartGapCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();
        if self.segment_index == 0 {
            self.removed_gap = segments.first().map(|s| s.timeline_offset);
        } else {
            let prev_segment = &segments[self.segment_index - 1];
            let target_segment = &segments[self.segment_index];
            let prev_end = prev_segment.timeline_offset + prev_segment.duration;
            self.removed_gap = Some(target_segment.timeline_offset.saturating_sub(prev_end));
        }

        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;
        track.trim_start_gap(self.segment_index, self.shift_timeline)?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let removed_gap = self
            .removed_gap
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        if removed_gap.is_zero() {
            return Ok(());
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if self.shift_timeline {
            let start_index = self.segment_index;
            for i in start_index..track.segments_count() {
                track.shift_segment_timeline(i, removed_gap)?;
            }
        } else {
            track.shift_segment_timeline(self.segment_index, removed_gap)?;
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Trim start gap from segment {} in track {}",
            self.segment_index, self.track_index
        )
    }
}

pub struct TrimEndGapCommand {
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
    removed_gap: Option<Duration>,
}

impl TrimEndGapCommand {
    pub fn new(track_index: usize, segment_index: usize, shift_timeline: bool) -> Self {
        Self {
            track_index,
            segment_index,
            shift_timeline,
            removed_gap: None,
        }
    }
}

impl Command for TrimEndGapCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();
        if self.segment_index == segments.len() - 1 {
            let target_segment = &segments[self.segment_index];
            let segment_end = target_segment.timeline_offset + target_segment.duration;
            self.removed_gap = Some(track.duration().saturating_sub(segment_end));
        } else {
            let target_segment = &segments[self.segment_index];
            let next_segment = &segments[self.segment_index + 1];
            let segment_end = target_segment.timeline_offset + target_segment.duration;
            self.removed_gap = Some(next_segment.timeline_offset.saturating_sub(segment_end));
        }

        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.trim_end_gap(self.segment_index, self.shift_timeline)?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let removed_gap = self
            .removed_gap
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        if removed_gap.is_zero() {
            return Ok(());
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments_count = track.segments_count();

        if self.segment_index < segments_count - 1 {
            if self.shift_timeline {
                for i in (self.segment_index + 1)..segments_count {
                    track.shift_segment_timeline(i, removed_gap)?;
                }
            } else {
                track.shift_segment_timeline(self.segment_index + 1, removed_gap)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Trim end gap from segment {} in track {}",
            self.segment_index, self.track_index
        )
    }
}

pub struct ToggleTrackMutedCommand {
    track_index: usize,
    old_muted: Option<bool>,
}

impl ToggleTrackMutedCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            old_muted: None,
        }
    }
}

impl Command for ToggleTrackMutedCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        self.old_muted = Some(track.is_muted());
        track.set_muted(!track.is_muted());
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_muted = self
            .old_muted
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.set_muted(old_muted);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Toggle track {} muted state", self.track_index)
    }
}

pub struct ToggleTrackLockedCommand {
    track_index: usize,
    old_locked: Option<bool>,
}

impl ToggleTrackLockedCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            old_locked: None,
        }
    }
}

impl Command for ToggleTrackLockedCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        self.old_locked = Some(track.is_locked());
        track.set_locked(!track.is_locked());
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_locked = self
            .old_locked
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.set_locked(old_locked);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Toggle track {} locked state", self.track_index)
    }
}

pub struct SetTrackMutedCommand {
    track_index: usize,
    new_muted: bool,
    old_muted: Option<bool>,
}

impl SetTrackMutedCommand {
    pub fn new(track_index: usize, muted: bool) -> Self {
        Self {
            track_index,
            new_muted: muted,
            old_muted: None,
        }
    }
}

impl Command for SetTrackMutedCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        self.old_muted = Some(track.is_muted());
        track.set_muted(self.new_muted);
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_muted = self
            .old_muted
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.set_muted(old_muted);
        Ok(())
    }

    fn describe(&self) -> String {
        let action = if self.new_muted { "Mute" } else { "Unmute" };
        format!("{} track {}", action, self.track_index)
    }
}

pub struct RemoveAllGapsCommand {
    track_index: usize,
    original_offsets: Option<Vec<Duration>>, // 原始每个片段的 timeline_offset
}

impl RemoveAllGapsCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            original_offsets: None,
        }
    }
}

impl Command for RemoveAllGapsCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        // 保存所有片段的原始 timeline_offset
        let segments = track.segments();
        let original_offsets: Vec<Duration> = segments.iter().map(|s| s.timeline_offset).collect();

        self.original_offsets = Some(original_offsets);
        track.remove_all_gaps()?;
        manager.update_duration();

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let original_offsets = self
            .original_offsets
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        if original_offsets.is_empty() {
            return Ok(());
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        // 恢复每个片段的原始 timeline_offset
        for (i, &offset) in original_offsets.iter().enumerate() {
            track.modify_segment(i, |seg| {
                seg.timeline_offset = offset;
            })?;
        }
        manager.update_duration();

        Ok(())
    }

    fn describe(&self) -> String {
        format!("Remove all gaps from track {}", self.track_index)
    }
}

pub struct UpdateTrackMetadataCommand {
    track_index: usize,
    old_metadata: Option<Arc<Metadata>>,
    new_metadata: Arc<Metadata>,
}

impl UpdateTrackMetadataCommand {
    pub fn new(track_index: usize, new_metadata: Arc<Metadata>) -> Self {
        Self {
            track_index,
            old_metadata: None,
            new_metadata,
        }
    }
}

impl Command for UpdateTrackMetadataCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match track {
            Track::Video(vt) => {
                self.old_metadata = Some(vt.track.metadata.clone());
                let vt = Arc::make_mut(vt);
                vt.track.metadata = self.new_metadata.clone();
            }
            Track::Audio(_) => {
                return Err(Error::InvalidConfig(
                    "Audio tracks do not support metadata updates".into(),
                ));
            }
            Track::Subtitle(_) => {
                return Err(Error::InvalidConfig(
                    "Subtitle tracks do not support metadata updates".into(),
                ));
            }
            Track::Image(_) => {
                return Err(Error::InvalidConfig(
                    "Image tracks do not support metadata updates".into(),
                ));
            }
            Track::Text(_) => {
                return Err(Error::InvalidConfig(
                    "Text tracks do not have metadata".into(),
                ));
            }
        }
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_metadata = self
            .old_metadata
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match track {
            Track::Video(vt) => {
                let vt = Arc::make_mut(vt);
                vt.track.metadata = old_metadata;
            }
            _ => {}
        }
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Update track {} metadata", self.track_index)
    }
}

pub struct DetachAudioTracksCommand {
    track_index: usize,
    old_metadata: Option<Arc<Metadata>>,
    detached_track_indices: Option<Vec<usize>>,
    segments_count: Option<usize>,
}

impl DetachAudioTracksCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            old_metadata: None,
            detached_track_indices: None,
            segments_count: None,
        }
    }
}

impl Command for DetachAudioTracksCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let segments_count = manager
            .get(self.track_index)
            .map(|t| t.segments_count())
            .unwrap_or(0);
        self.segments_count = Some(segments_count);

        execute_detach(
            manager,
            self.track_index,
            DetachType::Audio,
            &mut self.old_metadata,
            &mut self.detached_track_indices,
        )
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        undo_detach(
            manager,
            self.track_index,
            &mut self.old_metadata,
            &mut self.detached_track_indices,
        )
    }

    fn describe(&self) -> String {
        format!("Detach audio tracks from track {}", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        // After detaching audio, the video track segments should NOT have their
        // audio samples refreshed because we explicitly cleared the display cache
        // and the audio has been moved to new tracks.
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        // After undo, the audio samples need to be regenerated for all segments
        // in the video track since they were cleared during execute()
        let segments_count = self.segments_count.unwrap_or(0);
        let mut affected = AffectedSegments::new();
        affected.tracks_changed = true;

        for seg_idx in 0..segments_count {
            affected.add_audio_only(self.track_index, seg_idx);
        }

        affected
    }
}

pub struct DetachSubtitleTracksCommand {
    track_index: usize,
    old_metadata: Option<Arc<Metadata>>,
    detached_track_indices: Option<Vec<usize>>,
    segments_count: Option<usize>,
}

impl DetachSubtitleTracksCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            old_metadata: None,
            detached_track_indices: None,
            segments_count: None,
        }
    }
}

impl Command for DetachSubtitleTracksCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let segments_count = manager
            .get(self.track_index)
            .map(|t| t.segments_count())
            .unwrap_or(0);
        self.segments_count = Some(segments_count);

        execute_detach(
            manager,
            self.track_index,
            DetachType::Subtitle,
            &mut self.old_metadata,
            &mut self.detached_track_indices,
        )
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        undo_detach(
            manager,
            self.track_index,
            &mut self.old_metadata,
            &mut self.detached_track_indices,
        )
    }

    fn describe(&self) -> String {
        format!("Detach subtitle tracks from track {}", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum DetachType {
    Audio,
    Subtitle,
}

fn execute_detach(
    manager: &mut Manager,
    track_index: usize,
    detach_type: DetachType,
    old_metadata: &mut Option<Arc<Metadata>>,
    detached_track_indices: &mut Option<Vec<usize>>,
) -> Result<()> {
    let manager_len = manager.len();
    if track_index >= manager_len {
        return Err(Error::IndexOutOfBounds(track_index, manager_len));
    }

    let track = manager
        .get(track_index)
        .ok_or_else(|| Error::IndexOutOfBounds(track_index, manager_len))?;

    let video_track = match track {
        Track::Video(vt) => vt,
        _ => {
            return Err(Error::InvalidConfig(format!(
                "Only video tracks can have detached {}",
                match detach_type {
                    DetachType::Audio => "audio",
                    DetachType::Subtitle => "subtitles",
                }
            )));
        }
    };

    let has_streams = match detach_type {
        DetachType::Audio => !video_track.track.metadata.audios.is_empty(),
        DetachType::Subtitle => !video_track.track.metadata.subtitles.is_empty(),
    };

    if !has_streams {
        return Err(Error::InvalidConfig(format!(
            "Video track has no {} streams to detach",
            match detach_type {
                DetachType::Audio => "audio",
                DetachType::Subtitle => "subtitle",
            }
        )));
    }

    let mut video_track_mut = video_track.as_ref().clone();
    *old_metadata = Some(video_track.track.metadata.clone());

    let global_speed = manager.get_global_speed();
    let detached_tracks: Vec<Track> = match detach_type {
        DetachType::Audio => video_track_mut
            .detach_audio_tracks()
            .into_iter()
            .map(|at| Track::Audio(Arc::new(at)))
            .collect(),
        DetachType::Subtitle => video_track_mut
            .detach_subtitle_tracks(global_speed)
            .into_iter()
            .map(|st| Track::Subtitle(Arc::new(st)))
            .collect(),
    };

    if detached_tracks.is_empty() {
        return Err(Error::InvalidConfig(format!(
            "No {} tracks were created{}",
            match detach_type {
                DetachType::Audio => "audio",
                DetachType::Subtitle => "subtitle",
            },
            match detach_type {
                DetachType::Audio => "",
                DetachType::Subtitle => " (extraction may have failed)",
            }
        )));
    }

    // Update video track metadata
    let new_metadata = video_track_mut.track.metadata.clone();
    let track = manager
        .get_mut(track_index)
        .ok_or_else(|| Error::IndexOutOfBounds(track_index, manager_len))?;

    if let Track::Video(vt) = track {
        let vt = Arc::make_mut(vt);
        vt.track.metadata = new_metadata;

        if detach_type == DetachType::Audio {
            for seg in vt.track.segments.iter_mut() {
                Arc::make_mut(seg).clear_display_audio_samples();
            }
        }
    }

    // Insert detached tracks
    // Subtitle tracks go to top layer (index 0, 1, 2, ...)
    // Audio tracks go after the video track (track_index + 1, + 2, ...)
    let mut inserted_indices = Vec::new();
    for (i, track) in detached_tracks.into_iter().enumerate() {
        let insert_idx = match detach_type {
            DetachType::Subtitle => i,                // Insert at top layer
            DetachType::Audio => track_index + 1 + i, // Insert after video track
        };
        manager.insert_track(insert_idx, track)?;
        inserted_indices.push(insert_idx);
    }

    *detached_track_indices = Some(inserted_indices);
    Ok(())
}

fn undo_detach(
    manager: &mut Manager,
    track_index: usize,
    old_metadata: &mut Option<Arc<Metadata>>,
    detached_track_indices: &mut Option<Vec<usize>>,
) -> Result<()> {
    let old_meta = old_metadata
        .take()
        .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

    let detached_indices = detached_track_indices
        .take()
        .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

    // Remove detached tracks in reverse order
    for idx in detached_indices.into_iter().rev() {
        manager.remove_track(idx)?;
    }

    // Restore original metadata
    let manager_len = manager.len();
    let track = manager
        .get_mut(track_index)
        .ok_or_else(|| Error::IndexOutOfBounds(track_index, manager_len))?;

    if let Track::Video(vt) = track {
        let vt = Arc::make_mut(vt);
        vt.track.metadata = old_meta;
    }

    Ok(())
}

pub struct StretchTrackToEndCommand {
    track_index: usize,
    target_duration: Duration,
    stretch_command: Option<StretchSegmentRightCommand>,
}

impl StretchTrackToEndCommand {
    pub fn new(track_index: usize) -> Self {
        Self {
            track_index,
            target_duration: Duration::ZERO,
            stretch_command: None,
        }
    }
}

impl Command for StretchTrackToEndCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        self.target_duration = manager.duration;

        let track_duration = track.duration();
        if track_duration >= self.target_duration {
            return Ok(());
        }

        let segments = track.segments();
        if segments.is_empty() {
            return Err(Error::InvalidConfig(
                "Track has no segments to stretch".into(),
            ));
        }

        let last_segment_index = segments.len() - 1;
        let last_segment = &segments[last_segment_index];
        let stretch_duration = self.target_duration.saturating_sub(track_duration);

        if !last_segment.metadata.is_time_independent() {
            // For video/audio, check if source file has enough remaining content
            let current_end = last_segment.source_offset + last_segment.duration;
            let remaining = last_segment.metadata.duration.saturating_sub(current_end);

            if remaining < stretch_duration {
                return Err(Error::InvalidConfig(format!(
                    "Cannot stretch track to end: source file only has {:?} remaining, need {:?}",
                    remaining, stretch_duration
                )));
            }
        }

        let mut cmd = StretchSegmentRightCommand::new(
            self.track_index,
            last_segment_index,
            stretch_duration,
            false, // Don't shift timeline since we're only stretching the last segment
        );

        cmd.execute(manager)?;
        self.stretch_command = Some(cmd);

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(ref mut cmd) = self.stretch_command {
            cmd.undo(manager)?;
        }
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Stretch track {} to end (target: {:?})",
            self.track_index, self.target_duration
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        if let Some(ref cmd) = self.stretch_command {
            return cmd.affected_segments_after_execute();
        }
        AffectedSegments::new()
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        if let Some(ref cmd) = self.stretch_command {
            return cmd.affected_segments_after_undo();
        }
        AffectedSegments::new()
    }
}
