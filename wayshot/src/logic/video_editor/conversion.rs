use super::command::with_history_manager;
use crate::db::{PresetTextStyleData, TextStyleConfig};
use crate::logic::video_editor::project::TEXT_STYLE_CONFIG_ID;
use crate::slint_generatedAppWindow::{
    AudioChannels as UIAudioChannels, AudioFormat as UIAudioFormat,
    AudioSampleRate as UIAudioSampleRate, CircleMaskDetail as UICircleMaskDetail,
    CropDetail as UICropDetail, DrawCircleDetail as UIDrawCircleDetail,
    DrawRectangleDetail as UIDrawRectangleDetail, FilterType as UIFilterType,
    FlyInDetail as UIFlyInDetail, FocusDetail as UIFocusDetail, Fps as UIFps,
    GaussianBlurDetail as UIGaussianBlurDetail, LinearMaskDetail as UILinearMaskDetail,
    LiquidGlassDetail as UILiquidGlassDetail, LocalMagnifyDetail as UILocalMagnifyDetail,
    MagnifierDetail as UIMagnifierDetail, MediaType as UIMediaType,
    MirrorMaskDetail as UIMirrorMaskDetail, MosaicDetail as UIMosaicDetail,
    PresetTextStyle as UIPresetTextStyle, RectangleMaskDetail as UIRectangleMaskDetail,
    Resolution as UIResolution, SegmentFilter as UISegmentFilter, SubtitleType as UISubtitleType,
    TextElement as UITextElement, TextHighlightDetail as UITextHighlightDetail,
    TransformDetail as UITransformDetail, VideoEditorLayerImage as UIVideoEditorLayerImage,
    VideoEditorPlaylistItem as UIVideoEditorPlaylistItem,
    VideoEditorPreviewConfig as UIVideoEditorPreviewConfig,
    VideoEditorRecentEntry as UIVideoEditorRecentEntry,
    VideoEditorRecoveryInfo as UIVideoEditorRecoveryInfo,
    VideoEditorSegmentMetadata as UIVideoEditorSegmentMetadata,
    VideoEditorTrack as UIVideoEditorTrack, VideoEditorTrackSegment as UIVideoEditorTrackSegment,
    VideoEditorTrackType as UIVideoEditorTrackType,
    VideoEditorTracksManager as UIVideoEditorTracksManager, VideoPreviewSize as UIVideoPreviewSize,
    ZoomDetail as UIZoomDetail,
};
use slint::{Image, ModelRc, SharedPixelBuffer, SharedString, ToSharedString, VecModel};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use video_editor::{
    commands::filter::FilterType,
    export::{AudioExportFormat, SubtitleFormat},
    filters::{
        AudioFilter, AudioFilterWrapper, ImageFilterWrapper, SubtitleFilter, SubtitleFilterWrapper,
        VideoFilter, VideoFilterWrapper,
        audio::{
            AudioSpeedFilter, CompressorFilter, CopyChannelFilter, DenoiseFilter,
            FadeInFilter as AudioFadeInFilter, FadeOutFilter as AudioFadeOutFilter, GainFilter,
            LimiterFilter, MuteFilter, NoiseGateFilter, NormalizeFilter, VoiceChangerFilter,
        },
        subtitle::style::{
            AlignmentFilter, BackgroundColorFilter, BorderRadiusFilter, FontPathFilter,
            FontSizeFilter, MarginHorizontalFilter, MarginVerticalFilter, OutlineColorFilter,
            OutlineWidthFilter, PaddingFilter, PrimaryColorFilter, SubtitleStyle, TextAlignment,
        },
        video::{
            BackgroundFilter, BorderFilter, BreathingFilter, ChromaKeyFilter, CircleMaskFilter,
            CropFilter, DeviceFrameFilter, DirectionalBlurFilter, DrawCircleFilter,
            DrawRectangleFilter, EdgeDetectFilter, FadeInFilter as VideoFadeInFilter,
            FadeOutFilter as VideoFadeOutFilter, FisheyeFilter, FlipFilter, FlyInFilter,
            FocusFilter, FrameExtractFilter, GaussianBlurFilter, GenieFilter, GrainFilter,
            GrayscaleFilter, GridFilter, HSLAdjustFilter, LightingFilter as VELightingFilter,
            LinearMaskFilter, LiquidGlassFilter, Live2dFilter, LocalMagnifyFilter, MagnifierFilter,
            MirrorMaskFilter, MosaicFilter, OldFilmFilter, OpacityFilter, PageFlipFilter,
            RectangleMaskFilter, ShadowFilter, SharpenFilter, SketchFilter, SlideFilter,
            SpeedFilter, SplitFilter, TextHighlightFilter, TransformFilter, VignetteFilter,
            WaveFilter, WipeFilter, ZoomFilter,
        },
    },
    media::{MediaItem, library::LibraryFolder, media_type::MediaType},
    metadata::{Metadata, MetadataType},
    project::{
        autosave::RecoveryInfo,
        project::{ProjectPreviewConfig, Resolution as ProjectPreviewResolution},
        recent::RecentFile,
    },
    tracks::{
        manager::Manager, segment::Segment, text_track::TextElement, track::Track,
        unified_mixer::UnifiedMixerConfig, video_frame_cache::VideoImage, video_track::LayerFrame,
    },
};

macro_rules! extract_filter_from_layer {
    ($layer:expr, $is_image_track:expr, $filter_type:ty) => {
        $layer
            .from_segment
            .as_ref()
            .and_then(|(_, segment)| {
                if $is_image_track {
                    segment
                        .image_filters
                        .iter()
                        .find(|f| f.inner.name() == <$filter_type>::NAME)
                        .and_then(|f| f.inner.as_any().downcast_ref::<$filter_type>())
                        .map(|t| t.clone().into())
                } else {
                    segment
                        .video_filters
                        .iter()
                        .find(|f| f.inner.name() == <$filter_type>::NAME)
                        .and_then(|f| f.inner.as_any().downcast_ref::<$filter_type>())
                        .map(|t| t.clone().into())
                }
            })
            .unwrap_or_else(|| <$filter_type>::default().into())
    };
}

