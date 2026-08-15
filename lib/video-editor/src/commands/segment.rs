use super::command::{AffectedSegments, Command};
use crate::{
    Error, Result,
    filters::{
        keyframe::{Keyframe, KeyframeTracks, KeyframeValue},
        traits::{
            AudioFilterWrapper, ImageFilterWrapper, SubtitleFilterWrapper, VideoFilterWrapper,
        },
    },
    metadata::Metadata,
    tracks::{manager::Manager, segment::Segment, text_track::TextElement, track::Track},
};
use std::{sync::Arc, time::Duration};

pub struct AddSegmentCommand {
    pub track_index: usize,
    pub segment: Arc<Segment>,
    segment_index: Option<usize>,
}

impl AddSegmentCommand {
    pub fn new(track_index: usize, segment: Arc<Segment>) -> Self {
        Self {
            track_index,
            segment,
            segment_index: None,
        }
    }
}

impl Command for AddSegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.segment.duration.is_zero() {
            return Err(Error::InvalidConfig(
                "Cannot add segment with zero duration".into(),
            ));
        }
        let track = get_track_from_manager(manager, self.track_index)?;
        track.add_segment(self.segment.clone());
        self.segment_index = Some(track.segments_count() - 1);
        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.remove_segment_shift(track.segments_count() - 1)?;
        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Add segment to track {}", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        if let Some(idx) = self.segment_index {
            let mut affected = AffectedSegments::new();
            affected.add_both_thumbnails(self.track_index, idx);
            affected
        } else {
            AffectedSegments::default()
        }
    }
}

pub struct InsertSegmentCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub segment: Arc<Segment>,
}

impl InsertSegmentCommand {
    pub fn new(track_index: usize, segment_index: usize, segment: Arc<Segment>) -> Self {
        Self {
            track_index,
            segment_index,
            segment,
        }
    }
}

impl Command for InsertSegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.segment.duration.is_zero() {
            return Err(Error::InvalidConfig(
                "Cannot insert segment with zero duration".into(),
            ));
        }
        let track = get_track_from_manager(manager, self.track_index)?;
        track.insert_segment_shift(self.segment_index, self.segment.clone())?;
        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.remove_segment_shift(self.segment_index)?;
        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Insert segment at index {} in track {}",
            self.segment_index, self.track_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_both_thumbnails(self.track_index, self.segment_index);
        affected
    }
}

// Insert segment at a specific time position, preserving the timeline_offset
// and optionally shifting subsequent segments.
pub struct InsertSegmentAtTimeCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub segment: Arc<Segment>,
    pub shift_timeline: bool,

    // Store actual shift amount for undo (calculated during execute)
    actual_shift_amount: Option<Duration>,

    // Store the range of shifted subsequent segments (for affected_segments reporting)
    // If shift happens, stores the last segment index that was shifted
    shifted_segments_end: Option<usize>,
}

impl InsertSegmentAtTimeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        segment: Arc<Segment>,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            segment,
            shift_timeline,
            actual_shift_amount: None,
            shifted_segments_end: None,
        }
    }
}

impl Command for InsertSegmentAtTimeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.segment.duration.is_zero() {
            return Err(Error::InvalidConfig(
                "Cannot insert segment with zero duration".into(),
            ));
        }

        let track = get_track_from_manager(manager, self.track_index)?;
        track.insert_segment_preserve(self.segment_index, self.segment.clone())?;

        if self.shift_timeline {
            let inserted_end = self.segment.timeline_offset + self.segment.duration;

            if self.segment_index + 1 < track.segments_count() {
                let next_seg = track.get_segment(self.segment_index + 1)?;
                if next_seg.timeline_offset < inserted_end {
                    let overlap = inserted_end - next_seg.timeline_offset;
                    self.actual_shift_amount = Some(overlap);

                    let segments_count_after_insert = track.segments_count();
                    self.shifted_segments_end = Some(segments_count_after_insert - 1);

                    for i in (self.segment_index + 1)..track.segments_count() {
                        track.shift_segment_timeline(i, overlap)?;
                    }
                } else {
                    self.actual_shift_amount = Some(Duration::ZERO);
                    self.shifted_segments_end = None;
                }
            } else {
                self.actual_shift_amount = Some(Duration::ZERO);
                self.shifted_segments_end = None;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        if self.shift_timeline && self.actual_shift_amount.is_some() {
            let shift_amount = self.actual_shift_amount.unwrap();
            if shift_amount > Duration::ZERO {
                for i in (self.segment_index + 1)..track.segments_count() {
                    track.shift_segment_timeline_backward(i, shift_amount)?;
                }
            }
        }

        track.remove_segment_leave_gap(self.segment_index)?;

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Insert segment at time {:?} in track {}",
            self.segment.timeline_offset, self.track_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_both_thumbnails(self.track_index, self.segment_index);

        if let Some(end) = self.shifted_segments_end {
            for i in (self.segment_index + 1)..=end {
                affected.add_both_thumbnails(self.track_index, i);
            }
        }

        affected
    }
}

pub struct RemoveSegmentCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub removed_segment: Option<Arc<Segment>>,
    pub shift_timeline: bool,
}

impl RemoveSegmentCommand {
    pub fn new(track_index: usize, segment_index: usize, shift_timeline: bool) -> Self {
        Self {
            track_index,
            segment_index,
            removed_segment: None,
            shift_timeline,
        }
    }
}

impl Command for RemoveSegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        self.removed_segment = Some(track.remove_segment(self.segment_index, self.shift_timeline)?);
        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        if let Some(segment) = self.removed_segment.take() {
            track.insert_segment_preserve(self.segment_index, segment)?;
            if self.shift_timeline {
                let seg = track.get_segment(self.segment_index)?;
                let shift_amount = seg.duration;
                for i in (self.segment_index + 1)..track.segments_count() {
                    track.shift_segment_timeline(i, shift_amount)?;
                }
            }
        }
        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Remove segment at index {} from track {}",
            self.segment_index, self.track_index
        )
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_both_thumbnails(self.track_index, self.segment_index);
        affected
    }
}

pub struct SplitSegmentCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub split_time: Duration,
    pub new_indices: Option<(usize, usize)>,
    original_segment: Option<Arc<Segment>>,
}

impl SplitSegmentCommand {
    pub fn new(track_index: usize, segment_index: usize, split_time: Duration) -> Self {
        Self {
            track_index,
            segment_index,
            split_time,
            new_indices: None,
            original_segment: None,
        }
    }
}

impl Command for SplitSegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        self.original_segment = Some(track.get_segment(self.segment_index)?.clone());

        let (index1, index2) = track.split_segment(self.segment_index, self.split_time)?;
        self.new_indices = Some((index1, index2));
        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        if let Some((index1, index2)) = self.new_indices {
            // Remove the two split segments (remove index2 first since it's larger)
            track.remove_segment_leave_gap(index2)?;
            track.remove_segment_leave_gap(index1)?;

            // Restore the original segment preserving its timeline_offset
            if let Some(original) = self.original_segment.take() {
                track.insert_segment_preserve(index1, original)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Split track {} segment {} at {:?}",
            self.track_index, self.segment_index, self.split_time
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        if let Some((index1, index2)) = self.new_indices {
            let mut affected = AffectedSegments::new();
            affected.add_both_thumbnails(self.track_index, index1);
            affected.add_both_thumbnails(self.track_index, index2);
            affected
        } else {
            AffectedSegments::default()
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        if let Some((index1, _)) = self.new_indices {
            let mut affected = AffectedSegments::new();
            affected.add_both_thumbnails(self.track_index, index1);
            affected
        } else {
            AffectedSegments::default()
        }
    }
}

pub struct MoveSegmentCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub new_index: usize,
}

