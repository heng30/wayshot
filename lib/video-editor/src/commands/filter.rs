use super::command::Command;
use crate::{
    Error, Result,
    filters::{
        keyframe::{Keyframe, KeyframeValue},
        traits::{
            AudioFilter, AudioFilterWrapper, ImageFilterWrapper, SubtitleFilter,
            SubtitleFilterWrapper, VideoFilter, VideoFilterWrapper,
        },
    },
    tracks::manager::Manager,
};
use std::{
    any::Any,
    sync::{Arc, atomic::Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Video,
    Audio,
    Subtitle,
    Image,
}

impl ToString for FilterType {
    fn to_string(&self) -> String {
        match self {
            FilterType::Video => "video".to_string(),
            FilterType::Audio => "audio".to_string(),
            FilterType::Subtitle => "subtitle".to_string(),
            FilterType::Image => "image".to_string(),
        }
    }
}

pub struct AddFilterCommand {
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    filter: Box<dyn Any + Send + Sync>,
}

impl AddFilterCommand {
    pub fn new_video(
        track_index: usize,
        segment_index: usize,
        filter: Box<dyn VideoFilter>,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Video,
            filter: Box::new(filter),
        }
    }

    pub fn new_audio(
        track_index: usize,
        segment_index: usize,
        filter: Box<dyn AudioFilter>,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Audio,
            filter: Box::new(filter),
        }
    }

    pub fn new_subtitle(
        track_index: usize,
        segment_index: usize,
        filter: Box<dyn SubtitleFilter>,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Subtitle,
            filter: Box::new(filter),
        }
    }

    pub fn new_image(track_index: usize, segment_index: usize, filter: ImageFilterWrapper) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Image,
            filter: Box::new(filter),
        }
    }
}

impl Command for AddFilterCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => {
                if let Some(filter) = self.filter.downcast_ref::<Box<dyn VideoFilter>>() {
                    segment.add_video_filter(filter.clone_box());
                }
            }
            FilterType::Audio => {
                if let Some(filter) = self.filter.downcast_ref::<Box<dyn AudioFilter>>() {
                    segment.add_audio_filter(filter.clone_box());
                }
            }
            FilterType::Subtitle => {
                if let Some(filter) = self.filter.downcast_ref::<Box<dyn SubtitleFilter>>() {
                    segment.add_subtitle_filter(filter.clone_box());
                }
            }
            FilterType::Image => {
                if let Some(filter) = self.filter.downcast_ref::<ImageFilterWrapper>() {
                    segment.add_image_filter(filter.clone());
                }
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => {
                let count = segment.video_filters.len();
                if count > 0 {
                    _ = segment.remove_video_filter(count - 1);
                }
            }
            FilterType::Audio => {
                let count = segment.audio_filters.len();
                if count > 0 {
                    _ = segment.remove_audio_filter(count - 1);
                }
            }
            FilterType::Subtitle => {
                let count = segment.subtitle_filters.len();
                if count > 0 {
                    _ = segment.remove_subtitle_filter(count - 1);
                }
            }
            FilterType::Image => {
                let count = segment.image_filters.len();
                if count > 0 {
                    _ = segment.remove_image_filter(count - 1);
                }
            }
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Add {:?} filter to segment {}",
            self.filter_type, self.segment_index
        )
    }
}

pub struct InsertFilterCommand {
    track_index: usize,
    segment_index: usize,
    filter_index: usize,
    filter_type: FilterType,
    filter: Box<dyn Any + Send + Sync>,
}

impl InsertFilterCommand {
    pub fn new_video(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        filter: Box<dyn VideoFilter>,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type: FilterType::Video,
            filter: Box::new(filter),
        }
    }

    pub fn new_audio(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        filter: Box<dyn AudioFilter>,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type: FilterType::Audio,
            filter: Box::new(filter),
        }
    }

    pub fn new_subtitle(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        filter: Box<dyn SubtitleFilter>,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type: FilterType::Subtitle,
            filter: Box::new(filter),
        }
    }

    pub fn new_image(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        filter: ImageFilterWrapper,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type: FilterType::Image,
            filter: Box::new(filter),
        }
    }
}