macro_rules! filter_to_json_match {
    ($filter:expr, $($filter_type:ty), *) => {{
        match $filter.name() {
            $(
                <$filter_type>::NAME => {
                    if let Some(f) = $filter.as_any().downcast_ref::<$filter_type>() {
                        return serde_json::to_string(f).unwrap_or_default();
                    }
                }
            )*
            _ => {}
        }

        return String::new();
    }};
}

impl From<RecentFile> for UIVideoEditorRecentEntry {
    fn from(rf: RecentFile) -> Self {
        let modified_at = if let Ok(modified) =
            rf.last_modified.duration_since(SystemTime::UNIX_EPOCH)
            && let Some(datetime) = chrono::DateTime::from_timestamp(modified.as_secs() as i64, 0)
        {
            datetime.format("%Y-%m-%d").to_string().into()
        } else {
            "--:--:--".to_string().into()
        };

        UIVideoEditorRecentEntry {
            name: rf.name.into(),
            path: rf.path.to_string_lossy().to_string().into(),
            modified_at,
        }
    }
}

impl From<RecoveryInfo> for UIVideoEditorRecoveryInfo {
    fn from(recovery: RecoveryInfo) -> Self {
        let saved_time = recovery
            .saved_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown time".to_string());

        UIVideoEditorRecoveryInfo {
            temp_file_path: recovery.temp_file_path.to_string_lossy().to_string().into(),
            original_project_path: recovery
                .original_project_path
                .map(|p| p.to_string_lossy().to_string().into())
                .unwrap_or_default(),
            saved_time: saved_time.into(),
        }
    }
}

impl From<UIVideoEditorRecoveryInfo> for RecoveryInfo {
    fn from(ui_recovery: UIVideoEditorRecoveryInfo) -> Self {
        RecoveryInfo {
            temp_file_path: PathBuf::from(ui_recovery.temp_file_path.as_str()),
            original_project_path: if ui_recovery.original_project_path.is_empty() {
                None
            } else {
                Some(PathBuf::from(ui_recovery.original_project_path.as_str()))
            },
            saved_at: SystemTime::now(), // Not used for restore operations
            file_size: 0,                // Not used for restore operations
        }
    }
}

impl From<Manager> for UIVideoEditorTracksManager {
    fn from(manager: Manager) -> Self {
        let duration = manager.duration;
        let tracks = manager
            .into_iter()
            .map(|item| item.into())
            .collect::<Vec<_>>();

        UIVideoEditorTracksManager {
            duration: duration_to_millis(duration),
            tracks: ModelRc::new(VecModel::from(tracks)),
        }
    }
}

impl From<Track> for UIVideoEditorTrack {
    fn from(track: Track) -> Self {
        match track {
            Track::Video(inner) => UIVideoEditorTrack {
                name: if inner.name.is_empty() {
                    "V".to_string().into()
                } else {
                    inner.name.clone().into()
                },
                hiding: inner.hiding,
                locked: inner.locked,
                muted: inner.muted,
                duration: duration_to_millis(inner.track.duration),
                ty: UIVideoEditorTrackType::Video,
                segments: ModelRc::new(VecModel::from_slice(
                    &inner
                        .track
                        .segments
                        .clone()
                        .into_iter()
                        .map(|item| item.into())
                        .collect::<Vec<UIVideoEditorTrackSegment>>(),
                )),
            },
            Track::Audio(inner) => UIVideoEditorTrack {
                name: if inner.name.is_empty() {
                    "A".to_string().into()
                } else {
                    inner.name.clone().into()
                },
                hiding: inner.hiding,
                locked: inner.locked,
                muted: false,
                duration: duration_to_millis(inner.track.duration),
                ty: UIVideoEditorTrackType::Audio,
                segments: ModelRc::new(VecModel::from_slice(
                    &inner
                        .track
                        .segments
                        .clone()
                        .into_iter()
                        .map(|item| item.into())
                        .collect::<Vec<UIVideoEditorTrackSegment>>(),
                )),
            },
            Track::Subtitle(inner) => UIVideoEditorTrack {
                name: if inner.name.is_empty() {
                    "S".to_string().into()
                } else {
                    inner.name.clone().into()
                },
                hiding: inner.hiding,
                locked: inner.locked,
                muted: false,
                duration: duration_to_millis(inner.track.duration),
                ty: UIVideoEditorTrackType::Subtitle,
                segments: ModelRc::new(VecModel::from_slice(
                    &inner
                        .track
                        .segments
                        .clone()
                        .into_iter()
                        .map(|item| item.into())
                        .collect::<Vec<UIVideoEditorTrackSegment>>(),
                )),
            },
            Track::Image(inner) => UIVideoEditorTrack {
                name: if inner.name.is_empty() {
                    "O".to_string().into()
                } else {
                    inner.name.clone().into()
                },
                hiding: inner.hiding,
                locked: inner.locked,
                muted: false,
                duration: duration_to_millis(inner.track.duration),
                ty: UIVideoEditorTrackType::Image,
                segments: ModelRc::new(VecModel::from_slice(
                    &inner
                        .track
                        .segments
                        .clone()
                        .into_iter()
                        .map(|item| item.into())
                        .collect::<Vec<UIVideoEditorTrackSegment>>(),
                )),
            },
            Track::Text(inner) => UIVideoEditorTrack {
                name: if inner.name.is_empty() {
                    "T".to_string().into()
                } else {
                    inner.name.clone().into()
                },
                hiding: inner.hiding,
                locked: inner.locked,
                muted: false,
                duration: duration_to_millis(inner.track.duration),
                ty: UIVideoEditorTrackType::Text,
                segments: ModelRc::new(VecModel::from_slice(
                    &inner
                        .track
                        .segments
                        .clone()
                        .into_iter()
                        .map(|item| item.into())
                        .collect::<Vec<UIVideoEditorTrackSegment>>(),
                )),
            },
        }
    }
}

