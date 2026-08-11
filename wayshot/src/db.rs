use crate::logic::{BG_ANIMATION_CONFIG_ID, CODE_IMAGE_CONFIG_ID, TEXT_STYLE_CONFIG_ID};
use crate::slint_generatedAppWindow::{
    AnimationType, ArrowAnimConfig as UIArrowAnimConfig,
    BackgroundAnimationConfig as UIBackgroundAnimationConfig,
    BlackHoleAnimConfig as UIBlackHoleAnimConfig, BokehAnimConfig as UIBokehAnimConfig,
    CodeImageConfig as UICodeImageConfig, CrossLineAnimConfig as UICrossLineAnimConfig,
    DanmakuDistributionMode as UIDanmakuDistributionMode,
    DanmakuGlobalFilterConfig as UIDanmakuGlobalFilterConfig, DanmakuItem as UIDanmakuItem,
    DanmakuSegment as UIDanmakuSegment, DanmakuStyle as UIDanmakuStyle, DashStyle as UIDashStyle,
    FlowFieldAnimConfig as UIFlowFieldAnimConfig, FluidAnimConfig as UIFluidAnimConfig,
    FontEntry as UIFontEntry, FontSource as UIFontSource, GalaxyAnimConfig as UIGalaxyAnimConfig,
    GlitchAnimConfig as UIGlitchAnimConfig, GlobalFilterConfig as UIGlobalFilterConfig,
    GlobalFilterType as UIGlobalFilterType, GlobalSpeedFilterConfig as UIGlobalSpeedFilterConfig,
    GradeMarkAnimConfig as UIGradeMarkAnimConfig, GradeMarkType as UIGradeMarkType,
    GridAnimConfig as UIGridAnimConfig, HistoryEntry as UIHistoryEntry,
    ImageAnimationConfig as UIImageAnimationConfig,
    ImageScrollAnimConfig as UIImageScrollAnimConfig,
    InkDissipationAnimConfig as UIInkDissipationAnimConfig, InkStyle as UIInkStyle,
    KaleidoscopeAnimConfig as UIKaleidoscopeAnimConfig,
    LightEffectsAnimConfig as UILightEffectsAnimConfig,
    MatrixRainAnimConfig as UIMatrixRainAnimConfig, MovingGridAnimConfig as UIMovingGridAnimConfig,
    NoiseFlowAnimConfig as UINoiseFlowConfig, OcrTaskMode as UIOcrTaskMode,
    OnlineSearchAudioSetting as UIOnlineSearchAudioSetting,
    OnlineSearchAudioSourceEntry as UIOnlineSearchAudioSourceEntry,
    OnlineSearchImageSetting as UIOnlineSearchImageSetting,
    OnlineSearchImageSourceEntry as UIOnlineSearchImageSourceEntry,
    ParticleLifeAnimConfig as UIParticleLifeAnimConfig,
    ParticleNetworkAnimConfig as UIParticleNetworkAnimConfig,
    ProgressBarGlobalFilterConfig as UIProgressBarGlobalFilterConfig,
    ProgressBarItem as UIProgressBarItem, PureColorImageConfig as UIPureColorImageConfig,
    RectDrawAnimConfig as UIRectDrawAnimConfig,
    RotationGlobalFilterConfig as UIRotationGlobalFilterConfig,
    SceneDetectConfig as UISceneDetectConfig, SceneDetectorAlgorithm as UISceneDetectorAlgorithm,
    SettingPlayer as UISettingPlayer, SettingTranscribe as UISettingTranscribe,
    ShapeAnimConfig as UIShapeAnimConfig, SmartMixSetting as UISmartMixSetting,
    Subtitle as UISubtitle, SubtitleTranslateConfig as UISubtitleTranslateConfig,
    TTSConfig as UITTSConfig, TimerGlobalFilterConfig as UITimerGlobalFilterConfig,
    TimerItem as UITimerItem, TimerMode as UITimerMode, TimerStyle as UITimerStyle,
    TriangleAnimConfig as UITriangleAnimConfig, VideoEditorBgRemoverConfig as UIBgRemoverConfig,
    VideoEditorClearVisionConfig as UIClearVisionConfig, VideoEditorCutoutConfig as UICutoutConfig,
    VideoEditorDedupPhotosConfig as UIDedupPhotosConfig,
    VideoEditorDedupPhotosItem as UIDedupPhotosItem,
    VideoEditorDeepFilterConfig as UIDeepFilterConfig,
    VideoEditorDewatermarkConfig as UIDewatermarkConfig,
    VideoEditorMusicGenConfig as UIMusicGenConfig, VideoEditorOCRConfig as UIOcrConfig,
    VideoEditorSimilarVideoSegmentConfig as UISimilarVideoSegmentConfig,
    VideoEditorSimilarVideoSegmentItem as UISimilarVideoSegmentItem,
    VideoEditorSpeakersConfig as UISpeakersConfig,
    VideoEditorStemSplitterConfig as UIStemSplitterConfig,
    VideoEditorSubtitleRemoverConfig as UISubtitleRemoverConfig,
    WaveAnimConfig as UIWaveAnimConfig,
};
use background_animation::{
    AnimationBaseConfig,
    black_hole::BlackHoleConfig,
    bokeh::BokehConfig,
    cross_line::CrossLineConfig,
    flow_field::FlowFieldConfig,
    fluid::FluidConfig,
    galaxy::GalaxyConfig,
    glitch::GlitchConfig,
    grid::GridConfig,
    ink::{InkDissipationConfig, InkStyle},
    kaleidoscope::KaleidoscopeConfig,
    light_effects::LightEffectsConfig,
    matrix_rain::MatrixRainConfig,
    moving_grid::MovingGridConfig,
    noise_flow::NoiseFlowConfig,
    particle_life::ParticleLifeConfig,
    particle_network::ParticleNetworkConfig,
    shape::ShapeConfig,
    triangle::TriangleConfig,
    wave::WaveConfig,
};
use pmacro::SlintFromConvert;
use serde::{Deserialize, Serialize};
use slint::Model;
use std::{path::PathBuf, time::Duration};
use video_editor::filters::global::{
    DanmakuDistributionMode, DanmakuFilter, DanmakuItem, DanmakuSegment, DanmakuStyle,
    GlobalSpeedFilter, ProgressBarFilter, RotationGlobalFilter, TimerFilter, TimerMode,
    TimerSegment,
};

pub const HISTORY_TABLE: &str = "history";
pub const PLAYER_SETTING_TABLE: &str = "player_setting";
pub const VIDEO_EDITOR_TABLE: &str = "video_editor";
pub const FONT_TABLE: &str = "font";

pub async fn init(db_path: &str) {
    sqldb::create_db(db_path).await.expect("create db");

    sqldb::entry::new(HISTORY_TABLE)
        .await
        .expect("history table failed");

    sqldb::entry::new(PLAYER_SETTING_TABLE)
        .await
        .expect("player setting table failed");

    sqldb::entry::new(VIDEO_EDITOR_TABLE)
        .await
        .expect("video editor table failed");

    sqldb::entry::new(FONT_TABLE)
        .await
        .expect("font table failed");
}

