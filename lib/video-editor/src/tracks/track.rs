use super::{
    audio_track::AudioTrack, image_track::ImageTrack, segment::Segment,
    subtitle_track::SubtitleTrack, text_track::TextTrack, video_track::VideoTrack,
};
use crate::{
    Error, Result, ensure_file_exists,
    metadata::{Metadata, MetadataType},
    tracks::subtitle_track::{extract_lrc_as_segments, extract_subtitles},
};
use std::{path::Path, sync::Arc, time::Duration};

// 轨道类型的显示优先级（数值越小优先级越高，位置越靠上）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrackPriority(u8);

impl From<MetadataType> for TrackPriority {
    fn from(mt: MetadataType) -> Self {
        match mt {
            MetadataType::Subtitle => TrackPriority::SUBTITLE,
            MetadataType::Image => TrackPriority::IMAGE,
            MetadataType::Video => TrackPriority::VIDEO,
            MetadataType::Audio => TrackPriority::AUDIO,
            MetadataType::None => TrackPriority(4),
        }
    }
}

impl TrackPriority {
    pub const SUBTITLE: TrackPriority = TrackPriority(0); // highest (topmost)
    pub const TEXT: TrackPriority = TrackPriority(1);
    pub const IMAGE: TrackPriority = TrackPriority(2);
    pub const VIDEO: TrackPriority = TrackPriority(2);
    pub const AUDIO: TrackPriority = TrackPriority(3); // lowest (bottommost)
}

#[derive(Debug, Clone)]
pub enum Track {
    Video(Arc<VideoTrack>),
    Audio(Arc<AudioTrack>),
    Subtitle(Arc<SubtitleTrack>),
    Image(Arc<ImageTrack>),
    Text(Arc<TextTrack>),
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct InnerTrack {
    pub metadata: Arc<Metadata>,
    pub duration: Duration,
    pub segments: Vec<Arc<Segment>>,
}

impl InnerTrack {
    pub fn new(metadata: Arc<Metadata>, duration: Duration, segments: Vec<Arc<Segment>>) -> Self {
        let duration = segments
            .iter()
            .map(|seg| seg.timeline_offset + seg.duration)
            .max()
            .unwrap_or_default()
            .max(duration);

        Self {
            metadata,
            duration,
            segments,
        }
    }
}

impl Track {
    pub fn new<P: AsRef<Path>>(path: P, global_speed: f32) -> Result<Vec<Track>> {
        let path = path.as_ref();
        ensure_file_exists!(path);

        let metadata = Arc::new(crate::metadata::get_metadata(path)?);
        let duration = metadata.duration;

        match metadata.get_type() {
            MetadataType::Video => {
                let video_segment = Arc::new(Segment::new(
                    Duration::ZERO,
                    duration,
                    metadata.clone(),
                    global_speed,
                ));

                Ok(vec![Track::Video(Arc::new(VideoTrack {
                    name: "V".to_string(),
                    hiding: false,
                    muted: false,
                    locked: false,
                    track: InnerTrack::new(metadata.clone(), duration, vec![video_segment]),
                }))])
            }
            MetadataType::Image => {
                let image_duration = if duration.is_zero() {
                    Duration::from_secs(5)
                } else {
                    duration
                };
                let image_segment = Arc::new(Segment::new(
                    Duration::ZERO,
                    image_duration,
                    metadata.clone(),
                    global_speed,
                ));

                Ok(vec![Track::Image(Arc::new(ImageTrack {
                    name: "I".to_string(),
                    hiding: false,
                    locked: false,
                    track: InnerTrack::new(metadata.clone(), image_duration, vec![image_segment]),
                }))])
            }
            MetadataType::Audio => {
                let audio_segment = Arc::new(Segment::new(
                    Duration::ZERO,
                    duration,
                    metadata.clone(),
                    global_speed,
                ));

                Ok(vec![Track::Audio(Arc::new(AudioTrack {
                    name: "A".to_string(),
                    hiding: false,
                    locked: false,
                    track: InnerTrack::new(metadata.clone(), duration, vec![audio_segment]),
                }))])
            }
            MetadataType::Subtitle => {
                let mut tracks = vec![];

                for subtitle_meta in metadata.subtitles.iter() {
                    if let Ok(subtitle_tracks) = Self::create_subtitle_tracks(
                        path,
                        metadata.clone(),
                        subtitle_meta.index,
                        duration,
                        global_speed,
                    ) {
                        tracks.extend(subtitle_tracks);
                    }
                }

                if tracks.is_empty() {
                    Err(Error::InvalidConfig(
                        "No subtitle tracks created".to_string(),
                    ))
                } else {
                    Ok(tracks)
                }
            }
            MetadataType::None => {
                return Err(Error::InvalidConfig("metadata type is `None`".to_string()));
            }
        }
    }