impl From<Arc<Segment>> for UIVideoEditorTrackSegment {
    fn from(seg: Arc<Segment>) -> Self {
        let left_thumbnail = seg
            .display_cache
            .thumbnail_left
            .as_ref()
            .map(|img| rgba_image_to_slint_image(img))
            .unwrap_or_default();

        let right_thumbnail = seg
            .display_cache
            .thumbnail_right
            .as_ref()
            .map(|img| rgba_image_to_slint_image(img))
            .unwrap_or_default();

        let (preview_audio_channels, preview_audio_samples, preview_audio_amplitude_scale) = seg
            .display_cache
            .audio_samples
            .as_ref()
            .map(|audio| {
                (
                    audio.channels as i32,
                    ModelRc::new(VecModel::from_slice(&audio.samples)),
                    1.0,
                )
            })
            .unwrap_or((0, ModelRc::new(VecModel::default()), 1.0));

        let filename = if seg.text_element.is_some() {
            "Text".into()
        } else {
            seg.metadata
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_shared_string()
        };

        let overlay_text = if let Some(text_element) = &seg.text_element {
            text_element.text.clone().into()
        } else {
            Default::default()
        };

        // 顺序不要动，get_filter_type_and_local_index函数使用了顺序
        let mut all_filters: Vec<UISegmentFilter> = Vec::new();
        all_filters.extend(video_filters_to_ui(&seg.video_filters));
        all_filters.extend(audio_filters_to_ui(&seg.audio_filters));
        all_filters.extend(subtitle_filters_to_ui(&seg.subtitle_filters));
        all_filters.extend(image_filters_to_ui(&seg.image_filters));

        UIVideoEditorTrackSegment {
            uuid: seg.uuid.clone().into(),
            filename,
            duration: duration_to_millis(seg.duration),
            original_duration: duration_to_millis(seg.original_duration),
            timeline_offset: duration_to_millis(seg.timeline_offset),
            source_offset: duration_to_millis(seg.source_offset),
            source_duration: duration_to_millis(seg.metadata.duration),
            playback_speed: seg.playback_speed,
            hiding: seg.hiding,
            audio_muted: seg.audio_muted,
            subtitle_text: seg.subtitle_text.clone().unwrap_or_default().into(),
            overlay_text,

            left_thumbnail,
            right_thumbnail,
            preview_audio_channels,
            preview_audio_samples,
            preview_audio_amplitude_scale,
            filters: ModelRc::new(VecModel::from_slice(&all_filters)),
        }
    }
}

impl From<&Metadata> for UIVideoEditorSegmentMetadata {
    fn from(metadata: &Metadata) -> Self {
        let filename = metadata
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_shared_string();

        let file_path = metadata.path.to_string_lossy().to_string().into();
        let file_size = cutil::str::pretty_size_string(metadata.size);

        // Format duration as HH:MM:SS or MM:SS
        let duration = if metadata.duration.is_zero() {
            "--:--".to_string()
        } else {
            let total_secs = metadata.duration.as_secs();
            let hours = total_secs / 3600;
            let minutes = (total_secs % 3600) / 60;
            let seconds = total_secs % 60;
            if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            } else {
                format!("{:02}:{:02}", minutes, seconds)
            }
        };

        let bitrate = if metadata.bitrate > 0 {
            format!("{} kbps", metadata.bitrate / 1000)
        } else {
            "--".to_string()
        };

        let format = metadata.format.join(", ");

        let media_type = match metadata.get_type() {
            MetadataType::Video => UIMediaType::Video,
            MetadataType::Audio => UIMediaType::Audio,
            MetadataType::Image => UIMediaType::Image,
            MetadataType::Subtitle => UIMediaType::Subtitle,
            MetadataType::None => UIMediaType::Video, // Default fallback
        };

        // Video metadata
        let (video_width, video_height, video_fps, video_codec, video_language) =
            if let Some(video) = metadata.first_video() {
                let codec_name = video.codec_id.name();
                (
                    video.width as i32,
                    video.height as i32,
                    video.fps,
                    codec_name.to_string(),
                    video.language.clone().unwrap_or_default(),
                )
            } else {
                (0, 0, 0.0, String::new(), String::new())
            };

        // Audio metadata
        let (audio_sample_rate, audio_channels, audio_codec, audio_language) =
            if let Some(audio) = metadata.first_audio() {
                let codec_name = audio.codec_id.name();
                (
                    audio.sample_rate as i32,
                    audio.channels as i32,
                    codec_name.to_string(),
                    audio.language.clone().unwrap_or_default(),
                )
            } else {
                (0, 0, String::new(), String::new())
            };

        UIVideoEditorSegmentMetadata {
            filename,
            file_path,
            file_size: file_size.into(),
            duration: duration.into(),
            bitrate: bitrate.into(),
            format: format.into(),
            media_type,
            video_width,
            video_height,
            video_fps,
            video_codec: video_codec.into(),
            video_language: video_language.into(),
            audio_sample_rate,
            audio_channels,
            audio_codec: audio_codec.into(),
            audio_language: audio_language.into(),
        }
    }
}