#[macro_export]
macro_rules! db_add {
    ($table:expr, $ty:ident) => {
        fn db_add(ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>, entry: $ty) {
            tokio::spawn(async move {
                let data = serde_json::to_string(&entry).expect("Not implement `Serialize` trait");
                if let Err(e) = sqldb::entry::insert($table, entry.id.as_str(), &data).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("insert entry failed")),
                    );
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_update {
    ($table:expr, $ty:ident) => {
        fn db_update(ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>, entry: $ty) {
            tokio::spawn(async move {
                let data = serde_json::to_string(&entry).expect("Not implement `Serialize` trait");
                if let Err(e) = sqldb::entry::update($table, entry.id.as_str(), &data).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
                    );
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_select_all {
    ($table:expr, $ty:ident) => {{
        match sqldb::entry::select_all($table).await {
            Ok(items) => items
                .into_iter()
                .filter_map(|item| serde_json::from_str::<$ty>(&item.data).ok())
                .collect(),
            Err(e) => {
                log::warn!("{:?}", e);
                vec![]
            }
        }
    }};
}

#[macro_export]
macro_rules! db_select {
    ($table:expr, $ty:ident) => {
        fn db_select<F>(
            ui: slint::Weak<$crate::slint_generatedAppWindow::AppWindow>,
            id: impl ToString,
            show_err_toast: bool,
            callback: F,
        ) where
            F: FnOnce(&$crate::slint_generatedAppWindow::AppWindow, $ty) + Send + 'static,
        {
            let id = id.to_string();
            tokio::spawn(async move {
                match sqldb::entry::select($table, id.as_str()).await {
                    Ok(item) => match serde_json::from_str::<$ty>(&item.data) {
                        Ok(data) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui.upgrade() {
                                    callback(&ui, data);
                                }
                            });
                        }
                        Err(e) => {
                            $crate::logic::toast::async_toast_warn(
                                ui,
                                format!("{}. {e}", crate::logic::tr::tr("parse entry failed")),
                            );
                        }
                    },
                    Err(e) => {
                        if show_err_toast {
                            $crate::logic::toast::async_toast_warn(
                                ui,
                                format!("{}. {e}", crate::logic::tr::tr("load entry failed")),
                            );
                        }
                    }
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_remove {
    ($table:expr) => {
        fn db_remove(
            ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>,
            id: impl ToString,
        ) {
            let id = id.to_string();
            tokio::spawn(async move {
                if let Err(e) = sqldb::entry::delete($table, id.as_str()).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("remove entry failed")),
                    );
                }
            });
        }
    };
}

#[macro_export]
macro_rules! db_remove_all {
    ($table:expr) => {
        fn db_remove_all(ui: slint::Weak<crate::slint_generatedAppWindow::AppWindow>) {
            tokio::spawn(async move {
                if let Err(e) = sqldb::entry::delete_all($table).await {
                    crate::logic::toast::async_toast_warn(
                        ui,
                        format!("{}. {e}", crate::logic::tr::tr("remove all entry failed")),
                    );
                }
            });
        }
    };
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UIHistoryEntry")]
pub struct HistoryEntry {
    pub id: String,
    pub file: String,
    pub size: String,
    pub duration: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[from("UISettingPlayer")]
pub struct SettingPlayer {
    pub id: String,
    pub current_time: String,
    pub end_time: String,
    pub sound: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[serde(default)]
#[from("UISubtitle")]
pub struct Subtitle {
    pub start_timestamp: String,
    pub end_timestamp: String,
    pub original_text: String,
    pub correction_text: String,
    pub audio_wave_amplitude: f32,
    pub is_timestamp_overlap: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[serde(default)]
#[from("UIFontEntry")]
pub struct FontEntry {
    pub id: String,
    pub family: String,
    pub path: String,
    pub style: String,
    pub marked: bool,
    pub source: FontSource,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub enum FontSource {
    #[default]
    System,
    Imported,
}

crate::impl_c_like_enum_convert!(UIFontSource, FontSource, System, Imported);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TextStyleConfig {
    pub id: String,
    pub font_path: String,
    pub font_family: String,
    pub font_style: String,
    pub font_size: i32,
    pub primary_color_r: i32,
    pub primary_color_g: i32,
    pub primary_color_b: i32,
    pub primary_color_a: i32,
    pub outline_width: i32,
    pub outline_color_r: i32,
    pub outline_color_g: i32,
    pub outline_color_b: i32,
    pub outline_color_a: i32,
    pub background_color_r: i32,
    pub background_color_g: i32,
    pub background_color_b: i32,
    pub background_color_a: i32,
    pub border_radius: i32,
    pub padding: i32,
    pub border_width: i32,
    pub border_color_r: i32,
    pub border_color_g: i32,
    pub border_color_b: i32,
    pub border_color_a: i32,
    #[serde(default = "default_alignment")]
    pub alignment: i32, // 0=Left, 1=Center, 2=Right
}

impl Default for TextStyleConfig {
    fn default() -> Self {
        Self {
            id: TEXT_STYLE_CONFIG_ID.to_string(),
            font_path: String::new(),
            font_family: String::new(),
            font_style: String::new(),
            font_size: 20,
            primary_color_r: 255,
            primary_color_g: 255,
            primary_color_b: 255,
            primary_color_a: 255,
            outline_width: 2,
            outline_color_r: 0,
            outline_color_g: 0,
            outline_color_b: 0,
            outline_color_a: 255,
            background_color_r: 0,
            background_color_g: 0,
            background_color_b: 0,
            background_color_a: 0,
            border_radius: 0,
            padding: 4,
            border_width: 0,
            border_color_r: 0,
            border_color_g: 0,
            border_color_b: 0,
            border_color_a: 0,
            alignment: 1,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PresetTextStyleConfig {
    pub styles: Vec<PresetTextStyleData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresetTextStyleData {
    pub name: String,
    pub style: TextStyleConfig,
}

fn default_alignment() -> i32 {
    1
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UICodeImageConfig")]
pub struct CodeImageConfigData {
    pub id: String,
    pub language: String,
    pub syntax_theme: String,
    pub font_size: f32,
    pub ascii_font_family: String,
    pub ascii_font_path: String,
    pub ascii_font_style: String,
    pub non_ascii_font_family: String,
    pub non_ascii_font_path: String,
    pub non_ascii_font_style: String,
    pub line_height_ratio: f32,
    pub padding: f32,
    pub scale: f32,
    pub line_numbers: bool,
    pub bg_color: String,
    pub enable_terminal: bool,
    pub terminal_style: String,
    pub terminal_title: String,
}

impl Default for CodeImageConfigData {
    fn default() -> Self {
        Self {
            id: CODE_IMAGE_CONFIG_ID.to_string(),
            language: "Rust".to_string(),
            syntax_theme: "InspiredGitHub".to_string(),
            font_size: 14.0,
            ascii_font_family: String::new(),
            ascii_font_path: String::new(),
            ascii_font_style: String::new(),
            non_ascii_font_family: String::new(),
            non_ascii_font_path: String::new(),
            non_ascii_font_style: String::new(),
            line_height_ratio: 1.5,
            padding: 10.0,
            scale: 2.0,
            line_numbers: true,
            bg_color: String::new(),
            enable_terminal: false,
            terminal_style: "MacOS".to_string(),
            terminal_title: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIPureColorImageConfig")]
pub struct PureColorImageConfigData {
    pub id: String,
    #[derivative(Default(value = "1920"))]
    pub width: i32,
    #[derivative(Default(value = "1080"))]
    pub height: i32,
    pub save_dir: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UITTSConfig")]
pub struct TTSConfigData {
    pub id: String,
    pub use_gpu: bool,
    pub reference_audio_path: String,
    pub save_dir: String,
    pub model_dir: String,
    #[derivative(Default(value = "200"))]
    pub max_char_count: i32,
    #[derivative(Default(value = "400"))]
    pub max_token_count: i32,
    #[derivative(Default(value = "2.0"))]
    pub cfg_value: f32,
    #[derivative(Default(value = "4"))]
    pub context_reset_interval: i32,
    #[derivative(Default(value = "\"飞流直下三千尺 疑是银河落九天\".to_string()"))]
    pub preamble: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UISettingTranscribe")]
pub struct TranscribeConfigData {
    pub model_path: String,
    pub model_tokenizer_path: String,

    #[derivative(Default(value = "100"))]
    pub keep_leading_silence_ms: i32,

    #[derivative(Default(value = "0.5"))]
    pub audio_sound: f32,

    #[derivative(Default(value = "1.0"))]
    pub audio_speed: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UISmartMixSetting")]
pub struct SmartMixConfigData {
    pub vl_fp32_model_dir: String,
    pub vl_fp16_model_dir: String,
    pub vl_q4_model_dir: String,
    pub vl_q8_model_dir: String,
    #[derivative(Default(value = "\"fp16\".to_string()"))]
    pub vl_precision: String,
    #[derivative(Default(value = "\"Origin\".to_string()"))]
    pub vl_resolution: String,
    #[derivative(Default(value = "\"用中文简要描述这张图片的内容。\".to_string()"))]
    pub vl_prompt: String,
    #[derivative(Default(value = "512"))]
    pub vl_max_tokens: i32,
    #[derivative(Default(value = "10"))]
    pub video_sample_fps: i32,
    #[derivative(Default(value = "true"))]
    pub reuse_media: bool,
    #[derivative(Default(value = "false"))]
    pub sequential_match: bool,
    #[derivative(Default(value = "false"))]
    pub must_cover_all_audio: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[serde(default)]
#[from("UIBgRemoverConfig")]
pub struct BgRemoverConfigData {
    pub id: String,
    pub selected_model_index: i32,
    pub export_dir: String,
    pub modnet_path: String,
    pub rmbg_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIMusicGenConfig")]
pub struct MusicGenConfigData {
    pub id: String,
    pub selected_model_index: i32,
    pub export_dir: String,
    pub small_model_dir: String,
    pub small_fp16_model_dir: String,
    pub small_quant_model_dir: String,
    pub medium_model_dir: String,
    pub medium_fp16_model_dir: String,
    pub medium_quant_model_dir: String,
    pub large_model_dir: String,
    #[derivative(Default(value = "10"))]
    pub duration: i32,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIOcrConfig")]
pub struct OcrConfigData {
    pub id: String,
    pub model_dir: String,
    pub task_mode: UIOcrTaskMode,
    #[derivative(Default(value = "300"))]
    pub timeout: i32,
}

crate::impl_slint_enum_serde!(UIOcrTaskMode, Text, Spotting);

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[serde(default)]
#[from("UIDewatermarkConfig")]
pub struct DewatermarkConfigData {
    pub id: String,
    pub export_dir: String,
    pub lama_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert, Derivative)]
#[serde(default)]
#[from("UISubtitleRemoverConfig")]
pub struct SubtitleRemoverConfigData {
    pub id: String,
    pub export_dir: String,
    pub lama_path: String,
    pub is_segment_save: bool,
    #[derivative(Default(value = "60"))]
    pub segment_duration: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert, Derivative)]
#[serde(default)]
#[from("UISubtitleTranslateConfig")]
pub struct SubtitleTranslateConfigData {
    pub id: String,
    pub prompt: String,
    #[derivative(Default(value = "10"))]
    pub batch_size: i32,
    #[derivative(Default(value = "3"))]
    pub max_retries: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[derivative(Default)]
#[serde(default)]
#[from("UICutoutConfig")]
pub struct CutoutConfigData {
    pub id: String,
    pub export_dir: String,
    pub selected_model_index: i32,
    pub u2net_path: String,
    pub u2netp_path: String,
    pub u2net_cloth_seg_path: String,
    pub u2net_human_seg_path: String,
    pub isnet_anime_path: String,
    pub isnet_general_use_path: String,
    pub silueta_path: String,
    #[derivative(Default(value = "160"))]
    pub threshold: i32,
    pub binary: bool,
    pub sticker_color_r: i32,
    pub sticker_color_g: i32,
    pub sticker_color_b: i32,
    pub sticker_color_a: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[serde(default)]
#[derivative(Default)]
#[from("UIClearVisionConfig")]
pub struct ClearVisionConfigData {
    pub id: String,
    pub export_dir: String,
    pub swinir_path: String,

    // deconvolution
    pub psf_type: String,
    #[derivative(Default(value = "2.15"))]
    pub sigma: f32,
    #[derivative(Default(value = "15"))]
    pub motion_length: i32,
    #[derivative(Default(value = "0.0"))]
    pub motion_angle: f32,
    #[derivative(Default(value = "5"))]
    pub defocus_radius: i32,
    #[derivative(Default(value = "3.0"))]
    pub sigma_major: f32,
    #[derivative(Default(value = "1.0"))]
    pub sigma_minor: f32,
    #[derivative(Default(value = "0.0"))]
    pub oriented_angle: f32,
    #[derivative(Default(value = "5"))]
    pub box_width: i32,
    #[derivative(Default(value = "5"))]
    pub box_height: i32,
    #[derivative(Default(value = "5"))]
    pub disk_radius: i32,
    #[derivative(Default(value = "2.0"))]
    pub lorentz_gamma: f32,
    #[derivative(Default(value = "4.0"))]
    pub lobe_separation: f32,

    // deconvolution - algorithm
    pub algorithm: String,
    #[derivative(Default(value = "0.01"))]
    pub tv_weight: f32,
    #[derivative(Default(value = "0.01"))]
    pub wiener_nsr: f32,
    #[derivative(Default(value = "0.0"))]
    pub relaxation: f32,
    #[derivative(Default(value = "0.01"))]
    pub tikhonov_lambda: f32,
    #[derivative(Default(value = "0.01"))]
    pub inverse_cutoff: f32,
    #[derivative(Default(value = "0.01"))]
    pub ista_lambda: f32,
    #[derivative(Default(value = "0.0"))]
    pub ista_step_size: f32,
    #[derivative(Default(value = "30"))]
    pub iterations: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[serde(default)]
#[from("UIStemSplitterConfig")]
pub struct StemSplitterConfigData {
    pub id: String,
    pub export_dir: String,
    pub model_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[serde(default)]
#[from("UIDeepFilterConfig")]
pub struct DeepFilterConfigData {
    pub id: String,
    pub selected_model_index: i32,
    pub export_dir: String,
    pub dfn2_model_dir: String,
    pub dfn2_ll_model_dir: String,
    pub dfn2_h0_model_dir: String,
    pub dfn3_model_dir: String,
    pub dfn3_ll_model_dir: String,
    pub dfn3_h0_model_dir: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[serde(default)]
#[derivative(Default)]
#[from("UIDedupPhotosConfig")]
pub struct DedupPhotosConfigData {
    pub id: String,
    pub semantic_enabled: bool,
    pub model_path: String,
    pub keep_strategy: i32,
    pub all_files: bool,

    #[derivative(Default(value = "10"))]
    pub threshold: i32,
    #[derivative(Default(value = "0.85"))]
    pub semantic_threshold: f32,
    #[derivative(Default(value = "\"duplicate\".to_string()"))]
    pub duplicate_dir_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[serde(default)]
#[from("UIDedupPhotosItem")]
pub struct DedupPhotosItemData {
    pub path: String,
    pub filename: String,
    pub status: i32,
    pub error_msg: String,
    pub result_msg: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[serde(default)]
#[derivative(Default)]
#[from("UISimilarVideoSegmentConfig")]
pub struct SimilarVideoSegmentConfigData {
    pub export_dir: String,
    #[derivative(Default(value = "0.75"))]
    pub similarity_threshold: f32,
    #[derivative(Default(value = "10"))]
    pub sample_interval: i32,
    #[derivative(Default(value = "5.0"))]
    pub before_duration_secs: f32,
    #[derivative(Default(value = "5.0"))]
    pub after_duration_secs: f32,
    #[derivative(Default(value = "true"))]
    pub keep_audio: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Derivative, SlintFromConvert)]
#[serde(default)]
#[derivative(Default)]
#[from("UISimilarVideoSegmentItem")]
pub struct SimilarVideoSegmentItemData {
    pub path: String,
    pub filename: String,
    #[derivative(Default(value = "0"))]
    pub status: i32,
    pub error_msg: String,
    #[derivative(Default(value = "0"))]
    pub match_count: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[serde(default)]
#[from("UISpeakersConfig")]
pub struct SpeakersConfigData {
    pub id: String,
    pub export_dir: String,
    pub models_dir: String,
    pub merge_gap: f32,
    pub timeline_mode: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIGridAnimConfig")]
pub struct GridAnimConfigData {
    pub rows: i32,
    pub cols: i32,
    pub amplitude: f32,
    pub node_amplitude: f32,
    pub frequency: f32,
    pub node_radius: i32,
    pub line_color_r: i32,
    pub line_color_g: i32,
    pub line_color_b: i32,
    pub line_color_a: i32,
    pub line_width: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub node_color_r: i32,
    pub node_color_g: i32,
    pub node_color_b: i32,
    pub node_color_a: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIMovingGridAnimConfig")]
pub struct MovingGridAnimConfigData {
    pub rows: i32,
    pub cols: i32,
    pub speed: f32,
    pub direction: String,
    pub line_color_r: i32,
    pub line_color_g: i32,
    pub line_color_b: i32,
    pub line_color_a: i32,
    pub line_width: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub supersample: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIGlitchAnimConfig")]
pub struct GlitchAnimConfigData {
    pub intensity: f32,
    pub scan_lines_enabled: bool,
    pub scan_line_spacing: i32,
    pub rgb_split_enabled: bool,
    pub rgb_split_offset: i32,
    pub block_shift_enabled: bool,
    pub block_shift_max_offset: i32,
    pub noise_enabled: bool,
    pub animation_speed: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UINoiseFlowConfig")]
pub struct NoiseFlowAnimConfigData {
    pub noise_scale: f32,
    pub animation_speed: f32,
    #[vec(from = "palette_r")]
    pub palette_r: Vec<i32>,
    #[vec(from = "palette_g")]
    pub palette_g: Vec<i32>,
    #[vec(from = "palette_b")]
    pub palette_b: Vec<i32>,
    #[vec(from = "palette_a")]
    pub palette_a: Vec<i32>,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub flow_direction: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIBokehAnimConfig")]
pub struct BokehAnimConfigData {
    pub spot_count: i32,
    pub min_size: f32,
    pub max_size: f32,
    pub blur_radius: f32,
    pub animation_speed: f32,
    pub hexagonal_enabled: bool,
    #[vec(from = "colors_r")]
    pub colors_r: Vec<i32>,
    #[vec(from = "colors_g")]
    pub colors_g: Vec<i32>,
    #[vec(from = "colors_b")]
    pub colors_b: Vec<i32>,
    #[vec(from = "colors_a")]
    pub colors_a: Vec<i32>,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIMatrixRainAnimConfig")]
pub struct MatrixRainAnimConfigData {
    pub columns: i32,
    pub cell_size: i32,
    pub min_speed: f32,
    pub max_speed: f32,
    pub trail_length: i32,
    pub fade_speed: f32,
    pub color_r: i32,
    pub color_g: i32,
    pub color_b: i32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub glow_intensity: f32,
    pub char_change_prob: f32,
    pub flicker_prob: f32,
    pub particle_density: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIFluidAnimConfig")]
pub struct FluidAnimConfigData {
    pub resolution_divisor: i32,
    pub viscosity: f32,
    pub diffusion: f32,
    pub force_source: String,
    pub num_sources: i32,
    pub steps_per_frame: i32,
    pub color_injection: bool,
    #[vec(from = "colors_r")]
    pub colors_r: Vec<i32>,
    #[vec(from = "colors_g")]
    pub colors_g: Vec<i32>,
    #[vec(from = "colors_b")]
    pub colors_b: Vec<i32>,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIKaleidoscopeAnimConfig")]
pub struct KaleidoscopeAnimConfigData {
    pub segments: i32,
    pub rotation_speed: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub scale: f32,
    pub complexity: i32,
    #[vec(from = "colors_r")]
    pub colors_r: Vec<i32>,
    #[vec(from = "colors_g")]
    pub colors_g: Vec<i32>,
    #[vec(from = "colors_b")]
    pub colors_b: Vec<i32>,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UILightEffectsAnimConfig")]
pub struct LightEffectsAnimConfigData {
    pub flare_count: i32,
    pub min_size: f32,
    pub max_size: f32,
    pub movement_speed: f32,
    pub elliptical_enabled: bool,
    pub bands_enabled: bool,
    #[vec(from = "colors_r")]
    pub colors_r: Vec<i32>,
    #[vec(from = "colors_g")]
    pub colors_g: Vec<i32>,
    #[vec(from = "colors_b")]
    pub colors_b: Vec<i32>,
    #[vec(from = "colors_a")]
    pub colors_a: Vec<i32>,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIInkDissipationAnimConfig")]
pub struct InkDissipationAnimConfigData {
    pub style: InkStyle,
    pub source_count: i32,
    pub spawn_rate: f32,
    pub source_lifetime: i32,
    pub initial_radius: f32,
    pub max_radius: f32,
    pub spread_rate: f32,
    pub diffusion_strength: f32,
    pub fade_speed: f32,
    pub max_drops: i32,
    pub resolution_divisor: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIParticleLifeAnimConfig")]
pub struct ParticleLifeAnimConfigData {
    pub particle_count: i32,
    pub type_count: i32,
    pub rmax: f32,
    pub friction: f32,
    pub force: f32,
    pub dt: f32,
    pub wrap: bool,
    pub particle_size: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub matrix_seed: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIParticleNetworkAnimConfig")]
pub struct ParticleNetworkAnimConfigData {
    pub density: i32,
    pub line_color_r: i32,
    pub line_color_g: i32,
    pub line_color_b: i32,
    pub line_color_a: i32,
    pub particle_color_r: i32,
    pub particle_color_g: i32,
    pub particle_color_b: i32,
    pub particle_color_a: i32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub pointer_enabled: bool,
    pub pointer_range: f32,
    pub pointer_count: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIFlowFieldAnimConfig")]
pub struct FlowFieldAnimConfigData {
    pub color_r: i32,
    pub color_g: i32,
    pub color_b: i32,
    pub color_a: i32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub trail_opacity: f32,
    pub particle_count: i32,
    pub speed: f32,
    pub pointer_enabled: bool,
    pub pointer_count: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIShapeAnimConfig")]
pub struct ShapeAnimConfigData {
    pub max_circles: i32,
    pub rad_min: f32,
    pub rad_max: f32,
    pub filled_circle_pct: i32,
    pub concentric_circle_pct: i32,
    pub rad_threshold: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub max_opacity: f32,
    pub circle_border: f32,
    pub background_mult: f32,
    pub line_border: f32,
    pub link_dist_fraction: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIBlackHoleAnimConfig")]
pub struct BlackHoleAnimConfigData {
    pub star_count: i32,
    pub black_hole_size: f32,
    pub event_horizon_offset: f32,
    pub max_consume_frames: i32,
    pub hue_speed: f32,
    pub star_saturation: f32,
    pub star_lightness: f32,
    pub trail_alpha: f32,
    pub trail_color_r: i32,
    pub trail_color_g: i32,
    pub trail_color_b: i32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
    pub center_x: f32,
    pub center_y: f32,
    pub hole_stroke_color_r: i32,
    pub hole_stroke_color_g: i32,
    pub hole_stroke_color_b: i32,
    pub hole_inner_color_r: i32,
    pub hole_inner_color_g: i32,
    pub hole_inner_color_b: i32,
    pub hole_mid_color_r: i32,
    pub hole_mid_color_g: i32,
    pub hole_mid_color_b: i32,
    pub hole_outer_color_r: i32,
    pub hole_outer_color_g: i32,
    pub hole_outer_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIGalaxyAnimConfig")]
pub struct GalaxyAnimConfigData {
    pub star_count: i32,
    pub rotation_period: f32,
    pub appear_duration: f32,
    pub breathing_period: f32,
    pub breathing_min: f32,
    pub perspective: f32,
    pub glow_intensity: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub min_size: f32,
    pub max_size: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UITriangleAnimConfig")]
pub struct TriangleAnimConfigData {
    pub triangle_size: f32,
    pub bleed: f32,
    pub noise: f32,
    pub color1_r: i32,
    pub color1_g: i32,
    pub color1_b: i32,
    pub color2_r: i32,
    pub color2_g: i32,
    pub color2_b: i32,
    pub stroke_color_r: i32,
    pub stroke_color_g: i32,
    pub stroke_color_b: i32,
    pub stroke_color_a: i32,
    pub stroke_width: f32,
    pub point_variation_x: f32,
    pub point_variation_y: f32,
    pub point_animation_speed: f32,
    pub particle_count: i32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIWaveAnimConfig")]
pub struct WaveAnimConfigData {
    pub wave_count: i32,
    pub wave_height: f32,
    pub duration: f32,
    pub wave_color_r: i32,
    pub wave_color_g: i32,
    pub wave_color_b: i32,
    pub wave_opacity: f32,
    pub gradient_duration: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UICrossLineAnimConfig")]
pub struct CrossLineAnimConfigData {
    pub lines_num: i32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub line_color_r: i32,
    pub line_color_g: i32,
    pub line_color_b: i32,
    pub line_color_a: i32,
    pub line_width: f32,
    pub point_color_r: i32,
    pub point_color_g: i32,
    pub point_color_b: i32,
    pub point_color_a: i32,
    pub point_radius: f32,
    pub bg_color_r: i32,
    pub bg_color_g: i32,
    pub bg_color_b: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[serde(default)]
#[from("UIBackgroundAnimationConfig")]
pub struct BackgroundAnimationConfigData {
    pub id: String,
    pub animation_type: AnimationType,
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub duration: i32,
    pub save_dir: String,
    pub grid: GridAnimConfigData,
    pub moving_grid: MovingGridAnimConfigData,
    pub glitch: GlitchAnimConfigData,
    pub noise_flow: NoiseFlowAnimConfigData,
    pub bokeh: BokehAnimConfigData,
    pub matrix_rain: MatrixRainAnimConfigData,
    pub fluid: FluidAnimConfigData,
    pub kaleidoscope: KaleidoscopeAnimConfigData,
    pub light_effects: LightEffectsAnimConfigData,
    pub ink_dissipation: InkDissipationAnimConfigData,
    pub particle_life: ParticleLifeAnimConfigData,
    pub particle_network: ParticleNetworkAnimConfigData,
    pub flow_field: FlowFieldAnimConfigData,
    pub shape: ShapeAnimConfigData,
    pub black_hole: BlackHoleAnimConfigData,
    pub galaxy: GalaxyAnimConfigData,
    pub triangle: TriangleAnimConfigData,
    pub wave: WaveAnimConfigData,
    pub cross_line: CrossLineAnimConfigData,
}

impl Default for GridAnimConfigData {
    fn default() -> Self {
        GridConfig::default().into()
    }
}

impl Default for MovingGridAnimConfigData {
    fn default() -> Self {
        MovingGridConfig::default().into()
    }
}

impl Default for GlitchAnimConfigData {
    fn default() -> Self {
        GlitchConfig::default().into()
    }
}

impl Default for NoiseFlowAnimConfigData {
    fn default() -> Self {
        NoiseFlowConfig::default().into()
    }
}

impl Default for BokehAnimConfigData {
    fn default() -> Self {
        BokehConfig::default().into()
    }
}

impl Default for MatrixRainAnimConfigData {
    fn default() -> Self {
        MatrixRainConfig::default().into()
    }
}

impl Default for FluidAnimConfigData {
    fn default() -> Self {
        FluidConfig::default().into()
    }
}

impl Default for KaleidoscopeAnimConfigData {
    fn default() -> Self {
        KaleidoscopeConfig::default().into()
    }
}

impl Default for LightEffectsAnimConfigData {
    fn default() -> Self {
        LightEffectsConfig::default().into()
    }
}

impl Default for InkDissipationAnimConfigData {
    fn default() -> Self {
        InkDissipationConfig::default().into()
    }
}

impl Default for ParticleLifeAnimConfigData {
    fn default() -> Self {
        ParticleLifeConfig::default().into()
    }
}

impl Default for ParticleNetworkAnimConfigData {
    fn default() -> Self {
        ParticleNetworkConfig::default().into()
    }
}

impl Default for FlowFieldAnimConfigData {
    fn default() -> Self {
        FlowFieldConfig::default().into()
    }
}

impl Default for ShapeAnimConfigData {
    fn default() -> Self {
        ShapeConfig::default().into()
    }
}

impl Default for BlackHoleAnimConfigData {
    fn default() -> Self {
        BlackHoleConfig::default().into()
    }
}

impl Default for GalaxyAnimConfigData {
    fn default() -> Self {
        GalaxyConfig::default().into()
    }
}

impl Default for TriangleAnimConfigData {
    fn default() -> Self {
        TriangleConfig::default().into()
    }
}

impl Default for WaveAnimConfigData {
    fn default() -> Self {
        WaveConfig::default().into()
    }
}

impl Default for CrossLineAnimConfigData {
    fn default() -> Self {
        CrossLineConfig::default().into()
    }
}

impl Default for BackgroundAnimationConfigData {
    fn default() -> Self {
        let base = AnimationBaseConfig::default();

        Self {
            id: BG_ANIMATION_CONFIG_ID.to_string(),
            animation_type: AnimationType::Grid,
            width: base.width as i32,
            height: base.height as i32,
            fps: base.fps as i32,
            duration: 5,
            save_dir: String::new(),
            grid: GridAnimConfigData::default(),
            moving_grid: MovingGridAnimConfigData::default(),
            glitch: GlitchAnimConfigData::default(),
            noise_flow: NoiseFlowAnimConfigData::default(),
            bokeh: BokehAnimConfigData::default(),
            matrix_rain: MatrixRainAnimConfigData::default(),
            fluid: FluidAnimConfigData::default(),
            kaleidoscope: KaleidoscopeAnimConfigData::default(),
            light_effects: LightEffectsAnimConfigData::default(),
            ink_dissipation: InkDissipationAnimConfigData::default(),
            particle_life: ParticleLifeAnimConfigData::default(),
            particle_network: ParticleNetworkAnimConfigData::default(),
            flow_field: FlowFieldAnimConfigData::default(),
            shape: ShapeAnimConfigData::default(),
            black_hole: BlackHoleAnimConfigData::default(),
            galaxy: GalaxyAnimConfigData::default(),
            triangle: TriangleAnimConfigData::default(),
            wave: WaveAnimConfigData::default(),
            cross_line: CrossLineAnimConfigData::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIImageScrollAnimConfig")]
pub struct ImageScrollAnimConfigData {
    pub image_path: String,
    #[derivative(Default(value = "0.2"))]
    pub scroll_speed: f32,
    #[derivative(Default(value = "1.0"))]
    pub start_pause: f32,
    #[derivative(Default(value = "1.0"))]
    pub end_pause: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIGradeMarkAnimConfig")]
pub struct GradeMarkAnimConfigData {
    #[derivative(Default(value = "UIGradeMarkType::Circle"))]
    pub mark_type: UIGradeMarkType,
    #[derivative(Default(value = "255"))]
    pub color_r: i32,
    #[derivative(Default(value = "80"))]
    pub color_g: i32,
    #[derivative(Default(value = "80"))]
    pub color_b: i32,
    #[derivative(Default(value = "255"))]
    pub color_a: i32,
    #[derivative(Default(value = "100.0"))]
    pub size: f32,
    #[derivative(Default(value = "10.0"))]
    pub line_width: f32,
    #[derivative(Default(value = "500"))]
    pub duration_ms: i32,
    #[derivative(Default(value = "1.0"))]
    pub end_pause: f32,
    #[derivative(Default(value = "0.5"))]
    pub position_x: f32,
    #[derivative(Default(value = "0.5"))]
    pub position_y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum DashStyleData {
    #[default]
    Solid,
    Dash,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIArrowAnimConfig")]
pub struct ArrowAnimConfigData {
    #[derivative(Default(value = "255"))]
    pub color_r: i32,
    #[derivative(Default(value = "255"))]
    pub color_g: i32,
    #[derivative(Default(value = "255"))]
    pub color_b: i32,
    #[derivative(Default(value = "255"))]
    pub color_a: i32,
    #[derivative(Default(value = "4.0"))]
    pub line_width: f32,
    #[derivative(Default(value = "UIDashStyle::Solid"))]
    pub dash_style: UIDashStyle,
    #[derivative(Default(value = "10.0"))]
    pub dash_length: f32,
    #[derivative(Default(value = "200.0"))]
    pub length: f32,
    #[derivative(Default(value = "40.0"))]
    pub head_length: f32,
    #[derivative(Default(value = "30.0"))]
    pub head_width: f32,
    #[derivative(Default(value = "0.0"))]
    pub direction: f32,
    #[derivative(Default(value = "800"))]
    pub duration_ms: i32,
    #[derivative(Default(value = "1.0"))]
    pub end_pause: f32,
    #[derivative(Default(value = "0.5"))]
    pub position_x: f32,
    #[derivative(Default(value = "0.5"))]
    pub position_y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIRectDrawAnimConfig")]
pub struct RectDrawAnimConfigData {
    #[derivative(Default(value = "255"))]
    pub color_r: i32,
    #[derivative(Default(value = "255"))]
    pub color_g: i32,
    #[derivative(Default(value = "255"))]
    pub color_b: i32,
    #[derivative(Default(value = "255"))]
    pub color_a: i32,
    #[derivative(Default(value = "4.0"))]
    pub line_width: f32,
    #[derivative(Default(value = "UIDashStyle::Solid"))]
    pub dash_style: UIDashStyle,
    #[derivative(Default(value = "10.0"))]
    pub dash_length: f32,
    #[derivative(Default(value = "300.0"))]
    pub rect_width: f32,
    #[derivative(Default(value = "200.0"))]
    pub rect_height: f32,
    #[derivative(Default(value = "0.0"))]
    pub corner_radius: f32,
    #[derivative(Default(value = "800"))]
    pub duration_ms: i32,
    #[derivative(Default(value = "1.0"))]
    pub end_pause: f32,
    #[derivative(Default(value = "0.5"))]
    pub position_x: f32,
    #[derivative(Default(value = "0.5"))]
    pub position_y: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum ImageAnimationType {
    #[default]
    Scroll,
    GradeMark,
    Arrow,
    RectDraw,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert, derivative::Derivative)]
#[derivative(Default)]
#[serde(default)]
#[from("UIImageAnimationConfig")]
pub struct ImageAnimationConfigData {
    pub id: String,
    pub animation_type: ImageAnimationType,
    #[derivative(Default(value = "1920"))]
    pub width: i32,
    #[derivative(Default(value = "1080"))]
    pub height: i32,
    #[derivative(Default(value = "25"))]
    pub fps: i32,
    pub save_dir: String,
    pub scroll: ImageScrollAnimConfigData,
    pub grade_mark: GradeMarkAnimConfigData,
    pub arrow: ArrowAnimConfigData,
    pub rect_draw: RectDrawAnimConfigData,
}

crate::impl_c_like_enum_convert!(UIInkStyle, InkStyle, InkOnPaper, PaperOnInk);
crate::impl_slint_enum_serde!(
    UIGlobalFilterType,
    ProgressBar,
    Timer,
    GlobalSpeed,
    Rotation,
    Danmaku
);
crate::impl_slint_enum_serde!(UITimerMode, CountUp, CountDown);
crate::impl_slint_enum_serde!(UIDanmakuDistributionMode, StartDense, Uniform, EndDense);
crate::impl_c_like_enum_convert!(
    UIDanmakuDistributionMode,
    DanmakuDistributionMode,
    StartDense,
    Uniform,
    EndDense
);

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIGlobalFilterConfig")]
#[serde(default)]
pub struct GlobalFilterConfigData {
    pub filter_type: UIGlobalFilterType,
    pub progress_bar: ProgressBarGlobalFilterConfigData,
    pub timer: TimerGlobalFilterConfigData,
    pub global_speed: GlobalSpeedFilterConfigData,
    pub rotation: RotationGlobalFilterConfigData,
    pub danmaku: DanmakuGlobalFilterConfigData,
}

#[derive(Serialize, Deserialize, Debug, Clone, derivative::Derivative, SlintFromConvert)]
#[from("UIGlobalSpeedFilterConfig")]
#[derivative(Default)]
#[serde(default)]
pub struct GlobalSpeedFilterConfigData {
    pub enabled: bool,

    #[derivative(Default(value = "1.0"))]
    pub speed: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[from("UIProgressBarGlobalFilterConfig")]
#[serde(default)]
pub struct ProgressBarGlobalFilterConfigData {
    pub enabled: bool,
    pub font_size: i32,
    pub font_path: String,
    pub font_family: String,
    pub font_style: String,
    #[vec(from = "items")]
    pub items: Vec<ProgressBarItemData>,
    pub position_y: f32,
    pub padding: i32,
    pub margin_h: i32,
    pub separator_width: i32,
    pub text_color_r: i32,
    pub text_color_g: i32,
    pub text_color_b: i32,
    pub text_color_a: i32,
    pub progress_color_r: i32,
    pub progress_color_g: i32,
    pub progress_color_b: i32,
    pub progress_color_a: i32,
    pub background_color_r: i32,
    pub background_color_g: i32,
    pub background_color_b: i32,
    pub background_color_a: i32,
    pub separator_color_r: i32,
    pub separator_color_g: i32,
    pub separator_color_b: i32,
    pub separator_color_a: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIProgressBarItem")]
#[serde(default)]
pub struct ProgressBarItemData {
    pub timeline_offset: i32,
    pub text: String,
}

impl Default for ProgressBarGlobalFilterConfigData {
    fn default() -> ProgressBarGlobalFilterConfigData {
        ProgressBarFilter::default().into()
    }
}

impl From<ProgressBarFilter> for ProgressBarGlobalFilterConfigData {
    fn from(v: ProgressBarFilter) -> ProgressBarGlobalFilterConfigData {
        ProgressBarGlobalFilterConfigData {
            enabled: true,
            font_size: v.font_size as i32,
            font_path: v
                .font_path
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            font_family: v.font_family.unwrap_or_default(),
            font_style: v.font_style.unwrap_or_default(),
            items: v
                .segments
                .iter()
                .map(|seg| ProgressBarItemData {
                    text: seg.text.clone(),
                    timeline_offset: seg.start_time.as_millis() as i32,
                })
                .collect(),
            position_y: v.position_y,
            padding: v.padding as i32,
            margin_h: v.margin_h as i32,
            separator_width: v.separator_width as i32,
            text_color_r: v.text_color.0 as i32,
            text_color_g: v.text_color.1 as i32,
            text_color_b: v.text_color.2 as i32,
            text_color_a: v.text_color.3 as i32,
            progress_color_r: v.progress_color.0 as i32,
            progress_color_g: v.progress_color.1 as i32,
            progress_color_b: v.progress_color.2 as i32,
            progress_color_a: v.progress_color.3 as i32,
            background_color_r: v.background_color.0 as i32,
            background_color_g: v.background_color.1 as i32,
            background_color_b: v.background_color.2 as i32,
            background_color_a: v.background_color.3 as i32,
            separator_color_r: v.separator_color.0 as i32,
            separator_color_g: v.separator_color.1 as i32,
            separator_color_b: v.separator_color.2 as i32,
            separator_color_a: v.separator_color.3 as i32,
        }
    }
}

impl From<ProgressBarGlobalFilterConfigData> for ProgressBarFilter {
    fn from(v: ProgressBarGlobalFilterConfigData) -> ProgressBarFilter {
        let mut filter = ProgressBarFilter::new();
        for item in v.items {
            filter.add_segment(
                item.text,
                Duration::from_millis(item.timeline_offset as u64),
            );
        }
        filter.position_y = v.position_y;
        filter.padding = v.padding as u32;
        filter.margin_h = v.margin_h as u32;
        filter.separator_width = v.separator_width as u32;
        filter.text_color = (
            v.text_color_r as u8,
            v.text_color_g as u8,
            v.text_color_b as u8,
            v.text_color_a as u8,
        );
        filter.progress_color = (
            v.progress_color_r as u8,
            v.progress_color_g as u8,
            v.progress_color_b as u8,
            v.progress_color_a as u8,
        );
        filter.background_color = (
            v.background_color_r as u8,
            v.background_color_g as u8,
            v.background_color_b as u8,
            v.background_color_a as u8,
        );
        filter.separator_color = (
            v.separator_color_r as u8,
            v.separator_color_g as u8,
            v.separator_color_b as u8,
            v.separator_color_a as u8,
        );
        filter.font_size = v.font_size as u32;
        if !v.font_path.is_empty() {
            filter.font_path = Some(PathBuf::from(v.font_path));
        }
        if !v.font_family.is_empty() {
            filter.font_family = Some(v.font_family);
        }
        if !v.font_style.is_empty() {
            filter.font_style = Some(v.font_style);
        }
        filter
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[from("UITimerGlobalFilterConfig")]
#[serde(default)]
pub struct TimerGlobalFilterConfigData {
    pub enabled: bool,
    #[vec(from = "items")]
    pub items: Vec<TimerItemData>,
    pub default_style: TimerStyleData,
}

impl Default for TimerGlobalFilterConfigData {
    fn default() -> TimerGlobalFilterConfigData {
        TimerFilter::default().into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UITimerStyle")]
#[serde(default)]
pub struct TimerStyleData {
    pub position_x: f32,
    pub position_y: f32,
    pub font_size: i32,
    pub font_path: String,
    pub font_family: String,
    pub font_style: String,
    pub text_color_r: i32,
    pub text_color_g: i32,
    pub text_color_b: i32,
    pub text_color_a: i32,
    pub background_color_r: i32,
    pub background_color_g: i32,
    pub background_color_b: i32,
    pub background_color_a: i32,
    pub padding: i32,
    pub border_radius: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UITimerItem")]
#[serde(default)]
pub struct TimerItemData {
    pub start_offset: i32,
    pub end_offset: i32,
    pub mode: UITimerMode,
    pub style: TimerStyleData,
}

impl From<TimerFilter> for TimerGlobalFilterConfigData {
    fn from(v: TimerFilter) -> TimerGlobalFilterConfigData {
        TimerGlobalFilterConfigData {
            enabled: true,
            items: v
                .segments
                .iter()
                .map(|seg| TimerItemData {
                    start_offset: seg.start_time.as_millis() as i32,
                    end_offset: seg.end_time.as_millis() as i32,
                    mode: match seg.mode {
                        TimerMode::CountUp => UITimerMode::CountUp,
                        TimerMode::CountDown => UITimerMode::CountDown,
                    },
                    style: TimerStyleData {
                        position_x: seg.position_x,
                        position_y: seg.position_y,
                        font_size: seg.font_size as i32,
                        font_path: seg
                            .font_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        font_family: seg.font_family.clone().unwrap_or_default(),
                        font_style: seg.font_style.clone().unwrap_or_default(),
                        text_color_r: seg.text_color.0 as i32,
                        text_color_g: seg.text_color.1 as i32,
                        text_color_b: seg.text_color.2 as i32,
                        text_color_a: seg.text_color.3 as i32,
                        background_color_r: seg.background_color.0 as i32,
                        background_color_g: seg.background_color.1 as i32,
                        background_color_b: seg.background_color.2 as i32,
                        background_color_a: seg.background_color.3 as i32,
                        padding: seg.padding as i32,
                        border_radius: seg.border_radius as i32,
                    },
                })
                .collect(),
            default_style: TimerStyleData::default(),
        }
    }
}

impl From<TimerGlobalFilterConfigData> for TimerFilter {
    fn from(v: TimerGlobalFilterConfigData) -> TimerFilter {
        let mut filter = TimerFilter::new();
        for item in v.items {
            let segment = TimerSegment {
                start_time: Duration::from_millis(item.start_offset as u64),
                end_time: Duration::from_millis(item.end_offset as u64),
                mode: match item.mode {
                    UITimerMode::CountUp => TimerMode::CountUp,
                    UITimerMode::CountDown => TimerMode::CountDown,
                },
                position_x: item.style.position_x,
                position_y: item.style.position_y,
                font_size: item.style.font_size as u32,
                font_path: if !item.style.font_path.is_empty() {
                    Some(PathBuf::from(item.style.font_path))
                } else {
                    None
                },
                font_family: if !item.style.font_family.is_empty() {
                    Some(item.style.font_family)
                } else {
                    None
                },
                font_style: if !item.style.font_style.is_empty() {
                    Some(item.style.font_style)
                } else {
                    None
                },
                text_color: (
                    item.style.text_color_r as u8,
                    item.style.text_color_g as u8,
                    item.style.text_color_b as u8,
                    item.style.text_color_a as u8,
                ),
                background_color: (
                    item.style.background_color_r as u8,
                    item.style.background_color_g as u8,
                    item.style.background_color_b as u8,
                    item.style.background_color_a as u8,
                ),
                padding: item.style.padding as u32,
                border_radius: item.style.border_radius as u32,
            };
            filter.add_segment(segment);
        }
        filter
    }
}

impl From<GlobalSpeedFilter> for GlobalSpeedFilterConfigData {
    fn from(v: GlobalSpeedFilter) -> GlobalSpeedFilterConfigData {
        GlobalSpeedFilterConfigData {
            enabled: true,
            speed: v.speed,
        }
    }
}

impl From<GlobalSpeedFilterConfigData> for GlobalSpeedFilter {
    fn from(v: GlobalSpeedFilterConfigData) -> GlobalSpeedFilter {
        GlobalSpeedFilter::new().with_speed(v.speed)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, derivative::Derivative, SlintFromConvert)]
#[from("UIRotationGlobalFilterConfig")]
#[derivative(Default)]
#[serde(default)]
pub struct RotationGlobalFilterConfigData {
    pub enabled: bool,
    pub rotation: f32,
}

impl From<RotationGlobalFilter> for RotationGlobalFilterConfigData {
    fn from(v: RotationGlobalFilter) -> RotationGlobalFilterConfigData {
        RotationGlobalFilterConfigData {
            enabled: true,
            rotation: v.rotation,
        }
    }
}

impl From<RotationGlobalFilterConfigData> for RotationGlobalFilter {
    fn from(v: RotationGlobalFilterConfigData) -> RotationGlobalFilter {
        RotationGlobalFilter::new(v.rotation)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[from("UIDanmakuGlobalFilterConfig")]
#[serde(default)]
pub struct DanmakuGlobalFilterConfigData {
    pub enabled: bool,
    #[vec(from = "items")]
    pub items: Vec<DanmakuSegmentData>,
    pub default_style: DanmakuStyleData,
}

impl Default for DanmakuGlobalFilterConfigData {
    fn default() -> DanmakuGlobalFilterConfigData {
        DanmakuFilter::default().into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, SlintFromConvert)]
#[from("UIDanmakuStyle")]
#[serde(default)]
pub struct DanmakuStyleData {
    pub font_path: String,
    pub font_family: String,
    pub font_style: String,
    pub font_size: i32,
    pub color_r: i32,
    pub color_g: i32,
    pub color_b: i32,
    pub color_a: i32,
    pub outline_width: i32,
    pub outline_color_r: i32,
    pub outline_color_g: i32,
    pub outline_color_b: i32,
    pub outline_color_a: i32,
    pub line_spacing: i32,
}

impl Default for DanmakuStyleData {
    fn default() -> DanmakuStyleData {
        DanmakuStyle::default().into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIDanmakuItem")]
#[serde(default)]
pub struct DanmakuItemData {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIDanmakuSegment")]
#[serde(default)]
pub struct DanmakuSegmentData {
    pub start_offset: i32,
    pub end_offset: i32,
    pub scroll_speed: f32,
    pub distribution: UIDanmakuDistributionMode,
    pub track_count: i32,
    pub track_distribution: UIDanmakuDistributionMode,
    pub position: f32,
    #[vec(from = "items")]
    pub items: Vec<DanmakuItemData>,
    pub style: DanmakuStyleData,
}

impl From<DanmakuFilter> for DanmakuGlobalFilterConfigData {
    fn from(v: DanmakuFilter) -> DanmakuGlobalFilterConfigData {
        DanmakuGlobalFilterConfigData {
            enabled: true,
            items: v
                .segments
                .into_iter()
                .map(|seg| DanmakuSegmentData {
                    start_offset: seg.start_time.as_millis() as i32,
                    end_offset: seg.end_time.as_millis() as i32,
                    scroll_speed: seg.scroll_speed,
                    distribution: seg.distribution.into(),
                    track_count: seg.track_count as i32,
                    track_distribution: seg.track_distribution.into(),
                    position: seg.position,
                    items: seg
                        .items
                        .iter()
                        .map(|item| DanmakuItemData {
                            text: item.text.clone(),
                        })
                        .collect(),
                    style: seg.style.into(),
                })
                .collect(),
            default_style: v.default_style.into(),
        }
    }
}

impl From<DanmakuGlobalFilterConfigData> for DanmakuFilter {
    fn from(v: DanmakuGlobalFilterConfigData) -> DanmakuFilter {
        let mut filter = DanmakuFilter::new();
        filter.default_style = v.default_style.clone().into();
        for seg in v.items {
            let segment = DanmakuSegment {
                start_time: Duration::from_millis(seg.start_offset as u64),
                end_time: Duration::from_millis(seg.end_offset as u64),
                scroll_speed: seg.scroll_speed,
                distribution: seg.distribution.into(),
                track_count: seg.track_count as u32,
                track_distribution: seg.track_distribution.into(),
                position: seg.position,
                items: seg
                    .items
                    .iter()
                    .map(|item| DanmakuItem {
                        text: item.text.clone(),
                    })
                    .collect(),
                style: seg.style.into(),
            };
            filter.segments.push(segment);
        }
        filter
    }
}

impl From<DanmakuStyle> for DanmakuStyleData {
    fn from(v: DanmakuStyle) -> DanmakuStyleData {
        DanmakuStyleData {
            font_path: v.font_path.to_string_lossy().to_string(),
            font_family: v.font_family,
            font_style: v.font_style,
            font_size: v.font_size as i32,
            color_r: v.color.0 as i32,
            color_g: v.color.1 as i32,
            color_b: v.color.2 as i32,
            color_a: v.color.3 as i32,
            outline_width: v.outline_width as i32,
            outline_color_r: v.outline_color.0 as i32,
            outline_color_g: v.outline_color.1 as i32,
            outline_color_b: v.outline_color.2 as i32,
            outline_color_a: v.outline_color.3 as i32,
            line_spacing: v.line_spacing as i32,
        }
    }
}

impl From<DanmakuStyleData> for DanmakuStyle {
    fn from(v: DanmakuStyleData) -> DanmakuStyle {
        DanmakuStyle {
            font_path: if !v.font_path.is_empty() {
                PathBuf::from(v.font_path)
            } else {
                PathBuf::new()
            },
            font_family: v.font_family,
            font_style: v.font_style,
            font_size: v.font_size as u32,
            color: (
                v.color_r as u8,
                v.color_g as u8,
                v.color_b as u8,
                v.color_a as u8,
            ),
            outline_width: v.outline_width as u32,
            outline_color: (
                v.outline_color_r as u8,
                v.outline_color_g as u8,
                v.outline_color_b as u8,
                v.outline_color_a as u8,
            ),
            line_spacing: v.line_spacing as u32,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, SlintFromConvert, Derivative)]
#[derivative(Default)]
#[from("UISceneDetectConfig")]
pub struct SceneDetectConfigData {
    algorithm: UISceneDetectorAlgorithm,
    #[derivative(Default(value = "1"))]
    min_duration: i32,
    #[derivative(Default(value = "27.0"))]
    content_threshold: f32,
    #[derivative(Default(value = "3.0"))]
    adaptive_threshold: f32,
    #[derivative(Default(value = "0.5"))]
    histogram_threshold: f32,
    #[derivative(Default(value = "12.0"))]
    threshold_threshold: f32,
}

crate::impl_slint_enum_serde!(
    UISceneDetectorAlgorithm,
    Content,
    Adaptive,
    Histogram,
    Threshold
);

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIOnlineSearchImageSourceEntry")]
#[serde(default)]
pub struct OnlineSearchImageSourceEntryData {
    pub name: String,
    pub enabled: bool,
    pub can_access: bool,
    pub proxy_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert, Derivative)]
#[from("UIOnlineSearchImageSetting")]
#[serde(default)]
pub struct OnlineSearchImageConfigData {
    pub id: String,
    pub download_dir: String,
    pub http_proxy_url: String,
    pub socks5_proxy_url: String,
    #[derivative(Default(value = "25"))]
    pub search_limits: i32,
    #[vec(from = "sources")]
    pub sources: Vec<OnlineSearchImageSourceEntryData>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert)]
#[from("UIOnlineSearchAudioSourceEntry")]
#[serde(default)]
pub struct OnlineSearchAudioSourceEntryData {
    pub name: String,
    pub enabled: bool,
    pub can_access: bool,
    pub proxy_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, SlintFromConvert, Derivative)]
#[from("UIOnlineSearchAudioSetting")]
#[serde(default)]
pub struct OnlineSearchAudioConfigData {
    pub id: String,
    pub download_dir: String,
    #[derivative(Default(value = "10"))]
    pub search_limits: i32,
    #[vec(from = "sources")]
    pub sources: Vec<OnlineSearchAudioSourceEntryData>,
}