impl MoveSegmentCommand {
    pub fn new(track_index: usize, segment_index: usize, new_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            new_index,
        }
    }
}

impl Command for MoveSegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?.clone();
        track.remove_segment_leave_gap(self.segment_index)?;
        track.insert_segment_image(self.new_index, segment)?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.new_index)?.clone();
        track.remove_segment_leave_gap(self.new_index)?;
        track.insert_segment_image(self.segment_index, segment)?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Move segment {} -> {} in track {}",
            self.segment_index, self.new_index, self.track_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.new_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }
}

pub struct ShrinkSegmentLeftCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub shrink_timeline_duration: Duration,
    pub original_start: Duration,
    pub original_source_offset: Duration,
    pub original_duration: Duration,
    pub shift_timeline: bool,
}

impl ShrinkSegmentLeftCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        shrink_timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            shrink_timeline_duration,
            original_start: Duration::ZERO,
            original_source_offset: Duration::ZERO,
            original_duration: Duration::ZERO,
            shift_timeline,
        }
    }
}

impl Command for ShrinkSegmentLeftCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?;
        self.original_start = segment.timeline_offset;
        self.original_source_offset = segment.source_offset;
        self.original_duration = segment.duration;

        track.shrink_segment_left(
            self.segment_index,
            self.shrink_timeline_duration,
            self.shift_timeline,
        )?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        track.modify_segment(self.segment_index, |segment| {
            segment.timeline_offset = self.original_start;
            segment.source_offset = self.original_source_offset;
            segment.duration = self.original_duration;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                track.shift_segment_timeline(i, self.shrink_timeline_duration)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Shrink left side of track {} segment {} by {:?}",
            self.track_index, self.segment_index, self.shrink_timeline_duration
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_left_thumbnail(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_left_thumbnail(self.track_index, self.segment_index);
        affected
    }
}

pub struct ShrinkSegmentRightCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub shrink_timeline_duration: Duration,
    pub original_end: Duration,
    pub shift_timeline: bool,
}

impl ShrinkSegmentRightCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        shrink_timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            shrink_timeline_duration,
            original_end: Duration::ZERO,
            shift_timeline,
        }
    }
}

impl Command for ShrinkSegmentRightCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?;
        self.original_end = segment.timeline_offset + segment.duration;

        track.shrink_segment_right(
            self.segment_index,
            self.shrink_timeline_duration,
            self.shift_timeline,
        )?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.duration = self.original_end - segment.timeline_offset;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                track.shift_segment_timeline(i, self.shrink_timeline_duration)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Shrink right side of track {} segment {} by {:?}",
            self.track_index, self.segment_index, self.shrink_timeline_duration
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_right_thumbnail(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_right_thumbnail(self.track_index, self.segment_index);
        affected
    }
}

pub struct StretchSegmentLeftCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub stretch_timeline_duration: Duration,
    pub original_duration: Duration,
    pub original_source_offset: Duration,
    pub original_timeline_offset: Duration,
    pub shift_timeline: bool,
}

impl StretchSegmentLeftCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        stretch_timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            stretch_timeline_duration,
            original_duration: Duration::ZERO,
            original_source_offset: Duration::ZERO,
            original_timeline_offset: Duration::ZERO,
            shift_timeline,
        }
    }
}

impl Command for StretchSegmentLeftCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?;
        self.original_duration = segment.duration;
        self.original_source_offset = segment.source_offset;
        self.original_timeline_offset = segment.timeline_offset;

        track.stretch_segment_left(
            self.segment_index,
            self.stretch_timeline_duration,
            self.shift_timeline,
        )?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.duration = self.original_duration;
            segment.source_offset = self.original_source_offset;
            segment.timeline_offset = self.original_timeline_offset;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                track.shift_segment_timeline_backward(i, self.stretch_timeline_duration)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Left stretch track {} segment {} to {:?}",
            self.track_index,
            self.segment_index,
            self.original_duration + self.stretch_timeline_duration
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_left_thumbnail(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_left_thumbnail(self.track_index, self.segment_index);
        affected
    }
}

pub struct StretchSegmentRightCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub stretch_timeline_duration: Duration,
    pub original_duration: Duration,
    pub original_source_offset: Duration,
    pub shift_timeline: bool,
}

impl StretchSegmentRightCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        stretch_timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            stretch_timeline_duration,
            original_duration: Duration::ZERO,
            original_source_offset: Duration::ZERO,
            shift_timeline,
        }
    }
}

impl Command for StretchSegmentRightCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?;
        self.original_duration = segment.duration;
        self.original_source_offset = segment.source_offset;

        track.stretch_segment_right(
            self.segment_index,
            self.stretch_timeline_duration,
            self.shift_timeline,
        )?;

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.duration = self.original_duration;
            segment.source_offset = self.original_source_offset;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                track.shift_segment_timeline_backward(i, self.stretch_timeline_duration)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Right stretch track {} segment {} to {:?}",
            self.track_index,
            self.segment_index,
            self.original_duration + self.stretch_timeline_duration
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_right_thumbnail(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_right_thumbnail(self.track_index, self.segment_index);
        affected
    }
}

pub struct SetSegmentDurationCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub new_duration: Duration,
    pub original_duration: Duration,
    pub shift_timeline: bool,
}

impl SetSegmentDurationCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        new_duration: Duration,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            new_duration,
            original_duration: Duration::ZERO,
            shift_timeline,
        }
    }
}

impl Command for SetSegmentDurationCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?;
        self.original_duration = segment.duration;

        track.modify_segment(self.segment_index, |segment| {
            segment.duration = self.new_duration;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                if self.original_duration > self.new_duration {
                    track.shift_segment_timeline_backward(
                        i,
                        self.original_duration - self.new_duration,
                    )?;
                } else if self.original_duration < self.new_duration {
                    track.shift_segment_timeline(i, self.new_duration - self.original_duration)?;
                }
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.duration = self.original_duration;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                if self.original_duration > self.new_duration {
                    track.shift_segment_timeline(i, self.original_duration - self.new_duration)?;
                } else if self.original_duration < self.new_duration {
                    track.shift_segment_timeline_backward(
                        i,
                        self.new_duration - self.original_duration,
                    )?;
                }
            }
        }
        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Set duration of track {} segment {} to {:?}",
            self.track_index, self.segment_index, self.new_duration
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_right_thumbnail(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_right_thumbnail(self.track_index, self.segment_index);
        affected
    }
}

pub struct CopySegmentCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub target_index: Option<usize>,
    pub new_segment_index: Option<usize>,
}

impl CopySegmentCommand {
    pub fn new(track_index: usize, segment_index: usize, target_index: Option<usize>) -> Self {
        Self {
            track_index,
            segment_index,
            target_index,
            new_segment_index: None,
        }
    }
}

impl Command for CopySegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?.clone();

        let insert_index = self.target_index.unwrap_or(track.segments_count());
        track.insert_segment_shift(insert_index, segment)?;

        self.new_segment_index = Some(insert_index);
        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let index = self
            .new_segment_index
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;
        track.remove_segment_shift(index)?;
        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Copy segment at track {} index {}",
            self.track_index, self.segment_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        if let Some(idx) = self.new_segment_index {
            let mut affected = AffectedSegments::new();
            affected.add(self.track_index, idx);
            affected
        } else {
            AffectedSegments::default()
        }
    }
}

