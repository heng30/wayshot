use super::{CURRENT_PROJECT_VERSION, filters::*, metadata::*};
use crate::{
    Error, Result,
    filters::{
        keyframe::KeyframeTracks,
        subtitle::style::{SubtitleStyle, text_alignment::TextAlignment},
        traits::SubtitleEntry,
    },
    media::library::MediaList,
    metadata::Metadata,
    tracks::{
        audio_track::AudioTrack,
        image_track::ImageTrack,
        manager::Manager,
        segment::Segment,
        subtitle_track::SubtitleTrack,
        text_track::{TextElement, TextTrack},
        track::{InnerTrack, Track},
        video_track::VideoTrack,
    },
};
use chrono::{DateTime, Utc};
use image::Rgba;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSummaryData {
    pub start_ms: u64,
    pub end_ms: u64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkData {
    pub time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub duration_secs: f64,
    pub tracks: Vec<TrackData>,
    pub playlist: MediaList,
    #[serde(default)]
    pub preview_config: ProjectPreviewConfig,
    #[serde(default)]
    pub global_filters: Vec<GlobalFilterData>,
    #[serde(default)]
    pub chapter_summary: Vec<ChapterSummaryData>,
    #[serde(default)]
    pub bookmarks: Vec<BookmarkData>,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub is_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "track_type")]
pub enum TrackData {
    #[serde(rename = "video")]
    Video(VideoTrackData),

    #[serde(rename = "audio")]
    Audio(AudioTrackData),

    #[serde(rename = "subtitle")]
    Subtitle(SubtitleTrackData),

    #[serde(rename = "image")]
    Image(ImageTrackData),