impl Command for InsertFilterCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => {
                if let Some(filter) = self.filter.downcast_ref::<Box<dyn VideoFilter>>() {
                    _ = segment.insert_video_filter(self.filter_index, filter.clone_box());
                }
            }
            FilterType::Audio => {
                if let Some(filter) = self.filter.downcast_ref::<Box<dyn AudioFilter>>() {
                    _ = segment.insert_audio_filter(self.filter_index, filter.clone_box());
                }
            }
            FilterType::Subtitle => {
                if let Some(filter) = self.filter.downcast_ref::<Box<dyn SubtitleFilter>>() {
                    _ = segment.insert_subtitle_filter(self.filter_index, filter.clone_box());
                }
            }
            FilterType::Image => {
                if let Some(filter) = self.filter.downcast_ref::<ImageFilterWrapper>() {
                    _ = segment.insert_image_filter(self.filter_index, filter.clone());
                }
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => _ = segment.remove_video_filter(self.filter_index),
            FilterType::Audio => _ = segment.remove_audio_filter(self.filter_index),
            FilterType::Subtitle => _ = segment.remove_subtitle_filter(self.filter_index),
            FilterType::Image => _ = segment.remove_image_filter(self.filter_index),
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Insert {:?} filter at index {} in segment {}",
            self.filter_type, self.filter_index, self.segment_index
        )
    }
}

pub struct RemoveFilterCommand {
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    filter_index: usize,
    removed_filter: Option<Box<dyn Any + Send + Sync>>,
}

impl RemoveFilterCommand {
    pub fn new_video(track_index: usize, segment_index: usize, filter_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Video,
            filter_index,
            removed_filter: None,
        }
    }

    pub fn new_audio(track_index: usize, segment_index: usize, filter_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Audio,
            filter_index,
            removed_filter: None,
        }
    }

    pub fn new_subtitle(track_index: usize, segment_index: usize, filter_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Subtitle,
            filter_index,
            removed_filter: None,
        }
    }

    pub fn new_image(track_index: usize, segment_index: usize, filter_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Image,
            filter_index,
            removed_filter: None,
        }
    }
}

impl Command for RemoveFilterCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segment = track.get_segment(self.segment_index)?;
        match self.filter_type {
            FilterType::Video => {
                let filter = segment
                    .video_filters
                    .get(self.filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(self.filter_index, segment.video_filters.len())
                    })?;
                self.removed_filter = Some(Box::new(filter.clone()));
            }
            FilterType::Audio => {
                let filter = segment
                    .audio_filters
                    .get(self.filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(self.filter_index, segment.audio_filters.len())
                    })?;
                self.removed_filter = Some(Box::new(filter.clone()));
            }
            FilterType::Subtitle => {
                let filter = segment
                    .subtitle_filters
                    .get(self.filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(self.filter_index, segment.subtitle_filters.len())
                    })?;
                self.removed_filter = Some(Box::new(filter.clone()));
            }
            FilterType::Image => {
                let filter = segment
                    .image_filters
                    .get(self.filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(self.filter_index, segment.image_filters.len())
                    })?;
                self.removed_filter = Some(Box::new(filter.clone()));
            }
        }

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => _ = segment.remove_video_filter(self.filter_index),
            FilterType::Audio => _ = segment.remove_audio_filter(self.filter_index),
            FilterType::Subtitle => _ = segment.remove_subtitle_filter(self.filter_index),
            FilterType::Image => _ = segment.remove_image_filter(self.filter_index),
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => {
                if let Some(wrapper) = self
                    .removed_filter
                    .as_ref()
                    .and_then(|f| f.downcast_ref::<Arc<VideoFilterWrapper>>())
                {
                    _ = segment.insert_video_filter(self.filter_index, wrapper.inner.clone_box());
                    _ = segment.set_video_filter_enabled(self.filter_index, wrapper.enabled());
                }
            }
            FilterType::Audio => {
                if let Some(wrapper) = self
                    .removed_filter
                    .as_ref()
                    .and_then(|f| f.downcast_ref::<Arc<AudioFilterWrapper>>())
                {
                    _ = segment.insert_audio_filter(self.filter_index, wrapper.inner.clone_box());
                    _ = segment.set_audio_filter_enabled(self.filter_index, wrapper.enabled());
                }
            }
            FilterType::Subtitle => {
                if let Some(wrapper) = self
                    .removed_filter
                    .as_ref()
                    .and_then(|f| f.downcast_ref::<Arc<SubtitleFilterWrapper>>())
                {
                    _ = segment
                        .insert_subtitle_filter(self.filter_index, wrapper.inner.clone_box());
                    _ = segment.set_subtitle_filter_enabled(self.filter_index, wrapper.enabled());
                }
            }
            FilterType::Image => {
                if let Some(wrapper) = self
                    .removed_filter
                    .as_ref()
                    .and_then(|f| f.downcast_ref::<Arc<ImageFilterWrapper>>())
                {
                    _ = segment.insert_image_filter(self.filter_index, wrapper.as_ref().clone());
                    _ = segment.set_image_filter_enabled(self.filter_index, wrapper.enabled());
                }
            }
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Remove {:?} filter {} from segment {}",
            self.filter_type, self.filter_index, self.segment_index
        )
    }
}