pub struct MoveSegmentToTimeCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub new_timeline_offset: Duration,
    pub original_timeline_offset: Duration,
    pub shift_timeline: bool,
}

impl MoveSegmentToTimeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        new_timeline_offset: Duration,
        shift_timeline: bool,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            new_timeline_offset,
            original_timeline_offset: Duration::ZERO,
            shift_timeline,
        }
    }
}

impl Command for MoveSegmentToTimeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segment = track.get_segment(self.segment_index)?;
        self.original_timeline_offset = segment.timeline_offset;

        track.modify_segment(self.segment_index, |segment| {
            segment.timeline_offset = self.new_timeline_offset;
        })?;

        if self.shift_timeline {
            // 只移动当前 segment 之后的 segments，不包括当前 segment（当前 segment 已经被设置为新 offset）
            for i in (self.segment_index + 1)..track.segments_count() {
                if self.original_timeline_offset > self.new_timeline_offset {
                    track.shift_segment_timeline_backward(
                        i,
                        self.original_timeline_offset - self.new_timeline_offset,
                    )?;
                } else if self.original_timeline_offset < self.new_timeline_offset {
                    track.shift_segment_timeline(
                        i,
                        self.new_timeline_offset - self.original_timeline_offset,
                    )?;
                }
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.timeline_offset = self.original_timeline_offset;
        })?;

        if self.shift_timeline {
            for i in (self.segment_index + 1)..track.segments_count() {
                if self.original_timeline_offset > self.new_timeline_offset {
                    track.shift_segment_timeline(
                        i,
                        self.original_timeline_offset - self.new_timeline_offset,
                    )?;
                } else if self.original_timeline_offset < self.new_timeline_offset {
                    track.shift_segment_timeline_backward(
                        i,
                        self.new_timeline_offset - self.original_timeline_offset,
                    )?;
                }
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Move track {} segment {} to time {:?}",
            self.track_index, self.segment_index, self.new_timeline_offset
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }
}

fn get_track_from_manager(manager: &mut Manager, track_index: usize) -> Result<&mut Track> {
    let len = manager.len();
    manager
        .get_mut(track_index)
        .ok_or_else(|| Error::IndexOutOfBounds(track_index, len))
}

pub struct ToggleSegmentVisibilityCommand {
    track_index: usize,
    segment_index: usize,
    old_visibility: Option<bool>,
}

impl ToggleSegmentVisibilityCommand {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            old_visibility: None,
        }
    }
}

impl Command for ToggleSegmentVisibilityCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segment = track.get_segment(self.segment_index)?;
        self.old_visibility = Some(segment.hiding);

        track.modify_segment(self.segment_index, |seg| {
            seg.hiding = !seg.hiding;
        })?;

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

        track.modify_segment(self.segment_index, |seg| {
            seg.hiding = old_visibility;
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Toggle track {} segment {} visibility",
            self.track_index, self.segment_index
        )
    }
}

pub struct ToggleSegmentAudioMutedCommand {
    track_index: usize,
    segment_index: usize,
    old_audio_muted: Option<bool>,
}

impl ToggleSegmentAudioMutedCommand {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            old_audio_muted: None,
        }
    }
}

impl Command for ToggleSegmentAudioMutedCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segment = track.get_segment(self.segment_index)?;
        self.old_audio_muted = Some(segment.audio_muted);

        track.modify_segment(self.segment_index, |seg| {
            seg.audio_muted = !seg.audio_muted;
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_audio_muted = self
            .old_audio_muted
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |seg| {
            seg.audio_muted = old_audio_muted;
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Toggle track {} segment {} audio muted",
            self.track_index, self.segment_index
        )
    }
}

pub struct SetSegmentVisibilityCommand {
    track_index: usize,
    segment_index: usize,
    new_visibility: bool,
    old_visibility: Option<bool>,
}

impl SetSegmentVisibilityCommand {
    pub fn new(track_index: usize, segment_index: usize, hiding: bool) -> Self {
        Self {
            track_index,
            segment_index,
            new_visibility: hiding,
            old_visibility: None,
        }
    }
}

impl Command for SetSegmentVisibilityCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segment = track.get_segment(self.segment_index)?;
        self.old_visibility = Some(segment.hiding);

        track.modify_segment(self.segment_index, |seg| {
            seg.hiding = self.new_visibility;
        })?;

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

        track.modify_segment(self.segment_index, |seg| {
            seg.hiding = old_visibility;
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "{} track {} segment {}",
            if self.new_visibility { "Hide" } else { "Show" },
            self.track_index,
            self.segment_index
        )
    }
}

pub struct DetachSegmentAudioCommand {
    track_index: usize,
    segment_index: usize,
    old_segment_metadata: Option<Arc<crate::metadata::Metadata>>,
    detached_track_indices: Option<Vec<usize>>,
}

impl DetachSegmentAudioCommand {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            old_segment_metadata: None,
            detached_track_indices: None,
        }
    }
}

impl Command for DetachSegmentAudioCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        if self.track_index >= manager_len {
            return Err(Error::IndexOutOfBounds(self.track_index, manager_len));
        }

        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        // Only video tracks support audio detachment
        let video_track = match track {
            Track::Video(vt) => vt,
            _ => {
                return Err(Error::InvalidConfig(
                    "Only video tracks support audio detachment".into(),
                ));
            }
        };

        // Check segment index
        if self.segment_index >= video_track.track.segments.len() {
            return Err(Error::IndexOutOfBounds(
                self.segment_index,
                video_track.track.segments.len(),
            ));
        }

        let segment = &video_track.track.segments[self.segment_index];

        // Check if segment has audio streams
        if segment.metadata.audios.is_empty() {
            return Err(Error::InvalidConfig(
                "Segment has no audio streams to detach".into(),
            ));
        }

        // Save original segment metadata for undo
        self.old_segment_metadata = Some(segment.metadata.clone());

        // Clone video track for modification
        let mut video_track_mut = video_track.as_ref().clone();

        // Detach audio tracks from the segment
        let detached_audio_tracks: Vec<Track> = video_track_mut
            .detach_segment_audio_tracks(self.segment_index)
            .into_iter()
            .map(|at| Track::Audio(std::sync::Arc::new(at)))
            .collect();

        if detached_audio_tracks.is_empty() {
            return Err(Error::InvalidConfig(
                "No audio tracks were created from segment".into(),
            ));
        }

        // Update the video track with modified segment
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Video(vt) = track {
            let vt = std::sync::Arc::make_mut(vt);
            vt.track.segments[self.segment_index] =
                video_track_mut.track.segments[self.segment_index].clone();
            Arc::make_mut(&mut vt.track.segments[self.segment_index]).clear_display_audio_samples();
            vt.update_duration();
        }

        // Insert detached audio tracks
        let mut inserted_indices = Vec::new();
        for (i, track) in detached_audio_tracks.into_iter().enumerate() {
            let insert_idx = self.track_index + 1 + i;
            manager.insert_track(insert_idx, track)?;
            inserted_indices.push(insert_idx);
        }

        self.detached_track_indices = Some(inserted_indices);
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_metadata = self
            .old_segment_metadata
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let detached_indices = self
            .detached_track_indices
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        // Remove detached tracks in reverse order
        for idx in detached_indices.into_iter().rev() {
            manager.remove_track(idx)?;
        }

        // Restore original segment metadata
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Video(vt) = track {
            let vt = Arc::make_mut(vt);
            if self.segment_index < vt.track.segments.len() {
                let segment = Arc::make_mut(&mut vt.track.segments[self.segment_index]);
                segment.metadata = old_metadata;
            }
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Detach audio from track {} segment {}",
            self.track_index, self.segment_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_audio_only(self.track_index, self.segment_index);
        affected.tracks_changed = true;
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_audio_only(self.track_index, self.segment_index);
        affected.tracks_changed = true;
        affected
    }
}