impl From<ProjectPreviewConfig> for UIVideoEditorPreviewConfig {
    fn from(value: ProjectPreviewConfig) -> Self {
        let fps = match value.fps as u32 {
            24 => UIFps::Fps24,
            25 => UIFps::Fps25,
            30 => UIFps::Fps30,
            60 => UIFps::Fps60,
            _ => UIFps::Fps25,
        };

        let resolution = match value.resolution {
            ProjectPreviewResolution::Original => UIResolution::Original,
            ProjectPreviewResolution::P480 => UIResolution::P480,
            ProjectPreviewResolution::P720 => UIResolution::P720,
            ProjectPreviewResolution::P1080 => UIResolution::P1080,
            ProjectPreviewResolution::P2K => UIResolution::P2K,
            ProjectPreviewResolution::P4K => UIResolution::P4K,
            ProjectPreviewResolution::Portrait480P => UIResolution::Portrait480P,
            ProjectPreviewResolution::Portrait720P => UIResolution::Portrait720P,
            ProjectPreviewResolution::Portrait1080P => UIResolution::Portrait1080P,
            ProjectPreviewResolution::Portrait4K => UIResolution::Portrait4K,
            ProjectPreviewResolution::Square480P => UIResolution::Square480P,
            ProjectPreviewResolution::Square720P => UIResolution::Square720P,
            ProjectPreviewResolution::Square1080P => UIResolution::Square1080P,
            ProjectPreviewResolution::InstagramPortrait => UIResolution::InstagramPortrait,
        };

        let channels = match value.channels {
            1 => UIAudioChannels::Mono,
            2 => UIAudioChannels::Stereo,
            _ => UIAudioChannels::Stereo,
        };

        let sample_rate = match value.sample_rate {
            8000 => UIAudioSampleRate::Hz8000,
            16000 => UIAudioSampleRate::Hz16000,
            24000 => UIAudioSampleRate::Hz24000,
            32000 => UIAudioSampleRate::Hz32000,
            44100 => UIAudioSampleRate::Hz44100,
            48000 => UIAudioSampleRate::Hz48000,
            96000 => UIAudioSampleRate::Hz96000,
            192000 => UIAudioSampleRate::Hz192000,
            _ => UIAudioSampleRate::Hz44100,
        };

        UIVideoEditorPreviewConfig {
            fps,
            resolution,
            channels,
            sample_rate,
        }
    }
}

impl From<UIVideoEditorPreviewConfig> for ProjectPreviewConfig {
    fn from(value: UIVideoEditorPreviewConfig) -> Self {
        let fps = match value.fps {
            UIFps::Fps24 => 24.0,
            UIFps::Fps25 => 25.0,
            UIFps::Fps30 => 30.0,
            UIFps::Fps60 => 60.0,
        };

        let resolution = match value.resolution {
            UIResolution::Original => ProjectPreviewResolution::Original,
            UIResolution::P480 => ProjectPreviewResolution::P480,
            UIResolution::P720 => ProjectPreviewResolution::P720,
            UIResolution::P1080 => ProjectPreviewResolution::P1080,
            UIResolution::P2K => ProjectPreviewResolution::P2K,
            UIResolution::P4K => ProjectPreviewResolution::P4K,
            UIResolution::Portrait480P => ProjectPreviewResolution::Portrait480P,
            UIResolution::Portrait720P => ProjectPreviewResolution::Portrait720P,
            UIResolution::Portrait1080P => ProjectPreviewResolution::Portrait1080P,
            UIResolution::Portrait4K => ProjectPreviewResolution::Portrait4K,
            UIResolution::Square480P => ProjectPreviewResolution::Square480P,
            UIResolution::Square720P => ProjectPreviewResolution::Square720P,
            UIResolution::Square1080P => ProjectPreviewResolution::Square1080P,
            UIResolution::InstagramPortrait => ProjectPreviewResolution::InstagramPortrait,
        };

        let channels = match value.channels {
            UIAudioChannels::Mono => 1,
            UIAudioChannels::Stereo => 2,
        };

        let sample_rate = match value.sample_rate {
            UIAudioSampleRate::Hz8000 => 8000,
            UIAudioSampleRate::Hz16000 => 16000,
            UIAudioSampleRate::Hz24000 => 24000,
            UIAudioSampleRate::Hz32000 => 32000,
            UIAudioSampleRate::Hz44100 => 44100,
            UIAudioSampleRate::Hz48000 => 48000,
            UIAudioSampleRate::Hz96000 => 96000,
            UIAudioSampleRate::Hz192000 => 192000,
        };

        ProjectPreviewConfig {
            fps,
            resolution,
            channels,
            sample_rate,
        }
    }
}

impl From<UIFps> for f32 {
    fn from(value: UIFps) -> Self {
        match value {
            UIFps::Fps24 => 24.0,
            UIFps::Fps25 => 25.0,
            UIFps::Fps30 => 30.0,
            UIFps::Fps60 => 60.0,
        }
    }
}

impl From<UIAudioChannels> for u16 {
    fn from(value: UIAudioChannels) -> Self {
        match value {
            UIAudioChannels::Mono => 1,
            UIAudioChannels::Stereo => 2,
        }
    }
}

impl From<UIAudioSampleRate> for u32 {
    fn from(value: UIAudioSampleRate) -> Self {
        match value {
            UIAudioSampleRate::Hz8000 => 8000,
            UIAudioSampleRate::Hz16000 => 16000,
            UIAudioSampleRate::Hz24000 => 24000,
            UIAudioSampleRate::Hz32000 => 32000,
            UIAudioSampleRate::Hz44100 => 44100,
            UIAudioSampleRate::Hz48000 => 48000,
            UIAudioSampleRate::Hz96000 => 96000,
            UIAudioSampleRate::Hz192000 => 192000,
        }
    }
}

impl From<UIVideoEditorPreviewConfig> for UnifiedMixerConfig {
    fn from(value: UIVideoEditorPreviewConfig) -> Self {
        let fps: Option<f32> = Some(value.fps.into());
        let output_channels: Option<u16> = Some(value.channels.into());
        let output_sample_rate: Option<u32> = Some(value.sample_rate.into());
        let resolution: Option<(u32, u32)> = value.resolution.into();
        let (output_width, output_height) = match resolution {
            Some((w, h)) => (Some(w), Some(h)),
            None => (None, None),
        };

        UnifiedMixerConfig::default()
            .with_output_fps(fps)
            .with_output_width(output_width)
            .with_output_height(output_height)
            .with_output_channels(output_channels)
            .with_output_sample_rate(output_sample_rate)
            .with_duration(None)
    }
}