    #[serde(rename = "text")]
    Text(TextTrackData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTrackData {
    pub name: String,
    pub hiding: bool,
    pub muted: bool,
    #[serde(default)]
    pub locked: bool,
    pub metadata: MetadataData,
    pub duration_secs: f64,
    pub segments: Vec<SegmentData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrackData {
    pub name: String,
    pub hiding: bool,
    #[serde(default)]
    pub locked: bool,
    pub metadata: MetadataData,
    pub duration_secs: f64,
    pub segments: Vec<SegmentData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrackData {
    pub name: String,
    pub hiding: bool,
    #[serde(default)]
    pub locked: bool,
    pub metadata: MetadataData,
    pub duration_secs: f64,
    pub segments: Vec<SegmentData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageTrackData {
    pub name: String,
    pub hiding: bool,
    #[serde(default)]
    pub locked: bool,
    pub metadata: MetadataData,
    pub duration_secs: f64,
    pub segments: Vec<SegmentData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextTrackData {
    pub name: String,
    pub hiding: bool,
    #[serde(default)]
    pub locked: bool,
    pub duration_secs: f64,
    pub segments: Vec<TextSegmentData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSegmentData {
    pub id: String,
    pub timeline_offset_secs: f64,
    pub duration_secs: f64,
    #[serde(default)]
    pub original_duration_secs: f64,
    #[serde(default = "default_speed")]
    pub global_speed: f32,
    pub element: TextElementData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TextElementData {
    pub text: String,
    pub position: (f32, f32),
    pub opacity: f32,
    pub rotation: f32,

    // Complete style fields (SubtitleStyle doesn't have Serialize/Deserialize)
    pub font_size: u32,
    pub font_path: Option<String>,
    pub font_family: String,
    pub font_style: String,
    pub primary_color: Option<(u8, u8, u8, u8)>,
    pub background_color: Option<(u8, u8, u8, u8)>,
    pub outline_color: Option<(u8, u8, u8, u8)>,
    pub outline_width: Option<u32>,
    pub border_radius: Option<u32>,
    pub alignment: Option<u32>,
    pub margin_vertical: Option<u32>,
    pub margin_horizontal: Option<u32>,
    pub padding: Option<u32>,
    pub border_width: Option<u32>,
    pub border_color: Option<(u8, u8, u8, u8)>,
    pub text_alignment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleEntryData {
    pub start_secs: f64,
    pub end_secs: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentData {
    pub hiding: bool,
    #[serde(default)]
    pub audio_muted: bool,
    pub timeline_offset_secs: f64,
    pub source_offset_secs: f64,
    pub duration_secs: f64,
    #[serde(default)]
    pub original_duration_secs: f64,
    #[serde(default = "default_speed")]
    pub playback_speed: f32,
    #[serde(default = "default_speed")]
    pub global_speed: f32,
    pub metadata: MetadataData,
    pub subtitle_text: Option<String>,
    pub video_filters: Vec<VideoFilterData>,
    pub audio_filters: Vec<AudioFilterData>,
    pub subtitle_filters: Vec<SubtitleFilterData>,
    pub image_filters: Vec<ImageFilterData>,
}

impl From<&SubtitleEntry> for SubtitleEntryData {
    fn from(entry: &SubtitleEntry) -> Self {
        Self {
            start_secs: entry.start.as_secs_f64(),
            end_secs: entry.end.as_secs_f64(),
            text: entry.text.clone(),
        }
    }
}

impl From<&Segment> for SegmentData {
    fn from(segment: &Segment) -> Self {
        Self {
            hiding: segment.hiding,
            audio_muted: segment.audio_muted,
            timeline_offset_secs: segment.timeline_offset.as_secs_f64(),
            source_offset_secs: segment.source_offset.as_secs_f64(),
            duration_secs: segment.duration.as_secs_f64(),
            original_duration_secs: segment.original_duration.as_secs_f64(),
            playback_speed: segment.playback_speed,
            global_speed: segment.global_speed,
            metadata: segment.metadata.as_ref().into(),
            subtitle_text: segment.subtitle_text.clone(),
            video_filters: segment
                .video_filters
                .iter()
                .map(|wrapper| video_filter_wrapper_to_data(wrapper))
                .collect(),
            audio_filters: segment
                .audio_filters
                .iter()
                .map(|wrapper| audio_filter_wrapper_to_data(wrapper))
                .collect(),
            subtitle_filters: segment
                .subtitle_filters
                .iter()
                .map(|wrapper| subtitle_filter_wrapper_to_data(wrapper))
                .collect(),
            image_filters: segment
                .image_filters
                .iter()
                .map(|wrapper| image_filter_wrapper_to_data(wrapper))
                .collect(),
        }
    }
}

impl From<&Track> for TrackData {
    fn from(track: &Track) -> Self {
        match track {
            Track::Video(vt) => TrackData::Video(VideoTrackData::from_video_track(vt)),
            Track::Audio(at) => TrackData::Audio(AudioTrackData::from_audio_track(at)),
            Track::Subtitle(st) => TrackData::Subtitle(SubtitleTrackData::from_subtitle_track(st)),
            Track::Image(ot) => TrackData::Image(ImageTrackData::from_image_track(ot)),
            Track::Text(tt) => TrackData::Text(TextTrackData::from_text_track(tt)),
        }
    }
}

impl VideoTrackData {
    fn from_video_track(track: &Arc<VideoTrack>) -> Self {
        Self {
            name: track.name.clone(),
            hiding: track.hiding,
            muted: track.muted,
            locked: track.locked,
            metadata: track.track.metadata.as_ref().into(),
            duration_secs: track.track.duration.as_secs_f64(),
            segments: track
                .track
                .segments
                .iter()
                .map(|s| s.as_ref().into())
                .collect(),
        }
    }
}

impl AudioTrackData {
    fn from_audio_track(track: &Arc<AudioTrack>) -> Self {
        Self {
            name: track.name.clone(),
            hiding: track.hiding,
            locked: track.locked,
            metadata: track.track.metadata.as_ref().into(),
            duration_secs: track.track.duration.as_secs_f64(),
            segments: track
                .track
                .segments
                .iter()
                .map(|s| s.as_ref().into())
                .collect(),
        }
    }
}

impl SubtitleTrackData {
    fn from_subtitle_track(track: &Arc<SubtitleTrack>) -> Self {
        Self {
            name: track.name.clone(),
            hiding: track.hiding,
            locked: track.locked,
            metadata: track.track.metadata.as_ref().into(),
            duration_secs: track.track.duration.as_secs_f64(),
            segments: track
                .track
                .segments
                .iter()
                .map(|s| s.as_ref().into())
                .collect(),
        }
    }
}

impl ImageTrackData {
    fn from_image_track(track: &Arc<ImageTrack>) -> Self {
        Self {
            name: track.name.clone(),
            hiding: track.hiding,
            locked: track.locked,
            metadata: track.track.metadata.as_ref().into(),
            duration_secs: track.track.duration.as_secs_f64(),
            segments: track
                .track
                .segments
                .iter()
                .map(|s| s.as_ref().into())
                .collect(),
        }
    }
}

impl TextTrackData {
    fn from_text_track(track: &Arc<TextTrack>) -> Self {
        Self {
            name: track.name.clone(),
            hiding: track.hiding,
            locked: track.locked,
            duration_secs: track.track.duration.as_secs_f64(),
            segments: track
                .track
                .segments
                .iter()
                .map(|s| s.as_ref().into())
                .collect(),
        }
    }
}

impl From<&Segment> for TextSegmentData {
    fn from(segment: &Segment) -> Self {
        Self {
            id: segment.uuid.clone(),
            timeline_offset_secs: segment.timeline_offset.as_secs_f64(),
            duration_secs: segment.duration.as_secs_f64(),
            original_duration_secs: segment.original_duration.as_secs_f64(),
            global_speed: segment.global_speed,
            element: segment
                .text_element
                .as_ref()
                .map(|e| e.into())
                .unwrap_or_default(),
        }
    }
}

impl From<&Arc<Segment>> for TextSegmentData {
    fn from(segment: &Arc<Segment>) -> Self {
        Self::from(segment.as_ref())
    }
}

impl From<&TextElement> for TextElementData {
    fn from(element: &TextElement) -> Self {
        Self {
            text: element.text.clone(),
            position: element.position,
            opacity: element.opacity,
            rotation: element.rotation,
            font_size: element.style.font_size,
            font_path: element.style.font_path.to_str().map(|s| s.to_string()),
            font_family: element.style.font_family.clone(),
            font_style: element.style.font_style.clone(),
            primary_color: element
                .style
                .primary_color
                .map(|c| (c[0], c[1], c[2], c[3])),
            background_color: element
                .style
                .background_color
                .map(|c| (c[0], c[1], c[2], c[3])),
            outline_color: element
                .style
                .outline_color
                .map(|c| (c[0], c[1], c[2], c[3])),
            outline_width: element.style.outline_width,
            border_radius: element.style.border_radius,
            alignment: element.style.alignment,
            margin_vertical: element.style.margin_vertical,
            margin_horizontal: element.style.margin_horizontal,
            padding: element.style.padding,
            border_width: element.style.border_width,
            border_color: element.style.border_color.map(|c| (c[0], c[1], c[2], c[3])),
            text_alignment: Some(element.style.text_alignment.to_string()),
        }
    }
}

impl From<&ManagerData> for ProjectFile {
    fn from(manager_data: &ManagerData) -> Self {
        let manager = manager_data
            .inner
            .as_ref()
            .expect("ManagerData inner manager is None");

        Self {
            version: CURRENT_PROJECT_VERSION,
            created_at: manager_data.created_at,
            modified_at: Utc::now(),
            duration_secs: manager.duration.as_secs_f64(),
            tracks: manager.tracks.iter().map(|t| t.into()).collect(),
            preview_config: manager_data.preview_config.clone(),
            playlist: manager_data.playlist.clone(),
            is_backup: manager_data.is_backup,
            global_filters: manager
                .global_filters
                .iter()
                .map(|f| global_filter_wrapper_to_data(f.as_ref()))
                .collect(),
            chapter_summary: manager_data.chapter_summary.clone(),
            bookmarks: manager_data.bookmarks.clone(),
            memo: manager_data.memo.clone(),
        }
    }
}

impl From<&SubtitleEntryData> for SubtitleEntry {
    fn from(data: &SubtitleEntryData) -> Self {
        Self {
            start: Duration::from_secs_f64(data.start_secs),
            end: Duration::from_secs_f64(data.end_secs),
            text: data.text.clone(),
        }
    }
}

impl TryFrom<&SegmentData> for Segment {
    type Error = crate::Error;

    fn try_from(data: &SegmentData) -> Result<Self> {
        let metadata = Arc::new((&data.metadata).try_into()?);

        let original_duration_secs = if data.original_duration_secs == 0.0 {
            data.duration_secs
        } else {
            data.original_duration_secs
        };

        let mut segment = Self::new_with_source_offset(
            Duration::from_secs_f64(data.timeline_offset_secs),
            Duration::from_secs_f64(data.source_offset_secs),
            Duration::from_secs_f64(original_duration_secs),
            data.playback_speed,
            data.global_speed,
            metadata,
        );

        segment.hiding = data.hiding;
        segment.audio_muted = data.audio_muted;
        segment.subtitle_text = data.subtitle_text.clone();

        for filter_data in &data.video_filters {
            match data_to_video_filter(filter_data) {
                Ok(filter) => {
                    let index = segment.video_filters.len();
                    segment.add_video_filter(filter);
                    segment
                        .set_video_filter_enabled(index, filter_data.enabled)
                        .ok();
                }
                Err(e) => log::warn!("Failed to load video filter: {}", e),
            }
        }

        for filter_data in &data.audio_filters {
            match data_to_audio_filter(filter_data) {
                Ok(filter) => {
                    let index = segment.audio_filters.len();
                    segment.add_audio_filter(filter);
                    segment
                        .set_audio_filter_enabled(index, filter_data.enabled)
                        .ok();
                }
                Err(e) => log::warn!("Failed to load audio filter: {}", e),
            }
        }

        for filter_data in &data.subtitle_filters {
            match data_to_subtitle_filter(filter_data) {
                Ok(filter) => {
                    let index = segment.subtitle_filters.len();
                    segment.add_subtitle_filter(filter);
                    segment
                        .set_subtitle_filter_enabled(index, filter_data.enabled)
                        .ok();
                }
                Err(e) => log::warn!("Failed to load subtitle filter: {}", e),
            }
        }

        for filter_data in &data.image_filters {
            match data_to_image_filter(filter_data) {
                Ok(filter) => {
                    let index = segment.image_filters.len();
                    segment.add_image_filter(filter);
                    segment
                        .set_image_filter_enabled(index, filter_data.enabled)
                        .ok();
                }
                Err(e) => log::warn!("Failed to load image filter: {}", e),
            }
        }

        Ok(segment)
    }
}

impl TryFrom<SegmentData> for Segment {
    type Error = crate::Error;

    fn try_from(data: SegmentData) -> Result<Self> {
        Segment::try_from(&data)
    }
}

impl TrackData {
    fn try_to_track(&self) -> Result<Track> {
        match self {
            TrackData::Video(data) => data.try_to_track(),
            TrackData::Audio(data) => data.try_to_track(),
            TrackData::Subtitle(data) => data.try_to_track(),
            TrackData::Image(data) => data.try_to_track(),
            TrackData::Text(data) => data.try_to_track(),
        }
    }
}

impl VideoTrackData {
    fn try_to_track(&self) -> Result<Track> {
        let metadata = Arc::new((&self.metadata).try_into()?);
        let segments = self
            .segments
            .iter()
            .map(|s| Ok(Arc::new(s.try_into()?)))
            .collect::<Result<Vec<_>>>()?;

        Ok(Track::Video(Arc::new(VideoTrack {
            name: self.name.clone(),
            hiding: self.hiding,
            muted: self.muted,
            locked: self.locked,
            track: InnerTrack {
                metadata,
                duration: Duration::from_secs_f64(self.duration_secs),
                segments,
            },
        })))
    }
}

impl AudioTrackData {
    fn try_to_track(&self) -> Result<Track> {
        let metadata = Arc::new((&self.metadata).try_into()?);
        let segments = self
            .segments
            .iter()
            .map(|s| Ok(Arc::new(s.try_into()?)))
            .collect::<Result<Vec<_>>>()?;

        Ok(Track::Audio(Arc::new(AudioTrack {
            name: self.name.clone(),
            hiding: self.hiding,
            locked: self.locked,
            track: InnerTrack {
                metadata,
                duration: Duration::from_secs_f64(self.duration_secs),
                segments,
            },
        })))
    }
}

impl SubtitleTrackData {
    fn try_to_track(&self) -> Result<Track> {
        let metadata = Arc::new((&self.metadata).try_into()?);
        let segments = self
            .segments
            .iter()
            .map(|s| Ok(Arc::new(s.try_into()?)))
            .collect::<Result<Vec<_>>>()?;

        Ok(Track::Subtitle(Arc::new(SubtitleTrack {
            name: self.name.clone(),
            hiding: self.hiding,
            locked: self.locked,
            track: InnerTrack::new(
                metadata,
                Duration::from_secs_f64(self.duration_secs),
                segments,
            ),
        })))
    }
}

impl ImageTrackData {
    fn try_to_track(&self) -> Result<Track> {
        let metadata = Arc::new((&self.metadata).try_into()?);
        let segments = self
            .segments
            .iter()
            .map(|s| Ok(Arc::new(s.try_into()?)))
            .collect::<Result<Vec<_>>>()?;

        Ok(Track::Image(Arc::new(ImageTrack {
            name: self.name.clone(),
            hiding: self.hiding,
            locked: self.locked,
            track: InnerTrack {
                metadata,
                duration: Duration::from_secs_f64(self.duration_secs),
                segments,
            },
        })))
    }
}

impl TextTrackData {
    fn try_to_track(&self) -> Result<Track> {
        let segments = self
            .segments
            .iter()
            .map(|s| {
                let metadata = Arc::new(Metadata {
                    path: PathBuf::from(format!("text://{}", s.id)),
                    ..Default::default()
                });

                // If original_duration_secs is set (new format), use it as the source duration
                // Otherwise (old project files), use duration_secs as the source duration
                let original_duration = if s.original_duration_secs > 0.0 {
                    Duration::from_secs_f64(s.original_duration_secs)
                } else {
                    Duration::from_secs_f64(s.duration_secs)
                };
                let global_speed = s.global_speed;

                Ok(Arc::new(
                    Segment::new(
                        Duration::from_secs_f64(s.timeline_offset_secs),
                        original_duration,
                        metadata,
                        global_speed,
                    )
                    .with_text_element((&s.element).into()),
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Track::Text(Arc::new(TextTrack {
            name: self.name.clone(),
            hiding: self.hiding,
            locked: self.locked,
            track: InnerTrack::new(
                Arc::new(Metadata {
                    path: PathBuf::from("text://"),
                    ..Default::default()
                }),
                Duration::from_secs_f64(self.duration_secs),
                segments,
            ),
        })))
    }
}

impl From<&TextElementData> for TextElement {
    fn from(data: &TextElementData) -> Self {
        Self {
            text: data.text.clone(),
            position: data.position,
            opacity: data.opacity,
            rotation: data.rotation,
            style: SubtitleStyle {
                font_size: data.font_size,
                font_path: data
                    .font_path
                    .as_ref()
                    .map(|s| PathBuf::from(s))
                    .unwrap_or_default(),
                font_family: data.font_family.clone(),
                font_style: data.font_style.clone(),
                primary_color: data.primary_color.map(|c| Rgba([c.0, c.1, c.2, c.3])),
                background_color: data.background_color.map(|c| Rgba([c.0, c.1, c.2, c.3])),
                outline_color: data.outline_color.map(|c| Rgba([c.0, c.1, c.2, c.3])),
                outline_width: data.outline_width,
                border_radius: data.border_radius,
                alignment: data.alignment,
                margin_vertical: data.margin_vertical,
                margin_horizontal: data.margin_horizontal,
                padding: data.padding,
                border_width: data.border_width,
                border_color: data.border_color.map(|c| Rgba([c.0, c.1, c.2, c.3])),
                text_alignment: data
                    .text_alignment
                    .as_ref()
                    .and_then(|s| s.parse::<TextAlignment>().ok())
                    .unwrap_or_default(),
            },
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Resolution {
    Original,
    P480,
    P720,
    #[default]
    P1080,
    P2K,
    P4K,
    Portrait480P,
    Portrait720P,
    Portrait1080P,
    Portrait4K,
    Square480P,
    Square720P,
    Square1080P,
    InstagramPortrait,
}

#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
#[derivative(Default)]
pub struct ProjectPreviewConfig {
    #[derivative(Default(value = "25.0"))]
    pub fps: f32,

    #[derivative(Default(value = "Resolution::P480"))]
    pub resolution: Resolution,

    #[derivative(Default(value = "2"))]
    pub channels: u16,

    #[derivative(Default(value = "44100"))]
    pub sample_rate: u32,
}

pub struct ManagerData {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub preview_config: ProjectPreviewConfig,
    pub playlist: MediaList,
    pub inner: Option<Manager>,
    pub chapter_summary: Vec<ChapterSummaryData>,
    pub bookmarks: Vec<BookmarkData>,
    pub memo: String,
    pub is_backup: bool,
}

impl ManagerData {
    pub fn new(manager: Manager) -> Self {
        Self {
            version: CURRENT_PROJECT_VERSION,
            created_at: chrono::Utc::now(),
            preview_config: ProjectPreviewConfig::default(),
            playlist: MediaList::new("Default".to_string()),
            inner: Some(manager),
            chapter_summary: vec![],
            bookmarks: vec![],
            memo: String::new(),
            is_backup: false,
        }
    }

    pub fn with_preview_config(mut self, config: ProjectPreviewConfig) -> Self {
        self.preview_config = config;
        self
    }

    pub fn with_playlist(mut self, playlist: MediaList) -> Self {
        self.playlist = playlist;
        self
    }

    pub fn with_chapter_summary(mut self, chapter_summary: Vec<ChapterSummaryData>) -> Self {
        self.chapter_summary = chapter_summary;
        self
    }

    pub fn with_bookmarks(mut self, bookmarks: Vec<BookmarkData>) -> Self {
        self.bookmarks = bookmarks;
        self
    }

    pub fn with_memo(mut self, memo: String) -> Self {
        self.memo = memo;
        self
    }

    pub fn with_is_backup(mut self, is_backup: bool) -> Self {
        self.is_backup = is_backup;
        self
    }
}

impl TryFrom<&ProjectFile> for ManagerData {
    type Error = crate::Error;

    fn try_from(file: &ProjectFile) -> Result<Self> {
        if file.version > CURRENT_PROJECT_VERSION {
            return Err(crate::Error::UnsupportedProjectVersion {
                file_version: file.version,
                current_version: CURRENT_PROJECT_VERSION,
            });
        }

        let mut manager = Manager::new();
        manager.duration = Duration::from_secs_f64(file.duration_secs);

        for track_data in &file.tracks {
            let track = track_data.try_to_track()?;
            manager.tracks.push(track);
        }

        for filter_data in &file.global_filters {
            match data_to_global_filter(filter_data) {
                Ok(filter) => manager.global_filters.push(Arc::new(filter)),
                Err(e) => log::warn!("Failed to load global filter: {}", e),
            }
        }

        let manager_data = ManagerData {
            version: file.version,
            created_at: file.created_at,
            preview_config: file.preview_config.clone(),
            playlist: file.playlist.clone(),
            inner: Some(manager),
            chapter_summary: file.chapter_summary.clone(),
            bookmarks: file.bookmarks.clone(),
            memo: file.memo.clone(),
            is_backup: file.is_backup,
        };

        Ok(manager_data)
    }
}

impl TryFrom<ProjectFile> for ManagerData {
    type Error = crate::Error;

    fn try_from(file: ProjectFile) -> Result<Self> {
        ManagerData::try_from(&file)
    }
}

pub fn load_project<P: AsRef<Path>>(path: P) -> Result<ManagerData> {
    let path = path.as_ref();
    let json = std::fs::read_to_string(&path)?;
    let project_file: ProjectFile = serde_json::from_str(&json)?;

    let base_dir = if project_file.is_backup {
        path.parent().map(|p| p.to_path_buf())
    } else {
        None
    };

    let project_file = if project_file.is_backup {
        resolve_project_file_relative_paths(project_file, base_dir.as_ref())
    } else {
        project_file
    };

    log::info!(
        "Project loaded from: {} (version {}, is_backup: {})",
        path.display(),
        project_file.version,
        project_file.is_backup
    );

    ManagerData::try_from(project_file)
}

pub fn save_project<P: AsRef<Path>>(manager_data: &ManagerData, path: P) -> Result<()> {
    if manager_data.inner.is_none() {
        return Err(Error::InvalidConfig("manager is None".to_string()));
    }

    let project_file = ProjectFile::from(manager_data);
    let json = serde_json::to_string_pretty(&project_file)?;

    fs::write(&path, json)?;

    log::info!("Project saved to: {}", path.as_ref().display());

    Ok(())
}

fn resolve_project_file_relative_paths(
    mut project_file: ProjectFile,
    base_dir: Option<&PathBuf>,
) -> ProjectFile {
    project_file.playlist.resolve_relative_paths(base_dir);

    for track_data in &mut project_file.tracks {
        resolve_track_data_paths(track_data, base_dir);
    }

    project_file
}

fn resolve_track_data_paths(track_data: &mut TrackData, base_dir: Option<&PathBuf>) {
    match track_data {
        TrackData::Video(data) => {
            data.metadata.path = resolve_relative_path(&data.metadata.path, base_dir);
            for seg in &mut data.segments {
                seg.metadata.path = resolve_relative_path(&seg.metadata.path, base_dir);
            }
        }
        TrackData::Audio(data) => {
            data.metadata.path = resolve_relative_path(&data.metadata.path, base_dir);
            for seg in &mut data.segments {
                seg.metadata.path = resolve_relative_path(&seg.metadata.path, base_dir);
            }
        }
        TrackData::Subtitle(data) => {
            data.metadata.path = resolve_relative_path(&data.metadata.path, base_dir);
            for seg in &mut data.segments {
                seg.metadata.path = resolve_relative_path(&seg.metadata.path, base_dir);
            }
        }
        TrackData::Image(data) => {
            data.metadata.path = resolve_relative_path(&data.metadata.path, base_dir);
            for seg in &mut data.segments {
                seg.metadata.path = resolve_relative_path(&seg.metadata.path, base_dir);
            }
        }
        // Text tracks don't have file-based metadata, no paths to resolve
        TrackData::Text(_) => {}
    }
}

fn default_speed() -> f32 {
    1.0
}
