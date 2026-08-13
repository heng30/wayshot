use super::project::UI_STATE_ID;
use crate::slint_generatedAppWindow::{
    AudioChannels as UIAudioChannels, AudioFormat as UIAudioFormat,
    AudioSampleRate as UIAudioSampleRate, Fps as UIFps, McpTransport as UIMcpTransport,
    MediaType as UIMediaType, PresetSubtitleStyle as UIPresetSubtitleStyle,
    Resolution as UIResolution, SubtitleStyle as UISubtitleStyle, SubtitleType as UISubtitleType,
    VideoEditorExportAudioConfig as UIVideoEditorExportAudioConfig,
    VideoEditorExportVideoConfig as UIVideoEditorExportVideoConfig,
    VideoEditorPreferenceCacheConfig as UIVideoEditorPreferenceCacheConfig,
    VideoEditorPreferenceConfig as UIVideoEditorPreferenceConfig,
    VideoEditorPreferenceMcpConfig as UIVideoEditorPreferenceMcpConfig,
    VideoEditorPreferenceTrackConfig as UIVideoEditorPreferenceTrackConfig,
    VideoEditorPreviewConfig as UIVideoEditorPreviewConfig,
    VideoEditorRecordAudioConfig as UIVideoEditorRecordAudioConfig,
    VideoEditorTrackType as UIVideoEditorTrackType, VideoEditorUIState as UIVideoEditorUIState,
};
use derivative::Derivative;
use pmacro::SlintFromConvert;
use serde::{Deserialize, Serialize};

crate::impl_slint_enum_serde!(UISubtitleType, Srt, Vtt, Ass);
crate::impl_slint_enum_serde!(UIAudioFormat, Aac, Mp3, Ogg, Wav, Flac);
crate::impl_slint_enum_serde!(UIMediaType, Video, Audio, Image, Subtitle, Text);
crate::impl_slint_enum_serde!(UIVideoEditorTrackType, Audio, Video, Subtitle, Image, Text);
crate::impl_slint_enum_serde!(UIAudioChannels, Mono, Stereo);
crate::impl_slint_enum_serde!(
    UIAudioSampleRate,
    Hz8000,
    Hz16000,
    Hz24000,
    Hz32000,
    Hz44100,
    Hz48000,
    Hz96000,
    Hz192000
);

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorUIState")]
#[serde(default)]
pub struct VideoEditorUIState {
    #[derivative(Default(value = "UI_STATE_ID.to_string()"))]
    pub id: String,

    #[derivative(Default(value = "true"))]
    pub enabled_link_track: bool,

    pub enabled_link_all_tracks: bool,

    #[derivative(Default(value = "\"0.25\".to_string()"))]
    pub tracks_height: String,

    #[derivative(Default(value = "\"0.2\".to_string()"))]
    pub left_panel_width: String,

    #[derivative(Default(value = "\"0.2\".to_string()"))]
    pub right_panel_width: String,

    #[derivative(Default(value = "\"1\".to_string()"))]
    pub tracks_zoom_level: String,

    #[derivative(Default(value = "80.0"))]
    pub preview_volume: f32,

    /// 预览窗口网格大小：0 表示关闭，其他值如 3 表示 3x3 网格
    pub preview_grid_size: i32,

    /// 播放列表视图模式：0 = 列表模式, 1 = 缩略图模式
    pub playlist_view_mode: i32,

    /// 媒体库的视图模式：0 = 列表模式, 1 = 缩略图模式
    pub library_view_mode: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorPreviewConfig")]
pub struct VideoEditorPreviewConfig {
    #[derivative(Default(value = "UIFps::Fps25"))]
    pub fps: UIFps,

    #[derivative(Default(value = "UIResolution::P1080"))]
    pub resolution: UIResolution,

    #[derivative(Default(value = "UIAudioChannels::Stereo"))]
    pub channels: UIAudioChannels,