impl From<MediaType> for UIVideoEditorTrackType {
    fn from(media_type: MediaType) -> UIVideoEditorTrackType {
        match media_type {
            MediaType::Video => UIVideoEditorTrackType::Video,
            MediaType::Audio => UIVideoEditorTrackType::Audio,
            MediaType::Image => UIVideoEditorTrackType::Image,
            MediaType::Subtitle => UIVideoEditorTrackType::Subtitle,
        }
    }
}

impl From<&Track> for UIVideoEditorTrackType {
    fn from(track: &Track) -> UIVideoEditorTrackType {
        match track {
            Track::Video(_) => UIVideoEditorTrackType::Video,
            Track::Audio(_) => UIVideoEditorTrackType::Audio,
            Track::Subtitle(_) => UIVideoEditorTrackType::Subtitle,
            Track::Image(_) => UIVideoEditorTrackType::Image,
            Track::Text(_) => UIVideoEditorTrackType::Text,
        }
    }
}

impl From<MediaType> for UIMediaType {
    fn from(media_type: MediaType) -> UIMediaType {
        match media_type {
            MediaType::Video => UIMediaType::Video,
            MediaType::Audio => UIMediaType::Audio,
            MediaType::Image => UIMediaType::Image,
            MediaType::Subtitle => UIMediaType::Subtitle,
        }
    }
}

impl From<MediaItem> for UIVideoEditorPlaylistItem {
    fn from(item: MediaItem) -> Self {
        let media_type: UIMediaType = item.media_type.into();

        let duration = if let Some(d) = item.duration {
            format!("{:.1}s", d.as_secs_f32())
        } else {
            "--:--".to_string()
        };

        let file_size = cutil::str::pretty_size_string(item.file_size);

        let thumbnail = item
            .thumbnail_path
            .as_ref()
            .and_then(|path| load_image_from_path(path).ok());

        UIVideoEditorPlaylistItem {
            file_path: item.file_path.to_string_lossy().to_string().into(),
            name: item.name.clone().into(),
            media_type,
            duration: duration.into(),
            file_size: file_size.into(),
            thumbnail: thumbnail.unwrap_or_default(),
            is_selected: false,
            is_marked: item.is_marked,
            is_folder: false,
            folder_id: item
                .parent_id
                .clone()
                .map(|id| id.into())
                .unwrap_or_default(),
            item_id: item.id.clone().into(),
            folder_source_path: SharedString::default(),
        }
    }
}

impl From<LibraryFolder> for UIVideoEditorPlaylistItem {
    fn from(folder: LibraryFolder) -> Self {
        UIVideoEditorPlaylistItem {
            file_path: SharedString::default(),
            name: folder.name.clone().into(),
            media_type: UIMediaType::Video, // placeholder, not meaningful for folders
            duration: SharedString::default(),
            file_size: SharedString::default(),
            thumbnail: Image::default(),
            is_selected: false,
            is_marked: folder.is_marked,
            is_folder: true,
            folder_id: folder
                .parent_id
                .clone()
                .map(|id| id.into())
                .unwrap_or_default(),
            item_id: folder.id.clone().into(),
            folder_source_path: folder
                .source_path
                .map(|p| p.display().to_string().into())
                .unwrap_or_default(),
        }
    }
}

impl From<UIAudioFormat> for AudioExportFormat {
    fn from(format: UIAudioFormat) -> Self {
        match format {
            UIAudioFormat::Aac => AudioExportFormat::Aac,
            UIAudioFormat::Mp3 => AudioExportFormat::Mp3,
            UIAudioFormat::Ogg => AudioExportFormat::Ogg,
            UIAudioFormat::Wav => AudioExportFormat::Wav,
            UIAudioFormat::Flac => AudioExportFormat::Flac,
        }
    }
}

impl From<UISubtitleType> for SubtitleFormat {
    fn from(ty: UISubtitleType) -> Self {
        match ty {
            UISubtitleType::Srt => SubtitleFormat::Srt,
            UISubtitleType::Vtt => SubtitleFormat::Vtt,
            UISubtitleType::Ass => SubtitleFormat::Ass,
        }
    }
}

impl From<UIResolution> for Option<(u32, u32)> {
    fn from(resolution: UIResolution) -> Self {
        match resolution {
            UIResolution::Original => None,
            UIResolution::P480 => Some((854, 480)),
            UIResolution::P720 => Some((1280, 720)),
            UIResolution::P1080 => Some((1920, 1080)),
            UIResolution::P2K => Some((2560, 1440)),
            UIResolution::P4K => Some((3840, 2160)),
            // Portrait resolutions (9:16)
            UIResolution::Portrait480P => Some((480, 854)),
            UIResolution::Portrait720P => Some((720, 1280)),
            UIResolution::Portrait1080P => Some((1080, 1920)),
            UIResolution::Portrait4K => Some((2160, 3840)),
            // Square resolutions (1:1)
            UIResolution::Square480P => Some((480, 480)),
            UIResolution::Square720P => Some((720, 720)),
            UIResolution::Square1080P => Some((1080, 1080)),
            // Instagram Portrait (4:5)
            UIResolution::InstagramPortrait => Some((1080, 1350)),
        }
    }
}

impl From<(i32, i32)> for UIVideoPreviewSize {
    fn from((width, height): (i32, i32)) -> Self {
        UIVideoPreviewSize { width, height }
    }
}

impl From<FilterType> for UIFilterType {
    fn from(v: FilterType) -> Self {
        match v {
            FilterType::Video => UIFilterType::Video,
            FilterType::Audio => UIFilterType::Audio,
            FilterType::Subtitle => UIFilterType::Subtitle,
            FilterType::Image => UIFilterType::Image,
        }
    }
}

impl From<UIFilterType> for FilterType {
    fn from(v: UIFilterType) -> Self {
        match v {
            UIFilterType::Video => FilterType::Video,
            UIFilterType::Audio => FilterType::Audio,
            UIFilterType::Subtitle => FilterType::Subtitle,
            UIFilterType::Image => FilterType::Image,
        }
    }
}