pub struct DetachSegmentSubtitleCommand {
    track_index: usize,
    segment_index: usize,
    old_segment_metadata: Option<Arc<crate::metadata::Metadata>>,
    detached_track_indices: Option<Vec<usize>>,
}

impl DetachSegmentSubtitleCommand {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            old_segment_metadata: None,
            detached_track_indices: None,
        }
    }
}

impl Command for DetachSegmentSubtitleCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        if self.track_index >= manager_len {
            return Err(Error::IndexOutOfBounds(self.track_index, manager_len));
        }

        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        // Only video tracks support subtitle detachment
        let video_track = match track {
            Track::Video(vt) => vt,
            _ => {
                return Err(Error::InvalidConfig(
                    "Only video tracks support subtitle detachment".into(),
                ));
            }
        };

        if self.segment_index >= video_track.track.segments.len() {
            return Err(Error::IndexOutOfBounds(
                self.segment_index,
                video_track.track.segments.len(),
            ));
        }

        let segment = &video_track.track.segments[self.segment_index];
        if segment.metadata.subtitles.is_empty() {
            return Err(Error::InvalidConfig(
                "Segment has no subtitle streams to detach".into(),
            ));
        }

        self.old_segment_metadata = Some(segment.metadata.clone());
        let mut video_track_mut = video_track.as_ref().clone();

        let global_speed = manager.get_global_speed();
        let detached_subtitle_tracks: Vec<Track> = video_track_mut
            .detach_segment_subtitle_tracks(self.segment_index, global_speed)
            .into_iter()
            .map(|st| Track::Subtitle(Arc::new(st)))
            .collect();

        if detached_subtitle_tracks.is_empty() {
            return Err(Error::InvalidConfig(
                "No subtitle tracks were created from segment".into(),
            ));
        }

        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Video(vt) = track {
            let vt = std::sync::Arc::make_mut(vt);
            vt.track.segments[self.segment_index] =
                video_track_mut.track.segments[self.segment_index].clone();
            vt.update_duration();
        }

        let mut inserted_indices = Vec::new();
        for (i, track) in detached_subtitle_tracks.into_iter().enumerate() {
            let insert_idx = i; // Insert at top layer (index 0, 1, 2, ...)
            manager.insert_track(insert_idx, track)?;
            inserted_indices.push(insert_idx);
        }

        self.detached_track_indices = Some(inserted_indices);
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_metadata = self
            .old_segment_metadata
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let detached_indices = self
            .detached_track_indices
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        // Remove detached tracks in reverse order
        for idx in detached_indices.into_iter().rev() {
            manager.remove_track(idx)?;
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Video(vt) = track {
            let vt = std::sync::Arc::make_mut(vt);
            if self.segment_index < vt.track.segments.len() {
                let segment = std::sync::Arc::make_mut(&mut vt.track.segments[self.segment_index]);
                segment.metadata = old_metadata;
            }
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Detach subtitles from track {} segment {}",
            self.track_index, self.segment_index
        )
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

pub struct RemoveSegmentGapCommand {
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
    start_gap: Option<Duration>,
    end_gap: Option<Duration>,
}

impl RemoveSegmentGapCommand {
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

impl Command for RemoveSegmentGapCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();

        if self.segment_index >= segments.len() {
            return Err(Error::IndexOutOfBounds(self.segment_index, segments.len()));
        }

        // Calculate start gap (gap before this segment)
        if self.segment_index == 0 {
            self.start_gap = segments.first().map(|s| s.timeline_offset);
        } else {
            let prev_segment = &segments[self.segment_index - 1];
            let target_segment = &segments[self.segment_index];
            let prev_end = prev_segment.timeline_offset + prev_segment.duration;
            self.start_gap = Some(target_segment.timeline_offset.saturating_sub(prev_end));
        }

        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.trim_start_gap(self.segment_index, self.shift_timeline)?;

        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();
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

        track.trim_end_gap(self.segment_index, self.shift_timeline)?;
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
            if self.shift_timeline {
                if self.segment_index < segments_count - 1 {
                    for i in (self.segment_index + 1)..segments_count {
                        track.shift_segment_timeline(i, end_gap)?;
                    }
                }
            } else if self.segment_index < segments_count - 1 {
                track.shift_segment_timeline(self.segment_index + 1, end_gap)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Remove gaps around segment {} in track {}",
            self.segment_index, self.track_index
        )
    }
}

pub struct RemoveSegmentLeftGapCommand {
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
    removed_gap: Option<Duration>,
}

impl RemoveSegmentLeftGapCommand {
    pub fn new(track_index: usize, segment_index: usize, shift_timeline: bool) -> Self {
        Self {
            track_index,
            segment_index,
            shift_timeline,
            removed_gap: None,
        }
    }
}

impl Command for RemoveSegmentLeftGapCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();

        if self.segment_index >= segments.len() {
            return Err(Error::IndexOutOfBounds(self.segment_index, segments.len()));
        }

        // Calculate the gap before this segment
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
            "Remove left gap from segment {} in track {}",
            self.segment_index, self.track_index
        )
    }
}

pub struct RemoveSegmentRightGapCommand {
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
    removed_gap: Option<Duration>,
}

impl RemoveSegmentRightGapCommand {
    pub fn new(track_index: usize, segment_index: usize, shift_timeline: bool) -> Self {
        Self {
            track_index,
            segment_index,
            shift_timeline,
            removed_gap: None,
        }
    }
}

impl Command for RemoveSegmentRightGapCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segments = track.segments();

        if self.segment_index >= segments.len() {
            return Err(Error::IndexOutOfBounds(self.segment_index, segments.len()));
        }

        // Calculate the gap after this segment
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
            "Remove right gap from segment {} in track {}",
            self.segment_index, self.track_index
        )
    }
}

// Command to shift all segments after a given index by a specified duration.
// Used after inserting segments in link mode to shift subsequent content.
pub struct ShiftSubsequentSegmentsCommand {
    pub track_index: usize,
    pub from_index: usize,
    pub shift_amount: Duration,
}

impl ShiftSubsequentSegmentsCommand {
    pub fn new(track_index: usize, from_index: usize, shift_amount: Duration) -> Self {
        Self {
            track_index,
            from_index,
            shift_amount,
        }
    }
}

impl Command for ShiftSubsequentSegmentsCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        for i in self.from_index..track.segments_count() {
            track.shift_segment_timeline(i, self.shift_amount)?;
        }

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        for i in self.from_index..track.segments_count() {
            track.shift_segment_timeline_backward(i, self.shift_amount)?;
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Shift segments from index {} by {:?} in track {}",
            self.from_index, self.shift_amount, self.track_index
        )
    }
}

// Command to merge two segments from the same source file.
// Requirements:
// 1. Same source file (metadata.path matches)
// 2. Segment1's source_offset < segment2's source_offset
// 3. Only for Video and Audio tracks
pub struct MergeSegmentsCommand {
    pub track_index: usize,
    pub first_segment_index: usize,
    pub second_segment_index: usize,