    pub fn set_global_speed(&mut self, global_speed: f32) {
        match self {
            Track::Video(track) => {
                let video_track = Arc::make_mut(track);
                for seg in video_track.track.segments.iter_mut() {
                    let seg = Arc::make_mut(seg);
                    seg.duration = Duration::from_secs_f64(
                        seg.original_duration.as_secs_f64()
                            / (seg.playback_speed * global_speed) as f64,
                    );
                    seg.global_speed = global_speed;
                }
                video_track.update_duration();
            }
            Track::Audio(track) => {
                let audio_track = Arc::make_mut(track);
                for seg in audio_track.track.segments.iter_mut() {
                    let seg = Arc::make_mut(seg);
                    seg.duration = Duration::from_secs_f64(
                        seg.original_duration.as_secs_f64()
                            / (seg.playback_speed * global_speed) as f64,
                    );
                    seg.global_speed = global_speed;
                }
                audio_track.update_duration();
            }
            Track::Subtitle(track) => {
                let subtitle_track = Arc::make_mut(track);
                for seg in subtitle_track.track.segments.iter_mut() {
                    let seg = Arc::make_mut(seg);
                    seg.duration = Duration::from_secs_f64(
                        seg.original_duration.as_secs_f64()
                            / (seg.playback_speed * global_speed) as f64,
                    );
                    seg.global_speed = global_speed;
                }
                subtitle_track.update_duration();
            }
            Track::Image(track) => {
                let image_track = Arc::make_mut(track);
                for seg in image_track.track.segments.iter_mut() {
                    let seg = Arc::make_mut(seg);
                    seg.duration = Duration::from_secs_f64(
                        seg.original_duration.as_secs_f64()
                            / (seg.playback_speed * global_speed) as f64,
                    );
                    seg.global_speed = global_speed;
                }
                image_track.update_duration();
            }
            Track::Text(track) => {
                let text_track = Arc::make_mut(track);
                for seg in text_track.track.segments.iter_mut() {
                    let seg = Arc::make_mut(seg);
                    seg.duration = Duration::from_secs_f64(
                        seg.original_duration.as_secs_f64()
                            / (seg.playback_speed * global_speed) as f64,
                    );
                    seg.global_speed = global_speed;
                }
                text_track.update_duration();
            }
        }
    }

    pub fn with_global_speed(mut self, global_speed: f32) -> Self {
        self.set_global_speed(global_speed);
        self
    }

    pub fn add_segment(&mut self, mut segment: Arc<Segment>) {
        let last_end = self
            .segments()
            .last()
            .map(|seg| seg.timeline_offset + seg.duration)
            .unwrap_or(Duration::ZERO);

        let seg = Arc::make_mut(&mut segment);
        seg.timeline_offset = last_end;
        self.push_segment(segment);
    }

    // 会获取当前节点的timeline_offset赋值给插入的segment
    pub fn insert_segment(
        &mut self,
        index: usize,
        mut segment: Arc<Segment>,
        shift_timeline: bool,
    ) -> Result<()> {
        if index > self.segments_count() {
            return Err(Error::IndexOutOfBounds(index, self.segments_count()));
        }

        let insert_offset = if index < self.segments_count() {
            self.get_segment(index)?.timeline_offset
        } else {
            self.segments()
                .last()
                .map(|seg| seg.timeline_offset + seg.duration)
                .unwrap_or(Duration::ZERO)
        };

        let seg = Arc::make_mut(&mut segment);
        seg.timeline_offset = insert_offset;

        if shift_timeline {
            let shift_amount = seg.duration;
            for i in index..self.segments_count() {
                self.shift_segment_timeline(i, shift_amount)?;
            }
        }

        self.insert_segment_at_index(index, segment);
        Ok(())
    }

    pub fn remove_segment(&mut self, index: usize, shift_timeline: bool) -> Result<Arc<Segment>> {
        if index >= self.segments_count() {
            return Err(Error::IndexOutOfBounds(index, self.segments_count()));
        }

        let segment = self.remove_segment_at_index(index);

        if shift_timeline {
            let shift_amount = segment.duration;
            for i in index..self.segments_count() {
                self.shift_segment_timeline_backward(i, shift_amount)?;
            }
        }

        Ok(segment)
    }