pub struct ClearFiltersCommand {
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    cleared_filters: Option<Vec<Box<dyn Any + Send + Sync>>>,
}

impl ClearFiltersCommand {
    pub fn new_video(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Video,
            cleared_filters: None,
        }
    }

    pub fn new_audio(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Audio,
            cleared_filters: None,
        }
    }

    pub fn new_subtitle(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Subtitle,
            cleared_filters: None,
        }
    }

    pub fn new_image(track_index: usize, segment_index: usize) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type: FilterType::Image,
            cleared_filters: None,
        }
    }
}

impl Command for ClearFiltersCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        let segment = track.get_segment(self.segment_index)?;
        self.cleared_filters = Some(match self.filter_type {
            FilterType::Video => segment
                .video_filters
                .iter()
                .map(|f| Box::new(f.clone()) as Box<dyn Any + Send + Sync>)
                .collect(),
            FilterType::Audio => segment
                .audio_filters
                .iter()
                .map(|f| Box::new(f.clone()) as Box<dyn Any + Send + Sync>)
                .collect(),
            FilterType::Subtitle => segment
                .subtitle_filters
                .iter()
                .map(|f| Box::new(f.clone()) as Box<dyn Any + Send + Sync>)
                .collect(),
            FilterType::Image => segment
                .image_filters
                .iter()
                .map(|f| Box::new(f.clone()) as Box<dyn Any + Send + Sync>)
                .collect(),
        });

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => segment.clear_video_filters(),
            FilterType::Audio => segment.clear_audio_filters(),
            FilterType::Subtitle => segment.clear_subtitle_filters(),
            FilterType::Image => segment.clear_image_filters(),
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Some(filters) = self.cleared_filters.take() {
            track.modify_segment(self.segment_index, |segment| {
                for (index, filter_box) in filters.into_iter().enumerate() {
                    match self.filter_type {
                        FilterType::Video => {
                            if let Some(wrapper) =
                                filter_box.downcast_ref::<Arc<VideoFilterWrapper>>()
                            {
                                _ = segment.insert_video_filter(index, wrapper.inner.clone_box());
                                _ = segment.set_video_filter_enabled(index, wrapper.enabled());
                            }
                        }
                        FilterType::Audio => {
                            if let Some(wrapper) =
                                filter_box.downcast_ref::<Arc<AudioFilterWrapper>>()
                            {
                                _ = segment.insert_audio_filter(index, wrapper.inner.clone_box());
                                _ = segment.set_audio_filter_enabled(index, wrapper.enabled());
                            }
                        }
                        FilterType::Subtitle => {
                            if let Some(wrapper) =
                                filter_box.downcast_ref::<Arc<SubtitleFilterWrapper>>()
                            {
                                _ = segment
                                    .insert_subtitle_filter(index, wrapper.inner.clone_box());
                                _ = segment.set_subtitle_filter_enabled(index, wrapper.enabled());
                            }
                        }
                        FilterType::Image => {
                            if let Some(wrapper) =
                                filter_box.downcast_ref::<Arc<ImageFilterWrapper>>()
                            {
                                _ = segment.insert_image_filter(index, wrapper.as_ref().clone());
                                _ = segment.set_image_filter_enabled(index, wrapper.enabled());
                            }
                        }
                    }
                }
            })?;
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Clear {:?} filters from segment {}",
            self.filter_type, self.segment_index
        )
    }
}