    // For undo
    original_first_duration: Option<Duration>,
    original_first_original_duration: Option<Duration>,
    original_playback_speed: Option<f32>,
    original_first_subtitle_text: Option<Option<String>>,
    removed_second_segment: Option<Arc<Segment>>,
    // Save entire filter lists for undo (not just counts)
    original_video_filters: Option<Vec<Arc<VideoFilterWrapper>>>,
    original_audio_filters: Option<Vec<Arc<AudioFilterWrapper>>>,
    original_subtitle_filters: Option<Vec<Arc<SubtitleFilterWrapper>>>,
    original_image_filters: Option<Vec<Arc<ImageFilterWrapper>>>,

    // Shift info for undo
    shift_amount: Option<Duration>,
    shifted_from_index: usize,
}

impl MergeSegmentsCommand {
    pub fn new(
        track_index: usize,
        first_segment_index: usize,
        second_segment_index: usize,
    ) -> Self {
        Self {
            track_index,
            first_segment_index,
            second_segment_index,
            original_first_duration: None,
            original_first_original_duration: None,
            original_playback_speed: None,
            original_first_subtitle_text: None,
            removed_second_segment: None,
            original_video_filters: None,
            original_audio_filters: None,
            original_subtitle_filters: None,
            original_image_filters: None,
            shift_amount: None,
            shifted_from_index: 0,
        }
    }
}

impl Command for MergeSegmentsCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        if !(track.is_video_or_audio() || track.is_subtitle()) {
            return Err(Error::InvalidConfig(
                "Merge only works on Video, Audio or Subtitle tracks".into(),
            ));
        }

        // Verify segments are adjacent on the track
        if self.second_segment_index != self.first_segment_index + 1 {
            return Err(Error::InvalidConfig(
                "Segments must be adjacent on the track".into(),
            ));
        }

        let is_subtitle = track.is_subtitle();

        let first_segment = track.get_segment(self.first_segment_index)?;
        let second_segment = track.get_segment(self.second_segment_index)?;

        if !is_subtitle {
            if first_segment.metadata.path != second_segment.metadata.path {
                return Err(Error::InvalidConfig(
                    "Segments must be from the same source file".into(),
                ));
            }

            if first_segment.source_offset >= second_segment.source_offset {
                return Err(Error::InvalidConfig(
                    "First segment's source_offset must be before second's".into(),
                ));
            }
        }

        // Save original state for undo
        self.original_first_duration = Some(first_segment.duration);
        self.original_first_original_duration = Some(first_segment.original_duration);
        self.original_playback_speed = Some(first_segment.playback_speed);
        self.original_first_subtitle_text = Some(first_segment.subtitle_text.clone());
        self.removed_second_segment = Some(second_segment.clone());
        self.original_video_filters = Some(first_segment.video_filters.clone());
        self.original_audio_filters = Some(first_segment.audio_filters.clone());
        self.original_subtitle_filters = Some(first_segment.subtitle_filters.clone());
        self.original_image_filters = Some(first_segment.image_filters.clone());

        // Subtitle merge: merge by timeline span, no requirement that the first
        // segment ends exactly when the second starts (gaps/overlaps are allowed).
        let (new_duration, new_original_duration) = if is_subtitle {
            let first_start = first_segment.timeline_offset;
            let first_end = first_segment.timeline_offset + first_segment.duration;
            let second_end = second_segment.timeline_offset + second_segment.duration;
            let span = second_end.max(first_end) - first_start;
            (span, span)
        } else {
            // Use the same playback_speed for merged segment (must be consistent)
            let playback_speed = first_segment.playback_speed;
            let source_end = second_segment.source_offset + second_segment.original_duration;
            let new_original_duration = source_end - first_segment.source_offset;
            let new_duration =
                Duration::from_secs_f64(new_original_duration.as_secs_f64() / playback_speed as f64);
            (new_duration, new_original_duration)
        };

        let original_total = first_segment.duration + second_segment.duration;

        // After removing second segment, shift subsequent segments if needed
        self.shifted_from_index = self.first_segment_index + 1;

        if new_duration > original_total {
            self.shift_amount = Some(new_duration - original_total);
        }

        // Clone second segment's filters for merging (before removing the segment)
        let second_video_filters = second_segment.video_filters.clone();
        let second_audio_filters = second_segment.audio_filters.clone();
        let second_subtitle_filters = second_segment.subtitle_filters.clone();
        let second_image_filters = second_segment.image_filters.clone();

        let merged_subtitle_text = if is_subtitle {
            Some(match (&first_segment.subtitle_text, &second_segment.subtitle_text) {
                (Some(a), Some(b)) => format!("{} {}", a.trim(), b.trim()),
                (Some(a), None) => a.clone(),
                (None, Some(b)) => b.clone(),
                (None, None) => String::new(),
            })
        } else {
            None
        };

        track.modify_segment(self.first_segment_index, |segment| {
            segment.duration = new_duration;
            segment.original_duration = new_original_duration;
            if let Some(text) = merged_subtitle_text {
                segment.subtitle_text = Some(text);
            }
            merge_filters_dedup_video(&mut segment.video_filters, &second_video_filters);
            merge_filters_dedup_audio(&mut segment.audio_filters, &second_audio_filters);
            merge_filters_dedup_subtitle(&mut segment.subtitle_filters, &second_subtitle_filters);
            merge_filters_dedup_image(&mut segment.image_filters, &second_image_filters);
        })?;

        track.remove_segment_leave_gap(self.second_segment_index)?;

        if let Some(shift) = self.shift_amount {
            let segments_count = track.segments_count();
            for i in self.shifted_from_index..segments_count {
                track.shift_segment_timeline(i, shift)?;
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;

        if let Some(original_duration) = self.original_first_duration {
            track.modify_segment(self.first_segment_index, |segment| {
                segment.duration = original_duration;
                if let Some(original_original_duration) = self.original_first_original_duration {
                    segment.original_duration = original_original_duration;
                }
                if let Some(speed) = self.original_playback_speed {
                    segment.playback_speed = speed;
                }
                if let Some(text) = self.original_first_subtitle_text.take() {
                    segment.subtitle_text = text;
                }
                if let Some(filters) = self.original_video_filters.take() {
                    segment.video_filters = filters;
                }
                if let Some(filters) = self.original_audio_filters.take() {
                    segment.audio_filters = filters;
                }
                if let Some(filters) = self.original_subtitle_filters.take() {
                    segment.subtitle_filters = filters;
                }
                if let Some(filters) = self.original_image_filters.take() {
                    segment.image_filters = filters;
                }
            })?;
        }

        // If we shifted segments, undo the shift first
        if let Some(shift) = self.shift_amount {
            let segments_count = track.segments_count();
            for i in self.shifted_from_index..segments_count {
                track.shift_segment_timeline_backward(i, shift)?;
            }
        }

        // Re-insert the second segment at its original position
        // After we removed second_segment, indices >= second_segment_index shifted down by 1
        // So we insert at the original second_segment_index
        if let Some(segment) = self.removed_second_segment.take() {
            track.insert_segment_preserve(self.second_segment_index, segment)?;
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Merge segments {} and {} in track {}",
            self.first_segment_index, self.second_segment_index, self.track_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_both_thumbnails(self.track_index, self.first_segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add_both_thumbnails(self.track_index, self.first_segment_index);
        affected.add_both_thumbnails(self.track_index, self.second_segment_index);
        affected
    }
}

macro_rules! impl_merge_filters_dedup {
    ($fn_name:ident, $wrapper_type:ty) => {
        fn $fn_name(target: &mut Vec<Arc<$wrapper_type>>, source: &[Arc<$wrapper_type>]) {
            for source_filter in source {
                let is_duplicate = target
                    .iter()
                    .any(|target_filter| target_filter.inner.name() == source_filter.inner.name());
                if !is_duplicate {
                    target.push(source_filter.clone());
                }
            }
        }
    };
}

impl_merge_filters_dedup!(merge_filters_dedup_video, VideoFilterWrapper);
impl_merge_filters_dedup!(merge_filters_dedup_audio, AudioFilterWrapper);
impl_merge_filters_dedup!(merge_filters_dedup_subtitle, SubtitleFilterWrapper);
impl_merge_filters_dedup!(merge_filters_dedup_image, ImageFilterWrapper);

pub struct AddTextSegmentCommand {
    pub track_index: usize,
    pub element: TextElement,
    pub timeline_offset: Duration,
    pub duration: Duration,
    segment_id: Option<uuid::Uuid>,
}

impl AddTextSegmentCommand {
    pub fn new(
        track_index: usize,
        element: TextElement,
        timeline_offset: Duration,
        duration: Duration,
    ) -> Self {
        Self {
            track_index,
            element,
            timeline_offset,
            duration,
            segment_id: None,
        }
    }
}

impl Command for AddTextSegmentCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.duration.is_zero() {
            return Err(Error::InvalidConfig(
                "Cannot add text segment with zero duration".into(),
            ));
        }

        let tracks_len = manager.tracks.len();
        let global_speed = manager.get_global_speed();
        let track = manager
            .tracks
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, tracks_len))?;

        match track {
            Track::Text(inner) => {
                let text_track = Arc::make_mut(inner);
                let metadata = Arc::new(Metadata::new_text());
                let segment = Arc::new(
                    Segment::new(self.timeline_offset, self.duration, metadata, global_speed)
                        .with_text_element(self.element.clone()),
                );
                self.segment_id =
                    Some(uuid::Uuid::parse_str(&segment.uuid).unwrap_or(uuid::Uuid::nil()));
                text_track.track.segments.push(segment);
                text_track.update_duration();
            }
            _ => {
                return Err(Error::InvalidConfig("Track is not a TextTrack".into()));
            }
        }

        let track = manager.tracks.get_mut(self.track_index).unwrap();
        track.sort_segments_by_timeline_offset();

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let Some(segment_id) = self.segment_id else {
            return Ok(());
        };

        let tracks_len = manager.tracks.len();
        let track = manager
            .tracks
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, tracks_len))?;

        match track {
            Track::Text(inner) => {
                let text_track = Arc::make_mut(inner);
                let segment_uuid = segment_id.to_string();
                let idx = text_track
                    .track
                    .segments
                    .iter()
                    .position(|seg| seg.uuid == segment_uuid);
                if let Some(idx) = idx {
                    text_track.track.segments.remove(idx);
                    text_track.update_duration();
                }
            }
            _ => {
                return Err(Error::InvalidConfig("Track is not a TextTrack".into()));
            }
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Add text segment to track {}", self.track_index)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments::new()
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        AffectedSegments::new()
    }
}