    fn push_segment(&mut self, segment: Arc<Segment>) {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.push(segment);
                track.update_duration();
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.push(segment);
                track.update_duration();
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.push(segment);
                track.update_duration();
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.push(segment);
                track.update_duration();
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.push(segment);
                track.update_duration();
            }
        }
    }

    fn insert_segment_at_index(&mut self, index: usize, segment: Arc<Segment>) {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.insert(index, segment);
                track.update_duration();
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.insert(index, segment);
                track.update_duration();
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.insert(index, segment);
                track.update_duration();
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.insert(index, segment);
                track.update_duration();
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.insert(index, segment);
                track.update_duration();
            }
        }
    }

    fn remove_segment_at_index(&mut self, index: usize) -> Arc<Segment> {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                let segment = track.track.segments.remove(index);
                track.update_duration();
                segment
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                let segment = track.track.segments.remove(index);
                track.update_duration();
                segment
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                let segment = track.track.segments.remove(index);
                track.update_duration();
                segment
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                let segment = track.track.segments.remove(index);
                track.update_duration();
                segment
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                let segment = track.track.segments.remove(index);
                track.update_duration();
                segment
            }
        }
    }

    pub fn shift_segment_timeline(&mut self, index: usize, amount: Duration) -> Result<()> {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_add(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset overflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_add(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset overflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_add(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset overflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_add(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset overflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_add(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset overflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
        }
        Ok(())
    }

    pub fn shift_segment_timeline_backward(
        &mut self,
        index: usize,
        amount: Duration,
    ) -> Result<()> {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_sub(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset underflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_sub(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset underflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_sub(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset underflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_sub(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset underflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                let segment = Arc::make_mut(&mut track.track.segments[index]);
                segment.timeline_offset =
                    segment.timeline_offset.checked_sub(amount).ok_or_else(|| {
                        Error::InvalidConfig("Timeline offset underflow after shifting".to_string())
                    })?;
                track.update_duration();
            }
        }
        Ok(())
    }

    pub fn get_segment(&self, index: usize) -> Result<&Arc<Segment>> {
        match self {
            Track::Video(track) => track
                .track
                .segments
                .get(index)
                .ok_or_else(|| Error::IndexOutOfBounds(index, track.track.segments.len())),
            Track::Audio(track) => track
                .track
                .segments
                .get(index)
                .ok_or_else(|| Error::IndexOutOfBounds(index, track.track.segments.len())),
            Track::Subtitle(track) => track
                .track
                .segments
                .get(index)
                .ok_or_else(|| Error::IndexOutOfBounds(index, track.track.segments.len())),
            Track::Image(track) => track
                .track
                .segments
                .get(index)
                .ok_or_else(|| Error::IndexOutOfBounds(index, track.track.segments.len())),
            Track::Text(track) => track
                .track
                .segments
                .get(index)
                .ok_or_else(|| Error::IndexOutOfBounds(index, track.track.segments.len())),
        }
    }

    pub fn insert_segment_shift(&mut self, index: usize, segment: Arc<Segment>) -> Result<()> {
        self.insert_segment(index, segment, true)
    }

    pub fn insert_segment_image(&mut self, index: usize, segment: Arc<Segment>) -> Result<()> {
        self.insert_segment(index, segment, false)
    }

    // Insert segment at index preserving original timeline_offset (for undo operations)
    pub fn insert_segment_preserve(&mut self, index: usize, segment: Arc<Segment>) -> Result<()> {
        if index > self.segments_count() {
            return Err(Error::IndexOutOfBounds(index, self.segments_count()));
        }
        self.insert_segment_at_index(index, segment);
        Ok(())
    }

    pub fn remove_segment_shift(&mut self, index: usize) -> Result<Arc<Segment>> {
        self.remove_segment(index, true)
    }

    pub fn remove_segment_leave_gap(&mut self, index: usize) -> Result<Arc<Segment>> {
        self.remove_segment(index, false)
    }

    pub fn sort_segments_by_timeline_offset(&mut self) {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                track.update_duration();
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                track.update_duration();
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                track.update_duration();
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                track.update_duration();
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                track.track.segments.sort_by_key(|seg| seg.timeline_offset);
                track.update_duration();
            }
        }
    }

    #[inline]
    pub fn segments_count(&self) -> usize {
        match self {
            Track::Video(track) => track.track.segments.len(),
            Track::Audio(track) => track.track.segments.len(),
            Track::Subtitle(track) => track.track.segments.len(),
            Track::Image(track) => track.track.segments.len(),
            Track::Text(track) => track.track.segments.len(),
        }
    }

    pub fn modify_segment<F>(&mut self, index: usize, f: F) -> Result<()>
    where
        F: FnOnce(&mut Segment),
    {
        match self {
            Track::Video(track) => {
                let track = Arc::make_mut(track);
                let len = track.track.segments.len();
                let segment = track
                    .track
                    .segments
                    .get_mut(index)
                    .ok_or_else(|| Error::IndexOutOfBounds(index, len))?;
                let segment = Arc::make_mut(segment);
                f(segment);
                track.update_duration();
            }
            Track::Audio(track) => {
                let track = Arc::make_mut(track);
                let len = track.track.segments.len();
                let segment = track
                    .track
                    .segments
                    .get_mut(index)
                    .ok_or_else(|| Error::IndexOutOfBounds(index, len))?;
                let segment = Arc::make_mut(segment);
                f(segment);
                track.update_duration();
            }
            Track::Subtitle(track) => {
                let track = Arc::make_mut(track);
                let len = track.track.segments.len();
                let segment = track
                    .track
                    .segments
                    .get_mut(index)
                    .ok_or_else(|| Error::IndexOutOfBounds(index, len))?;
                let segment = Arc::make_mut(segment);
                f(segment);
                track.update_duration();
            }
            Track::Image(track) => {
                let track = Arc::make_mut(track);
                let len = track.track.segments.len();
                let segment = track
                    .track
                    .segments
                    .get_mut(index)
                    .ok_or_else(|| Error::IndexOutOfBounds(index, len))?;
                let segment = Arc::make_mut(segment);
                f(segment);
                track.update_duration();
            }
            Track::Text(track) => {
                let track = Arc::make_mut(track);
                let len = track.track.segments.len();
                let segment = track
                    .track
                    .segments
                    .get_mut(index)
                    .ok_or_else(|| Error::IndexOutOfBounds(index, len))?;
                let segment = Arc::make_mut(segment);
                f(segment);
                track.update_duration();
            }
        }
        Ok(())
    }

    pub fn split_segment(
        &mut self,
        segment_index: usize,
        split_offset: Duration,
    ) -> Result<(usize, usize)> {
        if split_offset == Duration::ZERO {
            return Err(Error::InvalidConfig(
                "Split offset cannot be zero".to_string(),
            ));
        }

        if matches!(self, Track::Text(_)) {
            return Err(Error::InvalidConfig(
                "Text segments do not support splitting".to_string(),
            ));
        }

        let segment = match self {
            Track::Video(track) => Arc::make_mut(track).track.segments.as_mut(),
            Track::Audio(track) => Arc::make_mut(track).track.segments.as_mut(),
            Track::Subtitle(track) => Arc::make_mut(track).track.segments.as_mut(),
            Track::Image(track) => Arc::make_mut(track).track.segments.as_mut(),
            Track::Text(track) => Arc::make_mut(track).track.segments.as_mut(),
        };

        Self::split_segment_in_vec(segment, segment_index, split_offset)
    }

    fn create_split_segment(
        original: &Segment,
        timeline_offset: Duration,
        source_offset: Duration,
        timeline_duration: Duration,
    ) -> Segment {
        // 对于 image/subtitle 类型，source_offset 应该保持为 0，因为它们不受源文件时间限制
        let effective_source_offset = if original.metadata.is_time_independent() {
            Duration::ZERO
        } else {
            source_offset
        };

        // 计算 original_duration（源内容时长）= timeline_duration * (playback_speed * global_speed)
        let original_duration = Duration::from_secs_f64(
            timeline_duration.as_secs_f64()
                * (original.playback_speed * original.global_speed) as f64,
        );

        let mut seg = Segment::new_with_source_offset(
            timeline_offset,
            effective_source_offset,
            original_duration,
            original.playback_speed,
            original.global_speed,
            original.metadata.clone(),
        );

        seg.video_filters = original.video_filters.clone();
        seg.audio_filters = original.audio_filters.clone();
        seg.subtitle_filters = original.subtitle_filters.clone();
        seg.image_filters = original.image_filters.clone();
        seg.hiding = original.hiding;
        seg.audio_muted = original.audio_muted;
        seg.subtitle_text = original.subtitle_text.clone();
        seg
    }

    fn split_segment_in_vec(
        segments: &mut Vec<Arc<Segment>>,
        segment_index: usize,
        split_offset: Duration,
    ) -> Result<(usize, usize)> {
        let segment = segments
            .get(segment_index)
            .ok_or_else(|| Error::IndexOutOfBounds(segment_index, segments.len()))?;

        let segment = segment.as_ref();
        if split_offset >= segment.duration {
            return Err(Error::InvalidConfig(format!(
                "Split offset {:?} must be less than segment duration {:?}",
                split_offset, segment.duration
            )));
        }

        if split_offset.is_zero() {
            return Err(Error::InvalidConfig(
                "Split offset cannot be zero, would create zero-duration segment".into(),
            ));
        }

        let left_duration = split_offset;
        let right_duration = segment.duration - split_offset;

        if right_duration.is_zero() {
            return Err(Error::InvalidConfig(
                "Split would create zero-duration right segment".into(),
            ));
        }

        let right_source_offset = Duration::from_secs_f64(
            segment.source_offset.as_secs_f64()
                + split_offset.as_secs_f64()
                    * (segment.playback_speed * segment.global_speed) as f64,
        );

        let left_seg = Self::create_split_segment(
            segment,
            segment.timeline_offset,
            segment.source_offset,
            left_duration,
        );

        let right_seg = Self::create_split_segment(
            segment,
            segment.timeline_offset + split_offset,
            right_source_offset,
            right_duration,
        );

        segments.remove(segment_index);
        segments.insert(segment_index, Arc::new(left_seg));
        segments.insert(segment_index + 1, Arc::new(right_seg));

        Ok((segment_index, segment_index + 1))
    }

    pub fn move_segment(
        &mut self,
        segment_index: usize,
        new_timeline_offset: Duration,
    ) -> Result<()> {
        self.modify_segment(segment_index, |segment| {
            segment.timeline_offset = new_timeline_offset;
        })
    }

    #[inline]
    pub fn duration(&self) -> Duration {
        match self {
            Track::Video(track) => track.track.duration,
            Track::Audio(track) => track.track.duration,
            Track::Subtitle(track) => track.track.duration,
            Track::Image(track) => track.track.duration,
            Track::Text(track) => track.track.duration,
        }
    }

    #[inline]
    pub fn metadata(&self) -> &Arc<Metadata> {
        match self {
            Track::Video(track) => &track.track.metadata,
            Track::Audio(track) => &track.track.metadata,
            Track::Subtitle(track) => &track.track.metadata,
            Track::Image(track) => &track.track.metadata,
            Track::Text(track) => &track.track.metadata,
        }
    }

    #[inline]
    pub fn segments(&self) -> &[Arc<Segment>] {
        match self {
            Track::Video(track) => &track.track.segments,
            Track::Audio(track) => &track.track.segments,
            Track::Subtitle(track) => &track.track.segments,
            Track::Image(track) => &track.track.segments,
            Track::Text(track) => &track.track.segments,
        }
    }

    pub fn trim_gap(&mut self, segment_index: usize, shift_timeline: bool) -> Result<()> {
        self.trim_start_gap(segment_index, shift_timeline)?;
        self.trim_end_gap(segment_index, shift_timeline)
    }

    pub fn trim_start_gap(&mut self, segment_index: usize, shift_timeline: bool) -> Result<()> {
        if segment_index >= self.segments_count() {
            return Err(Error::IndexOutOfBounds(
                segment_index,
                self.segments_count(),
            ));
        }

        if segment_index == 0 {
            let segments = self.segments();
            let first_segment = &segments[0];
            let gap_to_remove = first_segment.timeline_offset;

            if gap_to_remove.is_zero() {
                return Ok(());
            }

            if shift_timeline {
                for i in 0..self.segments_count() {
                    self.shift_segment_timeline_backward(i, gap_to_remove)?;
                }
            } else {
                self.shift_segment_timeline_backward(0, gap_to_remove)?;
            }
        } else {
            // Remove gap between segment_index-1 and segment_index
            let segments = self.segments();
            let prev_segment = &segments[segment_index - 1];
            let target_segment = &segments[segment_index];

            let prev_end = prev_segment.timeline_offset + prev_segment.duration;
            let gap = target_segment.timeline_offset.saturating_sub(prev_end);

            if gap.is_zero() {
                return Ok(());
            }

            if shift_timeline {
                for i in segment_index..self.segments_count() {
                    self.shift_segment_timeline_backward(i, gap)?;
                }
            } else {
                self.shift_segment_timeline_backward(segment_index, gap)?;
            }
        }

        Self::update_track_duration(self);
        Ok(())
    }

    pub fn trim_end_gap(&mut self, segment_index: usize, shift_timeline: bool) -> Result<()> {
        if segment_index >= self.segments_count() {
            return Err(Error::IndexOutOfBounds(
                segment_index,
                self.segments_count(),
            ));
        }

        let segments = self.segments();
        let target_segment = &segments[segment_index];

        if segment_index == self.segments_count() - 1 {
            let segment_end = target_segment.timeline_offset + target_segment.duration;
            let current_duration = self.duration();

            if segment_end >= current_duration {
                return Ok(());
            }
        } else {
            // Remove gap between segment_index and segment_index+1
            let next_segment = &segments[segment_index + 1];
            let segment_end = target_segment.timeline_offset + target_segment.duration;
            let gap = next_segment.timeline_offset.saturating_sub(segment_end);

            if gap.is_zero() {
                return Ok(());
            }

            if shift_timeline {
                for i in (segment_index + 1)..self.segments_count() {
                    self.shift_segment_timeline_backward(i, gap)?;
                }
            } else {
                self.shift_segment_timeline_backward(segment_index + 1, gap)?;
            }
        }

        Self::update_track_duration(self);
        Ok(())
    }

    pub fn remove_all_gaps(&mut self) -> Result<()> {
        let segments = self.segments().to_vec();

        if segments.is_empty() {
            return Ok(());
        }

        let mut cumulative_duration = Duration::ZERO;

        for (i, segment) in segments.iter().enumerate() {
            self.modify_segment(i, |seg| {
                seg.timeline_offset = cumulative_duration;
            })?;

            cumulative_duration += segment.duration;
        }

        Self::update_track_duration(self);
        Ok(())
    }

    pub fn shrink_segment_left(
        &mut self,
        segment_index: usize,
        timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Result<()> {
        let segment = self.get_segment(segment_index)?;
        if segment.duration <= timeline_duration {
            return Err(Error::InvalidConfig(
                "Cannot shrink segment to zero or negative duration".into(),
            ));
        }

        // 计算源文件中需要跳过的时长（考虑 playback_speed 和 global_speed）
        // timeline shrink -> source shrink = timeline_duration * (playback_speed * global_speed)
        let source_shrink_duration = Duration::from_secs_f64(
            timeline_duration.as_secs_f64()
                * (segment.playback_speed * segment.global_speed) as f64,
        );

        self.modify_segment(segment_index, |seg| {
            if !shift_timeline {
                seg.timeline_offset += timeline_duration;
            }
            // 对于 image/subtitle 类型，不修改 source_offset
            if !seg.metadata.is_time_independent() {
                seg.source_offset += source_shrink_duration;
            }
            seg.duration = seg.duration - timeline_duration;
            seg.original_duration = seg.original_duration.saturating_sub(source_shrink_duration);
        })?;

        if shift_timeline {
            let shift_amount = timeline_duration;
            for i in (segment_index + 1)..self.segments_count() {
                self.shift_segment_timeline_backward(i, shift_amount)?;
            }
        }

        Self::update_track_duration(self);
        Ok(())
    }

    pub fn shrink_segment_right(
        &mut self,
        segment_index: usize,
        timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Result<()> {
        let segment = self.get_segment(segment_index)?;
        if segment.duration <= timeline_duration {
            return Err(Error::InvalidConfig(
                "Cannot shrink segment to zero or negative duration".into(),
            ));
        }

        // 计算源文件中需要减少的时长（考虑 playback_speed 和 global_speed）
        let source_shrink_duration = Duration::from_secs_f64(
            timeline_duration.as_secs_f64()
                * (segment.playback_speed * segment.global_speed) as f64,
        );

        self.modify_segment(segment_index, |seg| {
            seg.duration = seg.duration - timeline_duration;
            seg.original_duration = seg.original_duration.saturating_sub(source_shrink_duration);
        })?;

        if shift_timeline {
            let shift_amount = timeline_duration;
            for i in (segment_index + 1)..self.segments_count() {
                self.shift_segment_timeline_backward(i, shift_amount)?;
            }
        }

        Self::update_track_duration(self);
        Ok(())
    }

    // 如果超出源文件开始位置，将自动限制到源文件开始
    pub fn stretch_segment_left(
        &mut self,
        segment_index: usize,
        timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Result<()> {
        if segment_index >= self.segments_count() {
            return Err(Error::IndexOutOfBounds(
                segment_index,
                self.segments_count(),
            ));
        }

        let segment = self.get_segment(segment_index)?;

        // 对于 image/subtitle 类型，不限制拉伸时长
        let actual_timeline_duration = if segment.metadata.is_time_independent() {
            timeline_duration
        } else {
            let source_increase_needed = Duration::from_secs_f64(
                timeline_duration.as_secs_f64() * segment.playback_speed as f64,
            );
            let actual_source_increase = source_increase_needed.min(segment.source_offset);
            Duration::from_secs_f64(
                actual_source_increase.as_secs_f64() / segment.playback_speed as f64,
            )
        };

        // 计算源内容增加量（用于更新 original_duration）
        let actual_source_increase = Duration::from_secs_f64(
            actual_timeline_duration.as_secs_f64()
                * (segment.global_speed * segment.playback_speed) as f64,
        );

        self.modify_segment(segment_index, |seg| {
            if !shift_timeline {
                seg.timeline_offset = seg.timeline_offset.saturating_sub(actual_timeline_duration);
            }
            if !seg.metadata.is_time_independent() {
                seg.source_offset = seg.source_offset.saturating_sub(actual_source_increase);
            }
            seg.original_duration = seg.original_duration.saturating_add(actual_source_increase);
            seg.duration = seg.duration.saturating_add(actual_timeline_duration);
        })?;

        if shift_timeline && actual_timeline_duration > Duration::ZERO {
            let shift_amount = actual_timeline_duration;
            for i in (segment_index + 1)..self.segments_count() {
                self.shift_segment_timeline(i, shift_amount)?;
            }
        }

        Self::update_track_duration(self);
        Ok(())
    }

    // 如果超出源文件结束位置，将自动限制到源文件结束
    pub fn stretch_segment_right(
        &mut self,
        segment_index: usize,
        timeline_duration: Duration,
        shift_timeline: bool,
    ) -> Result<()> {
        if segment_index >= self.segments_count() {
            return Err(Error::IndexOutOfBounds(
                segment_index,
                self.segments_count(),
            ));
        }

        let segment = self.get_segment(segment_index)?;

        // 对于 image/subtitle 类型，不限制拉伸时长
        let actual_timeline_duration = if segment.metadata.is_time_independent() {
            timeline_duration
        } else {
            // 需要的源内容增加量 = timeline_increase * playback_speed
            let source_increase_needed = Duration::from_secs_f64(
                timeline_duration.as_secs_f64() * segment.playback_speed as f64,
            );
            let current_source_end = segment.source_offset + segment.original_duration;
            let remaining_source = segment.metadata.duration.saturating_sub(current_source_end);
            let actual_source_increase = source_increase_needed.min(remaining_source);
            Duration::from_secs_f64(
                actual_source_increase.as_secs_f64() / segment.playback_speed as f64,
            )
        };

        // 计算源内容增加量（用于更新 original_duration）
        let actual_source_increase = Duration::from_secs_f64(
            actual_timeline_duration.as_secs_f64()
                * (segment.global_speed * segment.playback_speed) as f64,
        );

        self.modify_segment(segment_index, |seg| {
            seg.duration = seg.duration.saturating_add(actual_timeline_duration);
            seg.original_duration = seg.original_duration.saturating_add(actual_source_increase);
        })?;

        if shift_timeline && actual_timeline_duration > Duration::ZERO {
            let shift_amount = actual_timeline_duration;
            for i in (segment_index + 1)..self.segments_count() {
                self.shift_segment_timeline(i, shift_amount)?;
            }
        }

        Self::update_track_duration(self);
        Ok(())
    }

    pub fn is_segment_overlap(&self, segment_index1: usize, segment_index2: usize) -> bool {
        let Ok(seg1) = self.get_segment(segment_index1) else {
            return false;
        };

        let Ok(seg2) = self.get_segment(segment_index2) else {
            return false;
        };

        let start1 = seg1.timeline_offset;
        let end1 = seg1.timeline_offset + seg1.duration;
        let start2 = seg2.timeline_offset;
        let end2 = seg2.timeline_offset + seg2.duration;

        // 检查两个区间是否重叠
        start1 < end2 && start2 < end1
    }

    #[inline]
    fn update_track_duration(track: &mut Track) {
        match track {
            Track::Video(track) => Arc::make_mut(track).update_duration(),
            Track::Audio(track) => Arc::make_mut(track).update_duration(),
            Track::Subtitle(track) => Arc::make_mut(track).update_duration(),
            Track::Image(track) => Arc::make_mut(track).update_duration(),
            Track::Text(track) => Arc::make_mut(track).update_duration(),
        }
    }

    #[inline]
    pub fn set_hiding(&mut self, hiding: bool) {
        match self {
            Track::Video(track) => Arc::make_mut(track).hiding = hiding,
            Track::Audio(track) => Arc::make_mut(track).hiding = hiding,
            Track::Subtitle(track) => Arc::make_mut(track).hiding = hiding,
            Track::Image(track) => Arc::make_mut(track).hiding = hiding,
            Track::Text(track) => Arc::make_mut(track).hiding = hiding,
        }
    }

    #[inline]
    pub fn is_hiding(&self) -> bool {
        match self {
            Track::Video(track) => track.hiding,
            Track::Audio(track) => track.hiding,
            Track::Subtitle(track) => track.hiding,
            Track::Image(track) => track.hiding,
            Track::Text(track) => track.hiding,
        }
    }

    #[inline]
    pub fn set_muted(&mut self, muted: bool) {
        match self {
            Track::Video(track) => Arc::make_mut(track).muted = muted,
            _ => {}
        }
    }

    #[inline]
    pub fn is_muted(&self) -> bool {
        match self {
            Track::Video(track) => track.muted,
            _ => false,
        }
    }

    #[inline]
    pub fn set_locked(&mut self, locked: bool) {
        match self {
            Track::Video(track) => Arc::make_mut(track).locked = locked,
            Track::Audio(track) => Arc::make_mut(track).locked = locked,
            Track::Subtitle(track) => Arc::make_mut(track).locked = locked,
            Track::Image(track) => Arc::make_mut(track).locked = locked,
            Track::Text(track) => Arc::make_mut(track).locked = locked,
        }
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        match self {
            Track::Video(track) => track.locked,
            Track::Audio(track) => track.locked,
            Track::Subtitle(track) => track.locked,
            Track::Image(track) => track.locked,
            Track::Text(track) => track.locked,
        }
    }

    #[inline]
    pub fn is_subtitle(&self) -> bool {
        matches!(self, Track::Subtitle(_))
    }

    #[inline]
    pub fn is_image(&self) -> bool {
        matches!(self, Track::Image(_))
    }

    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self, Track::Text(_))
    }

    #[inline]
    pub fn is_video_or_audio(&self) -> bool {
        matches!(self, Track::Video(_) | Track::Audio(_))
    }

    #[inline]
    pub fn priority(&self) -> TrackPriority {
        match self {
            Track::Text(_) => TrackPriority::TEXT,
            Track::Subtitle(_) => TrackPriority::SUBTITLE,
            Track::Image(_) => TrackPriority::IMAGE,
            Track::Video(_) => TrackPriority::VIDEO,
            Track::Audio(_) => TrackPriority::AUDIO,
        }
    }

    // 判断两个轨道类型是否可以互换位置（优先级相同）
    #[inline]
    pub fn can_swap_with(&self, other: &Track) -> bool {
        self.priority() == other.priority()
    }

    fn create_subtitle_tracks(
        path: &Path,
        metadata: Arc<Metadata>,
        stream_index: usize,
        total_duration: Duration,
        global_speed: f32,
    ) -> Result<Vec<Track>> {
        // Special handling for LRC files - FFmpeg cannot parse them
        if path
            .extension()
            .map(|e| e.to_ascii_lowercase() == "lrc")
            .unwrap_or(false)
        {
            let segments = extract_lrc_as_segments(path, metadata.clone(), global_speed)?;
            if segments.is_empty() {
                return Ok(vec![]);
            }
            return Ok(vec![Track::Subtitle(Arc::new(SubtitleTrack::new(
                InnerTrack::new(metadata, total_duration, segments),
            )))]);
        }

        let entries = extract_subtitles(path, stream_index)?;

        if entries.is_empty() {
            return Ok(vec![]);
        }

        let segments: Vec<Arc<Segment>> = entries
            .iter()
            .map(|entry| {
                let segment_duration = entry.end.saturating_sub(entry.start);
                Arc::new(
                    Segment::new_with_source_offset(
                        entry.start,    // timeline_offset（初始化时使用源时间）
                        Duration::ZERO, // source_offset = 0（不受源时间限制）
                        segment_duration,
                        1.0,
                        global_speed,
                        metadata.clone(),
                    )
                    .with_subtitle_text(&entry.text),
                )
            })
            .collect();

        Ok(vec![Track::Subtitle(Arc::new(SubtitleTrack::new(
            InnerTrack::new(metadata, total_duration, segments),
        )))])
    }
}
