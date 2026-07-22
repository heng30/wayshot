use super::command::Command;
use crate::{
    Error, Result,
    filters::traits::SubtitleEntry,
    metadata::Metadata,
    tracks::{manager::Manager, segment::Segment, track::Track},
};
use std::{sync::Arc, time::Duration};

pub struct AddSubtitleCommand {
    pub track_index: usize,
    pub entry: SubtitleEntry,
    pub inserted_index: Option<usize>,
}

impl AddSubtitleCommand {
    pub fn new(track_index: usize, entry: SubtitleEntry) -> Self {
        Self {
            track_index,
            entry,
            inserted_index: None,
        }
    }
}

impl Command for AddSubtitleCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match track {
            Track::Subtitle(inner) => {
                let track = Arc::make_mut(inner);
                log::debug!(
                    "AddSubtitleCommand: Before insert - track {} has {} segments. start={:?}, end={:?}",
                    self.track_index,
                    track.track.segments.len(),
                    self.entry.start,
                    self.entry.end,
                );

                let insert_pos = track
                    .track
                    .segments
                    .iter()
                    .position(|seg| seg.timeline_offset > self.entry.start)
                    .unwrap_or(track.track.segments.len());

                let segment_duration = self.entry.end.saturating_sub(self.entry.start);
                // 字幕segment的时长由用户直接指定（timeline时间），不需要受global_speed影响
                // 因为用户输入的时间范围已经是timeline坐标系中的时间
                let segment = Arc::new(
                    Segment::new_with_source_offset(
                        self.entry.start,
                        Duration::ZERO,
                        segment_duration,
                        1.0,
                        1.0, // 字幕segment不受global_speed影响
                        Arc::new(Metadata::new_subtitle()),
                    )
                    .with_subtitle_text(&self.entry.text),
                );

                track.track.segments.insert(insert_pos, segment);
                self.inserted_index = Some(insert_pos);
                track.update_duration();
            }
            _ => {
                return Err(Error::InvalidConfig(
                    "Cannot add subtitle to non-subtitle track".into(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let index = self
            .inserted_index
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Subtitle(inner) = track {
            let track = Arc::make_mut(inner);
            track.track.segments.remove(index);
            track.update_duration();
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Add subtitle to track {} at {:?}",
            self.track_index, self.entry.start
        )
    }
}

pub struct UpdateSubtitleCommand {
    pub track_index: usize,
    pub subtitle_index: usize,
    pub new_entry: SubtitleEntry,
    pub old_entry: Option<SubtitleEntry>,
}

impl UpdateSubtitleCommand {
    pub fn new(track_index: usize, subtitle_index: usize, new_entry: SubtitleEntry) -> Self {
        Self {
            track_index,
            subtitle_index,
            new_entry,
            old_entry: None,
        }
    }
}

impl Command for UpdateSubtitleCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match track {
            Track::Subtitle(inner) => {
                let track = Arc::make_mut(inner);

                if self.subtitle_index >= track.track.segments.len() {
                    return Err(Error::IndexOutOfBounds(
                        self.subtitle_index,
                        track.track.segments.len(),
                    ));
                }

                let old_segment = &track.track.segments[self.subtitle_index];
                self.old_entry = Some(SubtitleEntry {
                    start: old_segment.timeline_offset,
                    end: old_segment.timeline_offset + old_segment.duration,
                    text: old_segment.subtitle_text.clone().unwrap_or_default(),
                });

                let segment_duration = self.new_entry.end.saturating_sub(self.new_entry.start);
                let segment = Arc::make_mut(&mut track.track.segments[self.subtitle_index]);
                segment.timeline_offset = self.new_entry.start;
                segment.source_offset = self.new_entry.start;
                segment.duration = segment_duration;
                segment.subtitle_text = Some(self.new_entry.text.clone());

                // Re-sort segments by timeline_offset if needed
                let mut needs_sort = false;
                if self.subtitle_index > 0
                    && track.track.segments[self.subtitle_index].timeline_offset
                        < track.track.segments[self.subtitle_index - 1].timeline_offset
                {
                    needs_sort = true;
                } else if self.subtitle_index < track.track.segments.len() - 1
                    && track.track.segments[self.subtitle_index].timeline_offset
                        > track.track.segments[self.subtitle_index + 1].timeline_offset
                {
                    needs_sort = true;
                }

                if needs_sort {
                    track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                }

                track.update_duration();
            }
            _ => {
                return Err(Error::InvalidConfig(
                    "Cannot update subtitle in non-subtitle track".into(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let old_entry = self
            .old_entry
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Subtitle(inner) = track {
            let track = Arc::make_mut(inner);

            // Find the entry by start time (since position may have changed due to sort)
            if let Some(index) = track
                .track
                .segments
                .iter()
                .position(|seg| seg.timeline_offset == self.new_entry.start)
            {
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset = old_entry.start;
                segment.source_offset = old_entry.start;
                segment.duration = old_entry.end.saturating_sub(old_entry.start);
                segment.subtitle_text = Some(old_entry.text.clone());

                track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                track.update_duration();
            }
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Update subtitle at index {} in track {}",
            self.subtitle_index, self.track_index
        )
    }
}

pub struct RemoveSubtitleCommand {
    pub track_index: usize,
    pub subtitle_index: usize,
    pub removed_segment: Option<Arc<Segment>>,
}

impl RemoveSubtitleCommand {
    pub fn new(track_index: usize, subtitle_index: usize) -> Self {
        Self {
            track_index,
            subtitle_index,
            removed_segment: None,
        }
    }
}

impl Command for RemoveSubtitleCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match track {
            Track::Subtitle(inner) => {
                let track = Arc::make_mut(inner);

                if self.subtitle_index >= track.track.segments.len() {
                    return Err(Error::IndexOutOfBounds(
                        self.subtitle_index,
                        track.track.segments.len(),
                    ));
                }

                self.removed_segment = Some(track.track.segments.remove(self.subtitle_index));
                track.update_duration();
            }
            _ => {
                return Err(Error::InvalidConfig(
                    "Cannot remove subtitle from non-subtitle track".into(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let segment = self
            .removed_segment
            .take()
            .ok_or_else(|| Error::InvalidConfig("Command not yet executed".into()))?;

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Track::Subtitle(inner) = track {
            let track = Arc::make_mut(inner);

            // Find the correct position to re-insert (sorted by timeline_offset)
            let insert_pos = track
                .track
                .segments
                .iter()
                .position(|seg| seg.timeline_offset > segment.timeline_offset)
                .unwrap_or(track.track.segments.len());

            track.track.segments.insert(insert_pos, segment);
            track.update_duration();
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Remove subtitle at index {} from track {}",
            self.subtitle_index, self.track_index
        )
    }
}