pub struct ShiftSegmentsAfterTimeCommand {
    track_index: usize,
    start_time: Duration,
    shift_amount: Duration,
    // Stores (segment_index, original_offset) for undo
    original_offsets: Option<Vec<(usize, Duration)>>,
}

impl ShiftSegmentsAfterTimeCommand {
    pub fn new(track_index: usize, start_time: Duration, shift_amount: Duration) -> Self {
        Self {
            track_index,
            start_time,
            shift_amount,
            original_offsets: None,
        }
    }
}

impl Command for ShiftSegmentsAfterTimeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        let segments = track.segments();

        // Find all segments starting after start_time and store their original offsets
        let offsets_to_shift: Vec<(usize, Duration)> = segments
            .iter()
            .enumerate()
            .filter(|(_, seg)| seg.timeline_offset >= self.start_time)
            .map(|(idx, seg)| (idx, seg.timeline_offset))
            .collect();

        self.original_offsets = Some(offsets_to_shift.clone());

        for (idx, _) in &offsets_to_shift {
            track.shift_segment_timeline(*idx, self.shift_amount)?;
        }

        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let offsets = self
            .original_offsets
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let track = get_track_from_manager(manager, self.track_index)?;

        // Restore original offsets
        for (idx, original_offset) in offsets {
            track.modify_segment(idx, |seg| {
                seg.timeline_offset = original_offset;
            })?;
        }

        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Shift segments after {:?} by {:?} in track {}",
            self.start_time, self.shift_amount, self.track_index
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        if let Some(ref offsets) = self.original_offsets {
            let mut affected = AffectedSegments::new();
            for (idx, _) in offsets {
                affected.add_both_thumbnails(self.track_index, *idx);
            }
            affected
        } else {
            AffectedSegments::default()
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct AddTextKeyframeCommand {
    track_index: usize,
    segment_index: usize,
    property_name: String,
    keyframe: Keyframe,
}

impl AddTextKeyframeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        property_name: String,
        time_ms: i64,
        value: KeyframeValue,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            property_name,
            keyframe: Keyframe::new(time_ms, value),
        }
    }

    pub fn new_with_keyframe(
        track_index: usize,
        segment_index: usize,
        property_name: String,
        keyframe: Keyframe,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            property_name,
            keyframe,
        }
    }
}

impl Command for AddTextKeyframeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            if let Some(ref text_element) = segment.text_element {
                let mut new_element = text_element.clone();
                new_element
                    .keyframe_tracks
                    .add_keyframe(&self.property_name, self.keyframe.clone());
                segment.text_element = Some(new_element);
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let remove_cmd = RemoveTextKeyframeCommand::new_with_keyframe(
            self.track_index,
            self.segment_index,
            self.property_name.clone(),
            self.keyframe.clone(),
        );
        let mut remove_cmd = remove_cmd;
        remove_cmd.execute(manager)
    }

    fn describe(&self) -> String {
        format!(
            "Add keyframe at {}ms to '{}' on text segment",
            self.keyframe.time_ms, self.property_name
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct RemoveTextKeyframeCommand {
    track_index: usize,
    segment_index: usize,
    property_name: String,
    time_ms: i64,
    removed_keyframe: Option<Keyframe>,
}

impl RemoveTextKeyframeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        property_name: String,
        time_ms: i64,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            property_name,
            time_ms,
            removed_keyframe: None,
        }
    }

    pub fn new_with_keyframe(
        track_index: usize,
        segment_index: usize,
        property_name: String,
        keyframe: Keyframe,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            property_name,
            time_ms: keyframe.time_ms,
            removed_keyframe: Some(keyframe),
        }
    }
}