pub fn layer_frame_to_ui(layer: &LayerFrame) -> Option<UIVideoEditorLayerImage> {
    let (original_buffer, filtered_buffer) = match (&layer.original_image, &layer.image) {
        (VideoImage::Image { buffer: orig }, VideoImage::Image { buffer: filtered }) => {
            (orig, filtered)
        }
        (VideoImage::Empty, VideoImage::Image { buffer }) => (buffer, buffer),
        (VideoImage::Image { buffer }, VideoImage::Empty) => (buffer, buffer),
        (VideoImage::Empty, VideoImage::Empty) => return None,
    };

    let (is_image_track, is_text_track) = with_history_manager(|state| {
        let track = state.tracks_manager.get(layer.track_index);
        let is_image_track = track.map(|t| t.is_image()).unwrap_or(false);
        let is_text_track = track.map(|t| t.is_text()).unwrap_or(false);
        (is_image_track, is_text_track)
    });

    // Text tracks store position/rotation in TextElement, not in TransformFilter.
    // Without this, TransformFilter::default() gives zoom_level=1.0 which stretches
    // the small text original_image to fill the entire preview.
    let transform: UITransformDetail = if is_text_track {
        layer
            .from_segment
            .as_ref()
            .and_then(|(_, segment)| segment.text_element.as_ref())
            .map(|element| {
                let text_w = original_buffer.width() as f32;
                let text_h = original_buffer.height() as f32;
                let output_w = filtered_buffer.width() as f32;
                let output_h = filtered_buffer.height() as f32;
                let output_rate = output_w / output_h;
                let text_rate = text_w / text_h;
                let zoom_level = if output_rate > text_rate {
                    text_h / output_h
                } else {
                    text_w / output_w
                };
                UITransformDetail {
                    zoom_level,
                    center_x_percent: element.position.0,
                    center_y_percent: element.position.1,
                    rotation: element.rotation,
                }
            })
            .unwrap_or_else(|| TransformFilter::default().into())
    } else {
        extract_filter_from_layer!(layer, is_image_track, TransformFilter)
    };
    let crop: UICropDetail = extract_filter_from_layer!(layer, is_image_track, CropFilter);
    let zoom: UIZoomDetail = extract_filter_from_layer!(layer, is_image_track, ZoomFilter);
    let fly_in: UIFlyInDetail = extract_filter_from_layer!(layer, is_image_track, FlyInFilter);
    let mosaic: UIMosaicDetail = extract_filter_from_layer!(layer, is_image_track, MosaicFilter);
    let liquid_glass: UILiquidGlassDetail =
        extract_filter_from_layer!(layer, is_image_track, LiquidGlassFilter);
    let draw_circle: UIDrawCircleDetail =
        extract_filter_from_layer!(layer, is_image_track, DrawCircleFilter);
    let draw_rectangle: UIDrawRectangleDetail =
        extract_filter_from_layer!(layer, is_image_track, DrawRectangleFilter);
    let local_magnify: UILocalMagnifyDetail =
        extract_filter_from_layer!(layer, is_image_track, LocalMagnifyFilter);
    let magnifier: UIMagnifierDetail =
        extract_filter_from_layer!(layer, is_image_track, MagnifierFilter);
    let text_highlight: UITextHighlightDetail =
        extract_filter_from_layer!(layer, is_image_track, TextHighlightFilter);
    let linear_mask: UILinearMaskDetail =
        extract_filter_from_layer!(layer, is_image_track, LinearMaskFilter);
    let circle_mask: UICircleMaskDetail =
        extract_filter_from_layer!(layer, is_image_track, CircleMaskFilter);
    let mirror_mask: UIMirrorMaskDetail =
        extract_filter_from_layer!(layer, is_image_track, MirrorMaskFilter);
    let rectangle_mask: UIRectangleMaskDetail =
        extract_filter_from_layer!(layer, is_image_track, RectangleMaskFilter);
    let focus: UIFocusDetail = extract_filter_from_layer!(layer, is_image_track, FocusFilter);
    let gaussian_blur: UIGaussianBlurDetail =
        extract_filter_from_layer!(layer, is_image_track, GaussianBlurFilter);

    Some(UIVideoEditorLayerImage {
        track_index: layer.track_index as i32,
        segment_index: layer
            .from_segment
            .as_ref()
            .map(|(i, _)| *i as i32)
            .unwrap_or(-1),
        original_image: rgba_image_to_slint_image(original_buffer),
        image: rgba_image_to_slint_image(filtered_buffer),
        transform,
        crop,
        zoom,
        fly_in,
        mosaic,
        liquid_glass,
        draw_circle,
        draw_rectangle,
        local_magnify,
        magnifier,
        text_highlight,
        linear_mask,
        circle_mask,
        mirror_mask,
        rectangle_mask,
        focus,
        gaussian_blur,
    })
}

pub fn load_image_from_path<P: AsRef<Path>>(path: P) -> Result<slint::Image, image::ImageError> {
    let img = image::open(path.as_ref())?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let buffer =
        SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);

    Ok(slint::Image::from_rgba8(buffer))
}

pub fn rgba_image_to_slint_image(rgba: &image::RgbaImage) -> slint::Image {
    let (width, height) = rgba.dimensions();
    let buffer =
        SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(rgba.as_raw(), width, height);
    slint::Image::from_rgba8(buffer)
}

fn duration_to_millis(duration: Duration) -> i32 {
    duration.as_millis() as i32
}

pub fn track_to_filter_type(value: &Track) -> FilterType {
    match value {
        Track::Video(_) => FilterType::Video,
        Track::Audio(_) => FilterType::Audio,
        Track::Subtitle(_) => FilterType::Subtitle,
        Track::Image(_) => FilterType::Image,
        Track::Text(_) => FilterType::Video,
    }
}

fn video_filters_to_ui(filters: &[Arc<VideoFilterWrapper>]) -> Vec<UISegmentFilter> {
    filters
        .iter()
        .map(|wrapper| {
            let detail = video_filter_to_json_detail(&wrapper.inner);
            UISegmentFilter {
                ty: UIFilterType::Video,
                enabled: wrapper.enabled(),
                name: wrapper.inner.name().to_string().into(),
                detail: detail.into(),
            }
        })
        .collect()
}

