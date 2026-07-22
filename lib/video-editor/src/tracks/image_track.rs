use crate::{
    metadata::Metadata,
    tracks::{segment::Segment, track::InnerTrack},
};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct ImageTrack {
    pub name: String,
    pub hiding: bool,
    pub locked: bool,
    pub track: InnerTrack,
}

impl ImageTrack {
    pub fn new(metadata: Arc<Metadata>, duration: Duration, segments: Vec<Arc<Segment>>) -> Self {
        Self::new_with_inner(InnerTrack::new(metadata, duration, segments))
    }

    pub fn new_with_inner(track: InnerTrack) -> Self {
        Self {
            name: "I".to_string(),
            hiding: false,
            locked: false,
            track,
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_hiding(&mut self, hiding: bool) {
        self.hiding = hiding;
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub(crate) fn update_duration(&mut self) {
        self.track.duration = self
            .track
            .segments
            .last()
            .map(|seg| seg.timeline_offset + seg.duration)
            .unwrap_or(Duration::ZERO);
    }
}