impl Command for RemoveTextKeyframeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.removed_keyframe.is_none()
            && let Some(track) = manager.get(self.track_index)
            && let Ok(segment) = track.get_segment(self.segment_index)
            && let Some(ref text_element) = segment.text_element
            && let Some(prop_track) = text_element.keyframe_tracks.get_track(&self.property_name)
        {
            self.removed_keyframe = prop_track
                .keyframes
                .iter()
                .find(|kf| kf.time_ms == self.time_ms)
                .cloned();
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            if let Some(ref text_element) = segment.text_element {
                let mut new_element = text_element.clone();
                new_element
                    .keyframe_tracks
                    .remove_keyframe(&self.property_name, self.time_ms);
                segment.text_element = Some(new_element);
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(ref keyframe) = self.removed_keyframe {
            let add_cmd = AddTextKeyframeCommand::new_with_keyframe(
                self.track_index,
                self.segment_index,
                self.property_name.clone(),
                keyframe.clone(),
            );
            let mut add_cmd = add_cmd;
            add_cmd.execute(manager)
        } else {
            Err(Error::InvalidConfig(
                "Cannot undo: removed keyframe not saved".into(),
            ))
        }
    }

    fn describe(&self) -> String {
        format!(
            "Remove keyframe at {}ms from '{}' on text segment",
            self.time_ms, self.property_name
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct MoveTextKeyframeCommand {
    track_index: usize,
    segment_index: usize,
    property_name: String,
    old_time_ms: i64,
    new_time_ms: i64,
    keyframe_value: Option<KeyframeValue>,
}

impl MoveTextKeyframeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        property_name: String,
        old_time_ms: i64,
        new_time_ms: i64,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            property_name,
            old_time_ms,
            new_time_ms,
            keyframe_value: None,
        }
    }
}

impl Command for MoveTextKeyframeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.keyframe_value.is_none()
            && let Some(track) = manager.get(self.track_index)
            && let Ok(segment) = track.get_segment(self.segment_index)
            && let Some(ref text_element) = segment.text_element
            && let Some(prop_track) = text_element.keyframe_tracks.get_track(&self.property_name)
        {
            self.keyframe_value = prop_track
                .keyframes
                .iter()
                .find(|kf| kf.time_ms == self.old_time_ms)
                .map(|kf| kf.value.clone());
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            if let Some(ref text_element) = segment.text_element {
                let mut new_element = text_element.clone();
                new_element.keyframe_tracks.move_keyframe(
                    &self.property_name,
                    self.old_time_ms,
                    self.new_time_ms,
                );
                segment.text_element = Some(new_element);
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            if let Some(ref text_element) = segment.text_element {
                let mut new_element = text_element.clone();
                new_element.keyframe_tracks.move_keyframe(
                    &self.property_name,
                    self.new_time_ms,
                    self.old_time_ms,
                );
                segment.text_element = Some(new_element);
            }
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Move keyframe from {}ms to {}ms on '{}' on text segment",
            self.old_time_ms, self.new_time_ms, self.property_name
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct UpdateTextKeyframeValueCommand {
    track_index: usize,
    segment_index: usize,
    property_name: String,
    time_ms: i64,
    new_value: KeyframeValue,
    old_value: Option<KeyframeValue>,
}

impl UpdateTextKeyframeValueCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        property_name: String,
        time_ms: i64,
        new_value: KeyframeValue,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            property_name,
            time_ms,
            new_value,
            old_value: None,
        }
    }
}

impl Command for UpdateTextKeyframeValueCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.old_value.is_none()
            && let Some(track) = manager.get(self.track_index)
            && let Ok(segment) = track.get_segment(self.segment_index)
            && let Some(ref text_element) = segment.text_element
            && let Some(prop_track) = text_element.keyframe_tracks.get_track(&self.property_name)
        {
            self.old_value = prop_track
                .keyframes
                .iter()
                .find(|kf| kf.time_ms == self.time_ms)
                .map(|kf| kf.value.clone());
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            if let Some(ref text_element) = segment.text_element {
                let mut new_element = text_element.clone();
                new_element.keyframe_tracks.update_keyframe_value(
                    &self.property_name,
                    self.time_ms,
                    self.new_value.clone(),
                );
                segment.text_element = Some(new_element);
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(ref old_value) = self.old_value {
            let manager_len = manager.len();
            let track = manager
                .get_mut(self.track_index)
                .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

            track.modify_segment(self.segment_index, |segment| {
                if let Some(ref text_element) = segment.text_element {
                    let mut new_element = text_element.clone();
                    new_element.keyframe_tracks.update_keyframe_value(
                        &self.property_name,
                        self.time_ms,
                        old_value.clone(),
                    );
                    segment.text_element = Some(new_element);
                }
            })?;

            Ok(())
        } else {
            Err(Error::InvalidConfig(
                "Cannot undo: old keyframe value not saved".into(),
            ))
        }
    }

    fn describe(&self) -> String {
        format!(
            "Update keyframe value at {}ms for '{}' on text segment",
            self.time_ms, self.property_name
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct ClearSegmentKeyframesCommand {
    track_index: usize,
    segment_index: usize,
    saved_video_filter_keyframes: Vec<KeyframeTracks>,
    saved_audio_filter_keyframes: Vec<KeyframeTracks>,
    saved_image_filter_keyframes: Vec<KeyframeTracks>,
    saved_text_keyframes: Option<KeyframeTracks>,
}

impl ClearSegmentKeyframesCommand {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            saved_video_filter_keyframes: Vec::new(),
            saved_audio_filter_keyframes: Vec::new(),
            saved_image_filter_keyframes: Vec::new(),
            saved_text_keyframes: None,
        }
    }
}

impl Command for ClearSegmentKeyframesCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(track) = manager.get(self.track_index) {
            if let Ok(segment) = track.get_segment(self.segment_index) {
                self.saved_video_filter_keyframes = segment
                    .video_filters
                    .iter()
                    .map(|f| f.inner.get_keyframe_tracks())
                    .collect();
                self.saved_audio_filter_keyframes = segment
                    .audio_filters
                    .iter()
                    .map(|f| f.inner.get_keyframe_tracks())
                    .collect();
                self.saved_image_filter_keyframes = segment
                    .image_filters
                    .iter()
                    .map(|f| f.inner.get_keyframe_tracks())
                    .collect();

                if let Some(ref text_element) = segment.text_element {
                    self.saved_text_keyframes = Some(text_element.keyframe_tracks.clone());
                }
            }
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            let new_video_filters: Vec<Arc<VideoFilterWrapper>> = segment
                .video_filters
                .iter()
                .map(|wrapper| {
                    let mut new_filter = wrapper.inner.clone_box();
                    new_filter.set_keyframe_tracks(KeyframeTracks::default());
                    Arc::new(VideoFilterWrapper::new(wrapper.enabled(), new_filter))
                })
                .collect();
            segment.video_filters = new_video_filters;

            let new_audio_filters: Vec<Arc<AudioFilterWrapper>> = segment
                .audio_filters
                .iter()
                .map(|wrapper| {
                    let mut new_filter = wrapper.inner.clone_box();
                    new_filter.set_keyframe_tracks(KeyframeTracks::default());
                    Arc::new(AudioFilterWrapper::new(wrapper.enabled(), new_filter))
                })
                .collect();
            segment.audio_filters = new_audio_filters;

            let new_image_filters: Vec<Arc<ImageFilterWrapper>> = segment
                .image_filters
                .iter()
                .map(|wrapper| {
                    let mut new_filter = wrapper.inner.clone_box();
                    new_filter.set_keyframe_tracks(KeyframeTracks::default());
                    Arc::new(ImageFilterWrapper::new(wrapper.enabled(), new_filter))
                })
                .collect();
            segment.image_filters = new_image_filters;

            if let Some(ref mut text_element) = segment.text_element {
                text_element.keyframe_tracks.clear();
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            let saved_video = &self.saved_video_filter_keyframes;
            let new_video_filters: Vec<Arc<VideoFilterWrapper>> = segment
                .video_filters
                .iter()
                .enumerate()
                .map(|(i, wrapper)| {
                    let mut new_filter = wrapper.inner.clone_box();
                    if i < saved_video.len() {
                        new_filter.set_keyframe_tracks(saved_video[i].clone());
                    }
                    Arc::new(VideoFilterWrapper::new(wrapper.enabled(), new_filter))
                })
                .collect();
            segment.video_filters = new_video_filters;

            let saved_audio = &self.saved_audio_filter_keyframes;
            let new_audio_filters: Vec<Arc<AudioFilterWrapper>> = segment
                .audio_filters
                .iter()
                .enumerate()
                .map(|(i, wrapper)| {
                    let mut new_filter = wrapper.inner.clone_box();
                    if i < saved_audio.len() {
                        new_filter.set_keyframe_tracks(saved_audio[i].clone());
                    }
                    Arc::new(AudioFilterWrapper::new(wrapper.enabled(), new_filter))
                })
                .collect();
            segment.audio_filters = new_audio_filters;

            let saved_image = &self.saved_image_filter_keyframes;
            let new_image_filters: Vec<Arc<ImageFilterWrapper>> = segment
                .image_filters
                .iter()
                .enumerate()
                .map(|(i, wrapper)| {
                    let mut new_filter = wrapper.inner.clone_box();
                    if i < saved_image.len() {
                        new_filter.set_keyframe_tracks(saved_image[i].clone());
                    }
                    Arc::new(ImageFilterWrapper::new(wrapper.enabled(), new_filter))
                })
                .collect();
            segment.image_filters = new_image_filters;

            if let Some(ref saved_tracks) = self.saved_text_keyframes
                && let Some(ref mut text_element) = segment.text_element
            {
                text_element.keyframe_tracks = saved_tracks.clone();
            }
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        "Clear all keyframes from segment".to_string()
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct ClearSegmentFiltersCommand {
    track_index: usize,
    segment_index: usize,
    saved_video_filters: Vec<VideoFilterWrapper>,
    saved_audio_filters: Vec<AudioFilterWrapper>,
    saved_image_filters: Vec<ImageFilterWrapper>,
}

impl ClearSegmentFiltersCommand {
    pub fn new(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            saved_video_filters: Vec::new(),
            saved_audio_filters: Vec::new(),
            saved_image_filters: Vec::new(),
        }
    }
}

impl Command for ClearSegmentFiltersCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(track) = manager.get(self.track_index)
            && let Ok(segment) = track.get_segment(self.segment_index)
        {
            self.saved_video_filters = segment
                .video_filters
                .iter()
                .map(|f| f.as_ref().clone())
                .collect();
            self.saved_audio_filters = segment
                .audio_filters
                .iter()
                .map(|f| f.as_ref().clone())
                .collect();
            self.saved_image_filters = segment
                .image_filters
                .iter()
                .map(|f| f.as_ref().clone())
                .collect();
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            segment.video_filters.clear();
            segment.audio_filters.clear();
            segment.image_filters.clear();
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| {
            segment.video_filters = self
                .saved_video_filters
                .iter()
                .map(|item| Arc::new(item.clone()))
                .collect();

            segment.audio_filters = self
                .saved_audio_filters
                .iter()
                .map(|item| Arc::new(item.clone()))
                .collect();
            segment.image_filters = self
                .saved_image_filters
                .iter()
                .map(|item| Arc::new(item.clone()))
                .collect();
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        "Clear all filter from segment".to_string()
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

pub struct SetPlaybackSpeedCommand {
    pub track_index: usize,
    pub segment_index: usize,
    pub new_speed: f32,
    pub old_speed: f32,
    pub old_duration: Duration,
}

impl SetPlaybackSpeedCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        new_speed: f32,
        old_speed: f32,
        old_duration: Duration,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            new_speed,
            old_speed,
            old_duration,
        }
    }
}

impl Command for SetPlaybackSpeedCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.playback_speed = self.new_speed;
            segment.duration = Duration::from_secs_f64(
                segment.original_duration.as_secs_f64() / self.new_speed as f64,
            );
        })?;
        manager.update_duration();
        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let track = get_track_from_manager(manager, self.track_index)?;
        track.modify_segment(self.segment_index, |segment| {
            segment.playback_speed = self.old_speed;
            segment.duration = self.old_duration;
        })?;
        manager.update_duration();
        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Set playback speed of track {} segment {} to {}",
            self.track_index, self.segment_index, self.new_speed
        )
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut affected = AffectedSegments::new();
        affected.add(self.track_index, self.segment_index);
        affected
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