pub struct MoveFilterCommand {
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    from_index: usize,
    to_index: usize,
}

impl MoveFilterCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        filter_type: FilterType,
        from: usize,
        to: usize,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type,
            from_index: from,
            to_index: to,
        }
    }
}

impl Command for MoveFilterCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => _ = segment.move_video_filter(self.from_index, self.to_index),
            FilterType::Audio => _ = segment.move_audio_filter(self.from_index, self.to_index),
            FilterType::Image => _ = segment.move_image_filter(self.from_index, self.to_index),
            FilterType::Subtitle => {
                _ = segment.move_subtitle_filter(self.from_index, self.to_index)
            }
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => _ = segment.move_video_filter(self.to_index, self.from_index),
            FilterType::Audio => _ = segment.move_audio_filter(self.to_index, self.from_index),
            FilterType::Image => _ = segment.move_image_filter(self.to_index, self.from_index),
            FilterType::Subtitle => {
                _ = segment.move_subtitle_filter(self.to_index, self.from_index)
            }
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Move {:?} filter {} -> {} in segment {}",
            self.filter_type, self.from_index, self.to_index, self.segment_index
        )
    }
}

pub struct CopyFilterCommand {
    from_track_index: usize,
    from_segment_index: usize,
    from_filter_index: usize,
    to_track_index: usize,
    to_segment_index: usize,
    to_filter_index: usize,
    filter_type: FilterType,
    copied_filter: Option<Box<dyn Any + Send + Sync>>,
}

impl CopyFilterCommand {
    pub fn new(
        from_track_index: usize,
        from_segment_index: usize,
        from_filter_index: usize,
        to_track_index: usize,
        to_segment_index: usize,
        to_filter_index: usize,
        filter_type: FilterType,
    ) -> Self {
        Self {
            from_track_index,
            from_segment_index,
            from_filter_index,
            to_track_index,
            to_segment_index,
            to_filter_index,
            filter_type,
            copied_filter: None,
        }
    }
}