pub fn video_filter_to_json_detail(filter: &Box<dyn VideoFilter>) -> String {
    filter_to_json_match!(
        filter,
        ChromaKeyFilter,
        FlipFilter,
        CropFilter,
        TransformFilter,
        ZoomFilter,
        DrawCircleFilter,
        DrawRectangleFilter,
        VideoFadeInFilter,
        VideoFadeOutFilter,
        SlideFilter,
        WipeFilter,
        OpacityFilter,
        BorderFilter,
        FlyInFilter,
        MosaicFilter,
        LiquidGlassFilter,
        VignetteFilter,
        HSLAdjustFilter,
        SpeedFilter,
        BackgroundFilter,
        BreathingFilter,
        LocalMagnifyFilter,
        MagnifierFilter,
        GaussianBlurFilter,
        DirectionalBlurFilter,
        SharpenFilter,
        EdgeDetectFilter,
        GrainFilter,
        GridFilter,
        GrayscaleFilter,
        FisheyeFilter,
        FocusFilter,
        OldFilmFilter,
        SketchFilter,
        WaveFilter,
        TextHighlightFilter,
        LinearMaskFilter,
        CircleMaskFilter,
        MirrorMaskFilter,
        RectangleMaskFilter,
        ShadowFilter,
        DeviceFrameFilter,
        GenieFilter,
        PageFlipFilter,
        VELightingFilter,
        SplitFilter,
        FrameExtractFilter,
        Live2dFilter
    );
}

fn audio_filters_to_ui(filters: &[Arc<AudioFilterWrapper>]) -> Vec<UISegmentFilter> {
    filters
        .iter()
        .map(|wrapper| {
            let detail = audio_filter_to_json_detail(&wrapper.inner);
            UISegmentFilter {
                ty: UIFilterType::Audio,
                enabled: wrapper.enabled(),
                name: wrapper.inner.name().to_string().into(),
                detail: detail.into(),
            }
        })
        .collect()
}

pub fn audio_filter_to_json_detail(filter: &Box<dyn AudioFilter>) -> String {
    filter_to_json_match!(
        filter,
        GainFilter,
        NormalizeFilter,
        LimiterFilter,
        NoiseGateFilter,
        CompressorFilter,
        DenoiseFilter,
        MuteFilter,
        CopyChannelFilter,
        AudioFadeInFilter,
        AudioFadeOutFilter,
        VoiceChangerFilter,
        AudioSpeedFilter
    );
}

fn subtitle_filters_to_ui(filters: &[Arc<SubtitleFilterWrapper>]) -> Vec<UISegmentFilter> {
    filters
        .iter()
        .map(|wrapper| {
            let detail = subtitle_filter_to_json_detail(&wrapper.inner);
            UISegmentFilter {
                ty: UIFilterType::Subtitle,
                enabled: wrapper.enabled(),
                name: wrapper.inner.name().to_string().into(),
                detail: detail.into(),
            }
        })
        .collect()
}

pub fn subtitle_filter_to_json_detail(filter: &Box<dyn SubtitleFilter>) -> String {
    filter_to_json_match!(
        filter,
        PrimaryColorFilter,
        OutlineColorFilter,
        BackgroundColorFilter,
        FontSizeFilter,
        FontPathFilter,
        AlignmentFilter,
        OutlineWidthFilter,
        BorderRadiusFilter,
        PaddingFilter,
        MarginVerticalFilter,
        MarginHorizontalFilter
    );
}

fn image_filters_to_ui(filters: &[Arc<ImageFilterWrapper>]) -> Vec<UISegmentFilter> {
    filters
        .iter()
        .map(|wrapper| {
            let detail = image_filter_to_json_detail(&wrapper.inner);
            UISegmentFilter {
                ty: UIFilterType::Image,
                enabled: wrapper.enabled(),
                name: wrapper.inner.name().to_string().into(),
                detail: detail.into(),
            }
        })
        .collect()
}

pub fn image_filter_to_json_detail(filter: &Box<dyn VideoFilter>) -> String {
    video_filter_to_json_detail(filter)
}

impl From<TextElement> for UITextElement {
    fn from(element: TextElement) -> Self {
        let primary_color = element
            .style
            .primary_color
            .unwrap_or(image::Rgba([255, 255, 255, 255]));
        let outline_color = element
            .style
            .outline_color
            .unwrap_or(image::Rgba([0, 0, 0, 255]));
        let background_color = element
            .style
            .background_color
            .unwrap_or(image::Rgba([0, 0, 0, 0]));
        let border_color = element
            .style
            .border_color
            .unwrap_or(image::Rgba([0, 0, 0, 0]));
        let alignment = match element.style.text_alignment {
            TextAlignment::Center => 1,
            TextAlignment::Left => 0,
            TextAlignment::Right => 2,
        };

        UITextElement {
            text: element.text.replace("\\N", "\n").into(),
            position_x: element.position.0,
            position_y: element.position.1,
            opacity: element.opacity,
            rotation: element.rotation,
            font_path: element.style.font_path.to_string_lossy().to_string().into(),
            font_family: element.style.font_family.clone().into(),
            font_style: element.style.font_style.clone().into(),
            font_size: element.style.font_size as i32,
            primary_color_r: primary_color[0] as i32,
            primary_color_g: primary_color[1] as i32,
            primary_color_b: primary_color[2] as i32,
            primary_color_a: primary_color[3] as i32,
            outline_width: element.style.outline_width.unwrap_or(2) as i32,
            outline_color_r: outline_color[0] as i32,
            outline_color_g: outline_color[1] as i32,
            outline_color_b: outline_color[2] as i32,
            outline_color_a: outline_color[3] as i32,
            background_color_r: background_color[0] as i32,
            background_color_g: background_color[1] as i32,
            background_color_b: background_color[2] as i32,
            background_color_a: background_color[3] as i32,
            border_radius: element.style.border_radius.unwrap_or(0) as i32,
            padding: element.style.padding.unwrap_or(4) as i32,
            border_width: element.style.border_width.unwrap_or(0) as i32,
            border_color_r: border_color[0] as i32,
            border_color_g: border_color[1] as i32,
            border_color_b: border_color[2] as i32,
            border_color_a: border_color[3] as i32,
            alignment,
        }
    }
}