    #[derivative(Default(value = "UIAudioSampleRate::Hz44100"))]
    pub sample_rate: UIAudioSampleRate,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorPreferenceTrackConfig")]
#[serde(default)]
pub struct VideoEditorPreferenceTrackConfig {
    #[derivative(Default(value = "true"))]
    pub show_thumbnail: bool,

    pub show_filename: bool,
    pub compact_mode: bool,

    #[derivative(Default(value = "1.0"))]
    pub audio_track_waveform_amplification: f32,

    #[derivative(Default(value = "100"))]
    pub snap_threshold_ms: i32,

    #[serde(default)]
    #[derivative(Default(value = "25"))]
    pub waveform_samples_per_second: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorPreferenceCacheConfig")]
pub struct VideoEditorPreferenceCacheConfig {
    #[derivative(Default(value = "100"))]
    pub max_frames: i32,

    #[derivative(Default(value = "5"))]
    pub max_cache_duration: i32,
}

crate::impl_slint_enum_serde!(UIMcpTransport, Stdio, Http, Both);

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorPreferenceMcpConfig")]
pub struct VideoEditorPreferenceMcpConfig {
    pub enabled: bool,
    pub transport: UIMcpTransport,

    #[derivative(Default(value = "9527"))]
    pub port: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorPreferenceConfig")]
#[serde(default)]
pub struct VideoEditorPreferenceConfig {
    pub track: VideoEditorPreferenceTrackConfig,
    pub cache: VideoEditorPreferenceCacheConfig,
    #[serde(default)]
    pub mcp: VideoEditorPreferenceMcpConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorExportVideoConfig")]
#[serde(default)]
pub struct VideoEditorExportVideoConfig {
    pub fps: UIFps,
    pub resolution: UIResolution,
    pub channels: UIAudioChannels,
    pub sample_rate: UIAudioSampleRate,
    pub low_memory_mode: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorExportAudioConfig")]
#[serde(default)]
pub struct VideoEditorExportAudioConfig {
    pub format: UIAudioFormat,
    pub channels: UIAudioChannels,
    pub sample_rate: UIAudioSampleRate,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIVideoEditorRecordAudioConfig")]
#[serde(default)]
pub struct VideoEditorRecordAudioConfig {
    pub save_dir: String,
    pub device: String,
    #[derivative(Default(value = "1.0"))]
    pub gain: f32,
    pub mono: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SegmentFilterData {
    pub enabled: bool,
    pub name: String,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PresetFiltersConfig {
    pub video: Vec<PresetFilter>,
    pub audio: Vec<PresetFilter>,
    pub subtitle: Vec<PresetFilter>,
    pub image: Vec<PresetFilter>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresetFilter {
    pub filter_type: String,
    pub name: String,
    pub filters: String, // JSON serialized Vec<SegmentFilterData>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MarkedFiltersConfig {
    pub video: Vec<String>,
    pub audio: Vec<String>,
    pub subtitle: Vec<String>,
    pub image: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct SubtitleStyleConfig {
    pub font_path: String,
    pub font_family: String,
    pub font_style: String,

    #[derivative(Default(value = "20"))]
    pub font_size: i32,

    #[derivative(Default(value = "4"))]
    pub padding: i32,

    #[derivative(Default(value = "30"))]
    pub margin_vertical: i32,

    pub margin_horizontal: i32,

    #[derivative(Default(value = "2"))]
    pub outline_width: i32,
    pub border_radius: i32,

    #[derivative(Default(value = "255"))]
    pub primary_color_r: i32,
    #[derivative(Default(value = "255"))]
    pub primary_color_g: i32,
    #[derivative(Default(value = "255"))]
    pub primary_color_b: i32,
    #[derivative(Default(value = "255"))]
    pub primary_color_a: i32,

    #[derivative(Default(value = "0"))]
    pub outline_color_r: i32,
    #[derivative(Default(value = "0"))]
    pub outline_color_g: i32,
    #[derivative(Default(value = "0"))]
    pub outline_color_b: i32,
    #[derivative(Default(value = "255"))]
    pub outline_color_a: i32,

    #[derivative(Default(value = "0"))]
    pub background_color_r: i32,
    #[derivative(Default(value = "0"))]
    pub background_color_g: i32,
    #[derivative(Default(value = "0"))]
    pub background_color_b: i32,
    #[derivative(Default(value = "0"))]
    pub background_color_a: i32,

    // 0=Left, 1=Center, 2=Right, default Center
    #[derivative(Default(value = "1"))]
    pub text_alignment: i32,
}

impl From<UISubtitleStyle> for SubtitleStyleConfig {
    fn from(ui: UISubtitleStyle) -> Self {
        Self {
            font_path: ui.font_path.font_path.into(),
            font_family: ui.font_path.font_family.into(),
            font_style: ui.font_path.font_style.into(),
            font_size: ui.font_size.font_size,
            padding: ui.padding.padding,
            margin_vertical: ui.margin_vertical.margin,
            margin_horizontal: ui.margin_horizontal.margin,
            outline_width: ui.outline_width.width,
            border_radius: ui.border_radius.radius,
            primary_color_r: ui.primary_color.r,
            primary_color_g: ui.primary_color.g,
            primary_color_b: ui.primary_color.b,
            primary_color_a: ui.primary_color.a,
            outline_color_r: ui.outline_color.r,
            outline_color_g: ui.outline_color.g,
            outline_color_b: ui.outline_color.b,
            outline_color_a: ui.outline_color.a,
            background_color_r: ui.background_color.r,
            background_color_g: ui.background_color.g,
            background_color_b: ui.background_color.b,
            background_color_a: ui.background_color.a,
            text_alignment: ui.text_alignment.alignment,
        }
    }
}

impl From<SubtitleStyleConfig> for UISubtitleStyle {
    fn from(config: SubtitleStyleConfig) -> Self {
        use crate::slint_generatedAppWindow::{
            BackgroundColorDetail, BorderRadiusDetail, FontPathDetail, FontSizeDetail,
            MarginHorizontalDetail, MarginVerticalDetail, OutlineColorDetail, OutlineWidthDetail,
            PaddingDetail, PrimaryColorDetail, TextAlignmentDetail,
        };
        Self {
            font_path: FontPathDetail {
                font_path: config.font_path.into(),
                font_family: config.font_family.into(),
                font_style: config.font_style.into(),
            },
            font_size: FontSizeDetail {
                font_size: config.font_size,
            },
            padding: PaddingDetail {
                padding: config.padding,
            },
            margin_vertical: MarginVerticalDetail {
                margin: config.margin_vertical,
            },
            margin_horizontal: MarginHorizontalDetail {
                margin: config.margin_horizontal,
            },
            outline_width: OutlineWidthDetail {
                width: config.outline_width,
            },
            border_radius: BorderRadiusDetail {
                radius: config.border_radius,
            },
            primary_color: PrimaryColorDetail {
                r: config.primary_color_r,
                g: config.primary_color_g,
                b: config.primary_color_b,
                a: config.primary_color_a,
            },
            outline_color: OutlineColorDetail {
                r: config.outline_color_r,
                g: config.outline_color_g,
                b: config.outline_color_b,
                a: config.outline_color_a,
            },
            background_color: BackgroundColorDetail {
                r: config.background_color_r,
                g: config.background_color_g,
                b: config.background_color_b,
                a: config.background_color_a,
            },
            text_alignment: TextAlignmentDetail {
                alignment: config.text_alignment,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PresetSubtitleStyleConfig {
    pub styles: Vec<PresetSubtitleStyleData>,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[from("UIPresetSubtitleStyle")]
pub struct PresetSubtitleStyleData {
    pub name: String,
    pub style: SubtitleStyleConfig,
}