struct SegmentGlobalSpeedState {
    track_index: usize,
    segment_index: usize,
    timeline_offset: Duration,
    duration: Duration,
    global_speed: f32,
}

pub struct SetGlobalSpeedCommand {
    old_speed: f32,
    new_speed: f32,
    original_states: Vec<SegmentGlobalSpeedState>,
}

impl SetGlobalSpeedCommand {
    pub fn new(old_speed: f32, new_speed: f32) -> Self {
        Self {
            old_speed,
            new_speed,
            original_states: Vec::new(),
        }
    }
}

impl Command for SetGlobalSpeedCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.old_speed == self.new_speed {
            return Ok(());
        }

        if self.original_states.is_empty() {
            self.save_original_states(manager);
        }

        self.apply_speed(manager, self.old_speed, self.new_speed);

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        self.restore_original_states(manager);
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Set global speed: {} -> {}", self.old_speed, self.new_speed)
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        AffectedSegments {
            segments: vec![],
            tracks_changed: true,
        }
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        self.affected_segments_after_execute()
    }
}

impl SetGlobalSpeedCommand {
    fn save_original_states(&mut self, manager: &Manager) {
        for (track_idx, track) in manager.tracks.iter().enumerate() {
            for (seg_idx, seg) in track.segments().iter().enumerate() {
                self.original_states.push(SegmentGlobalSpeedState {
                    track_index: track_idx,
                    segment_index: seg_idx,
                    timeline_offset: seg.timeline_offset,
                    duration: seg.duration,
                    global_speed: seg.global_speed,
                });
            }
        }
    }

    fn apply_speed(&self, manager: &mut Manager, old_speed: f32, new_speed: f32) {
        if old_speed == new_speed {
            return;
        }

        let speed_ratio = new_speed / old_speed;

        for track in manager.tracks.iter_mut() {
            let segments_count = track.segments_count();
            for seg_idx in 0..segments_count {
                track
                    .modify_segment(seg_idx, |seg| {
                        seg.global_speed = new_speed;

                        seg.duration = Duration::from_secs_f64(
                            seg.original_duration.as_secs_f64()
                                / (seg.playback_speed * new_speed) as f64,
                        );

                        // Scale timeline_offset proportionally to maintain relative positions
                        // across all tracks. New offset = old offset / speed_ratio.
                        // Segments at offset 0 stay at 0 (0/ratio = 0).
                        seg.timeline_offset = Duration::from_secs_f64(
                            seg.timeline_offset.as_secs_f64() / speed_ratio as f64,
                        );
                    })
                    .ok();
            }
        }
        manager.update_duration();
    }

    fn restore_original_states(&self, manager: &mut Manager) {
        for state in &self.original_states {
            if let Some(track) = manager.get_mut(state.track_index) {
                track
                    .modify_segment(state.segment_index, |seg| {
                        seg.timeline_offset = state.timeline_offset;
                        seg.duration = state.duration;
                        seg.global_speed = state.global_speed;
                    })
                    .ok();
            }
        }
        manager.update_duration();
    }
}