impl Command for CopyFilterCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let from_track = manager
            .get_mut(self.from_track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.from_track_index, manager_len))?;

        let from_segment = from_track.get_segment(self.from_segment_index)?;

        match self.filter_type {
            FilterType::Video => {
                let filter = from_segment
                    .video_filters
                    .get(self.from_filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(
                            self.from_filter_index,
                            from_segment.video_filters.len(),
                        )
                    })?;
                self.copied_filter = Some(Box::new(filter.clone()));
            }
            FilterType::Audio => {
                let filter = from_segment
                    .audio_filters
                    .get(self.from_filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(
                            self.from_filter_index,
                            from_segment.audio_filters.len(),
                        )
                    })?;
                self.copied_filter = Some(Box::new(filter.clone()));
            }
            FilterType::Subtitle => {
                let filter = from_segment
                    .subtitle_filters
                    .get(self.from_filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(
                            self.from_filter_index,
                            from_segment.subtitle_filters.len(),
                        )
                    })?;
                self.copied_filter = Some(Box::new(filter.clone()));
            }
            FilterType::Image => {
                let filter = from_segment
                    .image_filters
                    .get(self.from_filter_index)
                    .ok_or_else(|| {
                        Error::IndexOutOfBounds(
                            self.from_filter_index,
                            from_segment.image_filters.len(),
                        )
                    })?;
                self.copied_filter = Some(Box::new(filter.clone()));
            }
        }

        let to_track = manager
            .get_mut(self.to_track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.to_track_index, manager_len))?;

        if let Some(filter) = &self.copied_filter {
            to_track.modify_segment(self.to_segment_index, |segment| match self.filter_type {
                FilterType::Video => {
                    if let Some(wrapper) = filter.downcast_ref::<Arc<VideoFilterWrapper>>() {
                        _ = segment
                            .insert_video_filter(self.to_filter_index, wrapper.inner.clone_box());
                        _ = segment
                            .set_video_filter_enabled(self.to_filter_index, wrapper.enabled());
                    }
                }
                FilterType::Audio => {
                    if let Some(wrapper) = filter.downcast_ref::<Arc<AudioFilterWrapper>>() {
                        _ = segment
                            .insert_audio_filter(self.to_filter_index, wrapper.inner.clone_box());
                        _ = segment
                            .set_audio_filter_enabled(self.to_filter_index, wrapper.enabled());
                    }
                }
                FilterType::Subtitle => {
                    if let Some(wrapper) = filter.downcast_ref::<Arc<SubtitleFilterWrapper>>() {
                        _ = segment.insert_subtitle_filter(
                            self.to_filter_index,
                            wrapper.inner.clone_box(),
                        );
                        _ = segment
                            .set_subtitle_filter_enabled(self.to_filter_index, wrapper.enabled());
                    }
                }
                FilterType::Image => {
                    if let Some(wrapper) = filter.downcast_ref::<Arc<ImageFilterWrapper>>() {
                        _ = segment
                            .insert_image_filter(self.to_filter_index, wrapper.as_ref().clone());
                        _ = segment
                            .set_image_filter_enabled(self.to_filter_index, wrapper.enabled());
                    }
                }
            })?;
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.to_track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.to_track_index, manager_len))?;

        track.modify_segment(self.to_segment_index, |segment| match self.filter_type {
            FilterType::Video => _ = segment.remove_video_filter(self.to_filter_index),
            FilterType::Audio => _ = segment.remove_audio_filter(self.to_filter_index),
            FilterType::Subtitle => _ = segment.remove_subtitle_filter(self.to_filter_index),
            FilterType::Image => _ = segment.remove_image_filter(self.to_filter_index),
        })?;

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Copy {:?} filter from ({}, {}, {}) to ({}, {}, {})",
            self.filter_type,
            self.from_track_index,
            self.from_segment_index,
            self.from_filter_index,
            self.to_track_index,
            self.to_segment_index,
            self.to_filter_index
        )
    }
}

pub struct ToggleFilterCommand {
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    filter_index: usize,
    previous_enabled: Option<bool>,
}

impl ToggleFilterCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        filter_type: FilterType,
        filter_index: usize,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_type,
            filter_index,
            previous_enabled: None,
        }
    }
}

impl Command for ToggleFilterCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        // 获取当前状态
        let segment = track.get_segment(self.segment_index)?;
        let current_enabled = match self.filter_type {
            FilterType::Video => segment
                .video_filters
                .get(self.filter_index)
                .map(|w| w.enabled.load(Ordering::Relaxed)),
            FilterType::Audio => segment
                .audio_filters
                .get(self.filter_index)
                .map(|w| w.enabled.load(Ordering::Relaxed)),
            FilterType::Subtitle => segment
                .subtitle_filters
                .get(self.filter_index)
                .map(|w| w.enabled.load(Ordering::Relaxed)),
            FilterType::Image => segment
                .image_filters
                .get(self.filter_index)
                .map(|w| w.enabled()),
        };

        self.previous_enabled = current_enabled;

        // 切换状态
        track.modify_segment(self.segment_index, |segment| match self.filter_type {
            FilterType::Video => _ = segment.toggle_video_filter(self.filter_index),
            FilterType::Audio => _ = segment.toggle_audio_filter(self.filter_index),
            FilterType::Subtitle => _ = segment.toggle_subtitle_filter(self.filter_index),
            FilterType::Image => _ = segment.toggle_image_filter(self.filter_index),
        })?;

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        if let Some(previous_enabled) = self.previous_enabled {
            track.modify_segment(self.segment_index, |segment| match self.filter_type {
                FilterType::Video => {
                    _ = segment.set_video_filter_enabled(self.filter_index, previous_enabled)
                }
                FilterType::Audio => {
                    _ = segment.set_audio_filter_enabled(self.filter_index, previous_enabled);
                }
                FilterType::Subtitle => {
                    _ = segment.set_subtitle_filter_enabled(self.filter_index, previous_enabled);
                }
                FilterType::Image => {
                    _ = segment.set_image_filter_enabled(self.filter_index, previous_enabled);
                }
            })?;
        }

        Ok(())
    }

    fn describe(&self) -> String {
        format!(
            "Toggle {:?} filter {} in segment {}",
            self.filter_type, self.filter_index, self.segment_index
        )
    }
}