impl From<UITextElement> for TextElement {
    fn from(ui_element: UITextElement) -> Self {
        let primary_color = if ui_element.primary_color_a > 0 {
            Some(image::Rgba([
                ui_element.primary_color_r as u8,
                ui_element.primary_color_g as u8,
                ui_element.primary_color_b as u8,
                ui_element.primary_color_a as u8,
            ]))
        } else {
            None
        };

        let outline_color = if ui_element.outline_color_a > 0 {
            Some(image::Rgba([
                ui_element.outline_color_r as u8,
                ui_element.outline_color_g as u8,
                ui_element.outline_color_b as u8,
                ui_element.outline_color_a as u8,
            ]))
        } else {
            None
        };

        let background_color = if ui_element.background_color_a > 0 {
            Some(image::Rgba([
                ui_element.background_color_r as u8,
                ui_element.background_color_g as u8,
                ui_element.background_color_b as u8,
                ui_element.background_color_a as u8,
            ]))
        } else {
            None
        };

        let border_color = if ui_element.border_color_a > 0 {
            Some(image::Rgba([
                ui_element.border_color_r as u8,
                ui_element.border_color_g as u8,
                ui_element.border_color_b as u8,
                ui_element.border_color_a as u8,
            ]))
        } else {
            None
        };

        let style = SubtitleStyle {
            font_path: PathBuf::from(ui_element.font_path.as_str()),
            font_family: ui_element.font_family.to_string(),
            font_style: ui_element.font_style.to_string(),
            font_size: ui_element.font_size as u32,
            primary_color,
            outline_color,
            outline_width: if ui_element.outline_width > 0 {
                Some(ui_element.outline_width as u32)
            } else {
                None
            },
            background_color,
            border_radius: if ui_element.border_radius > 0 {
                Some(ui_element.border_radius as u32)
            } else {
                None
            },
            padding: if ui_element.padding > 0 {
                Some(ui_element.padding as u32)
            } else {
                None
            },
            border_width: if ui_element.border_width > 0 {
                Some(ui_element.border_width as u32)
            } else {
                None
            },
            border_color,
            text_alignment: match ui_element.alignment {
                0 => TextAlignment::Left,
                1 => TextAlignment::Center,
                2 => TextAlignment::Right,
                _ => TextAlignment::Center,
            },
            ..Default::default()
        };

        TextElement {
            text: ui_element.text.replace('\n', "\\N").to_string(),
            position: (ui_element.position_x, ui_element.position_y),
            opacity: ui_element.opacity,
            rotation: ui_element.rotation,
            style,
            keyframe_tracks: Default::default(),
        }
    }
}

impl From<&UITextElement> for TextStyleConfig {
    fn from(element: &UITextElement) -> Self {
        Self {
            id: TEXT_STYLE_CONFIG_ID.to_string(),
            font_path: element.font_path.to_string(),
            font_family: element.font_family.to_string(),
            font_style: element.font_style.to_string(),
            font_size: element.font_size,
            primary_color_r: element.primary_color_r,
            primary_color_g: element.primary_color_g,
            primary_color_b: element.primary_color_b,
            primary_color_a: element.primary_color_a,
            outline_width: element.outline_width,
            outline_color_r: element.outline_color_r,
            outline_color_g: element.outline_color_g,
            outline_color_b: element.outline_color_b,
            outline_color_a: element.outline_color_a,
            background_color_r: element.background_color_r,
            background_color_g: element.background_color_g,
            background_color_b: element.background_color_b,
            background_color_a: element.background_color_a,
            border_radius: element.border_radius,
            padding: element.padding,
            border_width: element.border_width,
            border_color_r: element.border_color_r,
            border_color_g: element.border_color_g,
            border_color_b: element.border_color_b,
            border_color_a: element.border_color_a,
            alignment: element.alignment,
        }
    }
}

impl From<&TextStyleConfig> for UITextElement {
    fn from(config: &TextStyleConfig) -> Self {
        UITextElement {
            text: SharedString::new(),
            position_x: 0.5,
            position_y: 0.5,
            opacity: 1.0,
            rotation: 0.0,
            font_path: config.font_path.clone().into(),
            font_family: config.font_family.clone().into(),
            font_style: config.font_style.clone().into(),
            font_size: config.font_size,
            primary_color_r: config.primary_color_r,
            primary_color_g: config.primary_color_g,
            primary_color_b: config.primary_color_b,
            primary_color_a: config.primary_color_a,
            outline_width: config.outline_width,
            outline_color_r: config.outline_color_r,
            outline_color_g: config.outline_color_g,
            outline_color_b: config.outline_color_b,
            outline_color_a: config.outline_color_a,
            background_color_r: config.background_color_r,
            background_color_g: config.background_color_g,
            background_color_b: config.background_color_b,
            background_color_a: config.background_color_a,
            border_radius: config.border_radius,
            padding: config.padding,
            border_width: config.border_width,
            border_color_r: config.border_color_r,
            border_color_g: config.border_color_g,
            border_color_b: config.border_color_b,
            border_color_a: config.border_color_a,
            alignment: config.alignment,
        }
    }
}

impl From<UIPresetTextStyle> for PresetTextStyleData {
    fn from(ui: UIPresetTextStyle) -> Self {
        Self {
            name: ui.name.to_string(),
            style: TextStyleConfig::from(&ui.style),
        }
    }
}

impl From<PresetTextStyleData> for UIPresetTextStyle {
    fn from(data: PresetTextStyleData) -> Self {
        UIPresetTextStyle {
            name: data.name.into(),
            style: UITextElement::from(&data.style),
        }
    }
}