pub struct AddKeyframeCommand {
    track_index: usize,
    segment_index: usize,
    filter_index: usize,
    filter_type: FilterType,
    property_name: String,
    keyframe: Keyframe,
}

impl AddKeyframeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        property_name: String,
        time_ms: i64,
        value: KeyframeValue,
        filter_type: FilterType,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type,
            property_name,
            keyframe: Keyframe::new(time_ms, value),
        }
    }

    pub fn new_float(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        property_name: String,
        time_ms: i64,
        value: f32,
        filter_type: FilterType,
    ) -> Self {
        Self::new(
            track_index,
            segment_index,
            filter_index,
            property_name,
            time_ms,
            KeyframeValue::Float(value),
            filter_type,
        )
    }
}

impl Command for AddKeyframeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match self.filter_type {
            FilterType::Video => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.video_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        let mut tracks = tracks;
                        tracks.add_keyframe(&self.property_name, self.keyframe.clone());

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.video_filters[self.filter_index] =
                            Arc::new(VideoFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Image => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.image_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        let mut tracks = tracks;
                        tracks.add_keyframe(&self.property_name, self.keyframe.clone());

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.image_filters[self.filter_index] =
                            Arc::new(ImageFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Audio => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.audio_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        let mut tracks = tracks;
                        tracks.add_keyframe(&self.property_name, self.keyframe.clone());

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.audio_filters[self.filter_index] =
                            Arc::new(AudioFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Subtitle => {
                return Err(Error::TrackSegment(
                    "Subtitle filters don't support keyframes".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let remove_cmd = RemoveKeyframeCommand::new(
            self.track_index,
            self.segment_index,
            self.filter_index,
            self.property_name.clone(),
            self.keyframe.clone(),
            self.filter_type,
        );
        let mut remove_cmd = remove_cmd;
        remove_cmd.execute(manager)
    }

    fn describe(&self) -> String {
        format!(
            "Add keyframe at {}ms to property '{}' in {} filter {}",
            self.keyframe.time_ms,
            self.property_name,
            self.filter_type.to_string(),
            self.filter_index
        )
    }
}

pub struct RemoveKeyframeCommand {
    track_index: usize,
    segment_index: usize,
    filter_index: usize,
    filter_type: FilterType,
    property_name: String,
    time_ms: i64,
    removed_keyframe: Option<Keyframe>,
}

impl RemoveKeyframeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        property_name: String,
        keyframe: Keyframe,
        filter_type: FilterType,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type,
            property_name,
            time_ms: keyframe.time_ms,
            removed_keyframe: Some(keyframe),
        }
    }
}

impl Command for RemoveKeyframeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        if self.removed_keyframe.is_none()
            && let Some(track) = manager.get(self.track_index)
            && let Ok(segment) = track.get_segment(self.segment_index)
        {
            match self.filter_type {
                FilterType::Video => {
                    if let Some(wrapper) = segment.video_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        if let Some(prop_track) = tracks.get_track(&self.property_name) {
                            self.removed_keyframe = prop_track
                                .keyframes
                                .iter()
                                .find(|kf| kf.time_ms == self.time_ms)
                                .cloned();
                        }
                    }
                }
                FilterType::Image => {
                    if let Some(wrapper) = segment.image_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        if let Some(prop_track) = tracks.get_track(&self.property_name) {
                            self.removed_keyframe = prop_track
                                .keyframes
                                .iter()
                                .find(|kf| kf.time_ms == self.time_ms)
                                .cloned();
                        }
                    }
                }
                FilterType::Audio => {
                    if let Some(wrapper) = segment.audio_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        if let Some(prop_track) = tracks.get_track(&self.property_name) {
                            self.removed_keyframe = prop_track
                                .keyframes
                                .iter()
                                .find(|kf| kf.time_ms == self.time_ms)
                                .cloned();
                        }
                    }
                }
                FilterType::Subtitle => {}
            }
        }

        if self.removed_keyframe.is_none() {
            return Err(Error::TrackSegment(format!(
                "Keyframe at {}ms not found for property '{}'",
                self.time_ms, self.property_name
            )));
        }

        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match self.filter_type {
            FilterType::Video => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.video_filters.get(self.filter_index) {
                        let mut tracks = wrapper.inner.get_keyframe_tracks();
                        tracks.remove_keyframe(&self.property_name, self.time_ms);
                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.video_filters[self.filter_index] =
                            Arc::new(VideoFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Image => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.image_filters.get(self.filter_index) {
                        let mut tracks = wrapper.inner.get_keyframe_tracks();
                        tracks.remove_keyframe(&self.property_name, self.time_ms);
                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.image_filters[self.filter_index] =
                            Arc::new(ImageFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Audio => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.audio_filters.get(self.filter_index) {
                        let mut tracks = wrapper.inner.get_keyframe_tracks();
                        tracks.remove_keyframe(&self.property_name, self.time_ms);
                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.audio_filters[self.filter_index] =
                            Arc::new(AudioFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Subtitle => {
                return Err(Error::TrackSegment(
                    "Subtitle filters don't support keyframes".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(keyframe) = self.removed_keyframe.clone() {
            let add_cmd = AddKeyframeCommand::new(
                self.track_index,
                self.segment_index,
                self.filter_index,
                self.property_name.clone(),
                keyframe.time_ms,
                keyframe.value,
                self.filter_type,
            );
            let mut add_cmd = add_cmd;
            add_cmd.execute(manager)
        } else {
            Err(Error::TrackSegment(
                "No keyframe stored for undo".to_string(),
            ))
        }
    }

    fn describe(&self) -> String {
        format!(
            "Remove keyframe at {}ms from property '{}' in {} filter {}",
            self.time_ms,
            self.property_name,
            self.filter_type.to_string(),
            self.filter_index
        )
    }
}

pub struct UpdateKeyframeValueCommand {
    track_index: usize,
    segment_index: usize,
    filter_index: usize,
    filter_type: FilterType,
    property_name: String,
    time_ms: i64,
    new_value: KeyframeValue,
    old_value: Option<KeyframeValue>,
}

impl UpdateKeyframeValueCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        property_name: String,
        time_ms: i64,
        new_value: KeyframeValue,
        filter_type: FilterType,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type,
            property_name,
            time_ms,
            new_value,
            old_value: None,
        }
    }

    pub fn new_float(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        property_name: String,
        time_ms: i64,
        value: f32,
        filter_type: FilterType,
    ) -> Self {
        Self::new(
            track_index,
            segment_index,
            filter_index,
            property_name,
            time_ms,
            KeyframeValue::Float(value),
            filter_type,
        )
    }
}

impl Command for UpdateKeyframeValueCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match self.filter_type {
            FilterType::Video => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.video_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();

                        // Store old value for undo
                        if let Some(prop_track) = tracks.get_track(&self.property_name) {
                            self.old_value = prop_track
                                .keyframes
                                .iter()
                                .find(|kf| kf.time_ms == self.time_ms)
                                .map(|kf| kf.value.clone());
                        }

                        let mut tracks = tracks;
                        tracks.update_keyframe_value(
                            &self.property_name,
                            self.time_ms,
                            self.new_value.clone(),
                        );

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.video_filters[self.filter_index] =
                            Arc::new(VideoFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Image => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.image_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();

                        // Store old value for undo
                        if let Some(prop_track) = tracks.get_track(&self.property_name) {
                            self.old_value = prop_track
                                .keyframes
                                .iter()
                                .find(|kf| kf.time_ms == self.time_ms)
                                .map(|kf| kf.value.clone());
                        }

                        let mut tracks = tracks;
                        tracks.update_keyframe_value(
                            &self.property_name,
                            self.time_ms,
                            self.new_value.clone(),
                        );

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.image_filters[self.filter_index] =
                            Arc::new(ImageFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Audio => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.audio_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();

                        // Store old value for undo
                        if let Some(prop_track) = tracks.get_track(&self.property_name) {
                            self.old_value = prop_track
                                .keyframes
                                .iter()
                                .find(|kf| kf.time_ms == self.time_ms)
                                .map(|kf| kf.value.clone());
                        }

                        let mut tracks = tracks;
                        tracks.update_keyframe_value(
                            &self.property_name,
                            self.time_ms,
                            self.new_value.clone(),
                        );

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.audio_filters[self.filter_index] =
                            Arc::new(AudioFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Subtitle => {
                return Err(Error::TrackSegment(
                    "Subtitle filters don't support keyframes".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        if let Some(old_value) = self.old_value.clone() {
            let undo_cmd = UpdateKeyframeValueCommand::new(
                self.track_index,
                self.segment_index,
                self.filter_index,
                self.property_name.clone(),
                self.time_ms,
                old_value,
                self.filter_type,
            );
            let mut undo_cmd = undo_cmd;
            undo_cmd.execute(manager)
        } else {
            Ok(())
        }
    }

    fn describe(&self) -> String {
        format!(
            "Update keyframe value at {}ms in property '{}' of {} filter {}",
            self.time_ms,
            self.property_name,
            self.filter_type.to_string(),
            self.filter_index
        )
    }
}

pub struct MoveKeyframeCommand {
    track_index: usize,
    segment_index: usize,
    filter_index: usize,
    filter_type: FilterType,
    property_name: String,
    old_time_ms: i64,
    new_time_ms: i64,
}

impl MoveKeyframeCommand {
    pub fn new(
        track_index: usize,
        segment_index: usize,
        filter_index: usize,
        property_name: String,
        old_time_ms: i64,
        new_time_ms: i64,
        filter_type: FilterType,
    ) -> Self {
        Self {
            track_index,
            segment_index,
            filter_index,
            filter_type,
            property_name,
            old_time_ms,
            new_time_ms,
        }
    }
}

impl Command for MoveKeyframeCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        let manager_len = manager.len();
        let track = manager
            .get_mut(self.track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(self.track_index, manager_len))?;

        match self.filter_type {
            FilterType::Video => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.video_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        let mut tracks = tracks;

                        tracks.move_keyframe(
                            &self.property_name,
                            self.old_time_ms,
                            self.new_time_ms,
                        );

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.video_filters[self.filter_index] =
                            Arc::new(VideoFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Image => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.image_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        let mut tracks = tracks;

                        tracks.move_keyframe(
                            &self.property_name,
                            self.old_time_ms,
                            self.new_time_ms,
                        );

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.image_filters[self.filter_index] =
                            Arc::new(ImageFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Audio => {
                track.modify_segment(self.segment_index, |segment| {
                    if let Some(wrapper) = segment.audio_filters.get(self.filter_index) {
                        let tracks = wrapper.inner.get_keyframe_tracks();
                        let mut tracks = tracks;

                        tracks.move_keyframe(
                            &self.property_name,
                            self.old_time_ms,
                            self.new_time_ms,
                        );

                        let mut new_filter = wrapper.inner.clone_box();
                        new_filter.set_keyframe_tracks(tracks);
                        segment.audio_filters[self.filter_index] =
                            Arc::new(AudioFilterWrapper::new(wrapper.enabled(), new_filter));
                    }
                })?;
            }
            FilterType::Subtitle => {
                return Err(Error::TrackSegment(
                    "Subtitle filters don't support keyframes".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        let undo_cmd = MoveKeyframeCommand::new(
            self.track_index,
            self.segment_index,
            self.filter_index,
            self.property_name.clone(),
            self.new_time_ms,
            self.old_time_ms,
            self.filter_type,
        );
        let mut undo_cmd = undo_cmd;
        undo_cmd.execute(manager)
    }

    fn describe(&self) -> String {
        format!(
            "Move keyframe from {}ms to {}ms in property '{}' of {} filter {}",
            self.old_time_ms,
            self.new_time_ms,
            self.property_name,
            self.filter_type.to_string(),
            self.filter_index
        )
    }
}
