use crate::{
    config,
    db::{SmartMixConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory,
        toast,
        tr::tr,
        video_editor::{
            command::{sync_and_refresh, with_history_manager},
            project::SMART_MIX_CONFIG_ID,
            track::is_track_locked,
            vad::{detect_voice_segments, to_mono},
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        SettingTranscribe as UISettingTranscribe, SmartMix as UISmartMix,
        SmartMixEntry as UISmartMixEntry, SmartMixMatchItem as UISmartMixMatchItem,
        SmartMixProgressType as UISmartMixProgressType, SmartMixSetting as UISmartMixSetting,
        SmartMixVisualItem as UISmartMixVisualItem,
    },
    toast_info, toast_success, toast_warn,
};
use audio_utils::loader::AudioConfig;
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use downloader::DownloadState;
use fun_ast_nano::{FunASRModelConfig, FunAsrError, FunAsrNanoGenerateModel};
use image::DynamicImage::ImageRgba8;
use lfm_vl_onnx::{LfmTokenizer, LfmVlModel, Precision, generate};
use once_cell::sync::Lazy;
use pmacro::SlintFromConvert;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use video_editor::{
    commands::{
        AffectedSegment,
        batch::BatchCommand,
        segment::{AddSegmentCommand, RemoveSegmentCommand},
    },
    tracks::{
        audio_track::extract_segment_audio, segment::Segment, track::Track,
        video_frame_cache::VideoImage,
    },
};
use video_utils::convert::resize_rgba_image_contain;

const DEFAULT_ASR_PROMPT: &str = "Transcribe audio to text.";
const DEFAULT_VL_PROMPT: &str = "用中文简要描述这张图片的内容。";

#[derive(Default)]
struct SmartMixCache {
    stop_sig: Option<Arc<AtomicBool>>,
    download_stop_sigs: HashMap<usize, Arc<AtomicBool>>,
    inc_index: u64,
}

static SMART_MIX_CACHE: Lazy<Mutex<SmartMixCache>> =
    Lazy::new(|| Mutex::new(SmartMixCache::default()));

#[macro_export]
macro_rules! store_video_editor_smart_mix_entries {
    ($smart_mix: expr) => {
        $smart_mix
            .audio_entries
            .as_any()
            .downcast_ref::<VecModel<UISmartMixEntry>>()
            .expect("We know we set a VecModel<UISmartMixEntry> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_smart_mix_visual_items {
    ($smart_mix: expr) => {
        $smart_mix
            .visual_items
            .as_any()
            .downcast_ref::<VecModel<UISmartMixVisualItem>>()
            .expect("We know we set a VecModel<UISmartMixVisualItem> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_smart_mix_match_items {
    ($smart_mix: expr) => {
        $smart_mix
            .match_items
            .as_any()
            .downcast_ref::<VecModel<UISmartMixMatchItem>>()
            .expect("We know we set a VecModel<UISmartMixMatchItem> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_smart_mix_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_smart_mix_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier")
    };
}

#[derive(Debug, Clone, Default, SlintFromConvert)]
#[from("UISmartMixEntry")]
struct SmartMixEntryData {
    start_ms: i32,
    end_ms: i32,
    text: String,
}

#[derive(Debug, Clone, Default, SlintFromConvert)]
#[from("UISmartMixVisualItem")]
struct SmartMixVisualItemData {
    track_index: i32,
    segment_index: i32,
    is_image: bool,
    filename: String,
    description: String,
    duration_ms: i32,
}

#[derive(Debug, Clone, Default, SlintFromConvert)]
#[from("UISmartMixMatchItem")]
struct SmartMixMatchItemData {
    audio_index: i32,
    visual_index: i32,
    source_offset_ms: i32,
    duration_ms: i32,
    filename: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AiMatchResult {
    audio_index: usize,
    visual_index: usize,
    source_offset_ms: u64,
    duration_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct AiAudioSegment {
    index: usize,
    start_ms: u64,
    end_ms: u64,
    text: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct AiVisualSegment {
    index: usize,
    description: String,
    duration_ms: u64,
    is_image: bool,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_smart_mix_start, ui);
    logic_cb!(video_editor_smart_mix_cancel, ui);
    logic_cb!(video_editor_smart_mix_apply, ui);
    logic_cb!(video_editor_smart_mix_remove_all, ui);
    logic_cb!(video_editor_smart_mix_retry_ai_matching, ui);
    logic_cb!(video_editor_smart_mix_choose_vl_model_dir, ui, index);
    logic_cb_pure!(video_editor_smart_mix_setting_is_valid, ui);
    logic_cb_pure!(video_editor_smart_mix_vl_model_is_valid, ui, index);
    logic_cb!(video_editor_smart_mix_update_config, ui, config);
    logic_cb!(video_editor_smart_mix_model_start_download, ui, index);
    logic_cb!(video_editor_smart_mix_model_cancel_download, ui, index);
}

fn inner_init(ui: &AppWindow) {
    let mut smart_mix = UISmartMix::default();
    smart_mix.is_auto_find_tracks = true;
    smart_mix.audio_entries = ModelRc::new(VecModel::from_slice(&[]));
    smart_mix.visual_items = ModelRc::new(VecModel::from_slice(&[]));
    smart_mix.match_items = ModelRc::new(VecModel::from_slice(&[]));
    global_store!(ui).set_video_editor_smart_mix(smart_mix);

    let base_url = Precision::browser_url().to_string();
    let downloaders: Vec<UIDownloader> = Precision::all_precisions()
        .iter()
        .map(|_| UIDownloader {
            state: UIDownloaderState::UnStart,
            progress: 0.0,
            url: base_url.clone().into(),
            filename: Default::default(),
        })
        .collect();
    store_video_editor_smart_mix_models_downloader!(ui).set_vec(downloaders);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_smart_mix_config().await.unwrap_or_default();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_smart_mix_setting(config.into());
        });
    });
}

fn reset_smart_mix_stop_sig() {
    let mut cache = SMART_MIX_CACHE.lock().unwrap();
    if let Some(stop_sig) = cache.stop_sig.take() {
        stop_sig.store(true, Ordering::Relaxed);
    }
    cache.stop_sig = Some(Arc::new(AtomicBool::new(false)));
}

fn is_cancelled() -> bool {
    let cache = SMART_MIX_CACHE.lock().unwrap();
    cache
        .stop_sig
        .as_ref()
        .is_some_and(|sig| sig.load(Ordering::Relaxed))
}

fn bump_inc_index() -> u64 {
    let mut cache = SMART_MIX_CACHE.lock().unwrap();
    cache.inc_index += 1;
    cache.inc_index
}

fn is_stale(inc_index: u64) -> bool {
    inc_index != SMART_MIX_CACHE.lock().unwrap().inc_index
}

fn video_editor_smart_mix_setting_is_valid(ui: &AppWindow) -> bool {
    let config = global_store!(ui).get_video_editor_smart_mix_setting();

    let asr_setting = global_store!(ui).get_video_editor_transcribe_setting();
    let asr_valid = cutil::fs::file_exist(&asr_setting.model_path)
        && cutil::fs::file_exist(&asr_setting.model_tokenizer_path);

    let vl_index = precision_to_index(&config.vl_precision);
    asr_valid && video_editor_smart_mix_vl_model_is_valid(ui, vl_index)
}

fn precision_to_index(precision: &str) -> i32 {
    match precision {
        "fp32" => 0,
        "fp16" => 1,
        "q4" => 2,
        "q8" => 3,
        _ => 1,
    }
}

/// Check if the VL model at the given precision index contains all required files.
fn video_editor_smart_mix_vl_model_is_valid(ui: &AppWindow, index: i32) -> bool {
    let config = global_store!(ui).get_video_editor_smart_mix_setting();
    let index = index as usize;

    let model_dir = get_model_dir_by_index(&config, index);
    if model_dir.is_empty() || !Path::new(model_dir.as_str()).is_dir() {
        return false;
    }

    let precision = match index {
        0 => Precision::Fp32,
        1 => Precision::Fp16,
        2 => Precision::Q4,
        3 => Precision::Q8,
        _ => Precision::Fp16,
    };

    precision
        .missing_files(Path::new(model_dir.as_str()))
        .is_empty()
}

fn video_editor_smart_mix_start(ui: &AppWindow) {
    reset_smart_mix_stop_sig();

    if !video_editor_smart_mix_setting_is_valid(ui) {
        global_store!(ui).set_video_editor_smart_mix_is_show_setting_dialog(true);
        return;
    }

    let is_auto = global_store!(ui)
        .get_video_editor_smart_mix()
        .is_auto_find_tracks;
    let (audio_track_idx, visual_track_indices) = if is_auto {
        find_tracks()
    } else {
        find_tracks_from_selected(ui)
    };

    if audio_track_idx.is_none() {
        toast_warn!(
            ui,
            tr("No audio track found. Please select an audio track.")
        );
        return;
    }

    if visual_track_indices.is_empty() {
        toast_warn!(
            ui,
            tr("No video or image tracks found. Please add visual tracks.")
        );
        return;
    }

    let audio_track_idx = audio_track_idx.unwrap();
    if is_track_locked(ui, audio_track_idx as i32) {
        toast_warn!(ui, tr("Cannot operate on a locked audio track"));
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_smart_mix();
    entry.progress = 0.0;
    entry.progress_type = UISmartMixProgressType::TranscribingAudio;
    entry.audio_entries = ModelRc::new(VecModel::from_slice(&[]));
    entry.visual_items = ModelRc::new(VecModel::from_slice(&[]));
    entry.match_items = ModelRc::new(VecModel::from_slice(&[]));
    global_store!(ui).set_video_editor_smart_mix(entry);

    toast_info!(ui, tr("Starting smart mix, please wait..."));

    let setting = global_store!(ui).get_video_editor_smart_mix_setting();
    let asr_setting = global_store!(ui).get_video_editor_transcribe_setting();
    let inc_index = bump_inc_index();
    let ui_weak = ui.as_weak();

    std::thread::spawn(move || {
        // Phase 1: Audio transcription
        let transcribed = match transcribe_audio(&ui_weak, audio_track_idx, &asr_setting, inc_index)
        {
            Some(segments) => segments,
            None => return,
        };

        if is_cancelled() || is_stale(inc_index) {
            return;
        }

        // Update UI with audio entries
        let transcribed_clone = transcribed.clone();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_mix();
            entry.progress_type = UISmartMixProgressType::RecognizingVisual;
            entry.progress = 0.0;
            let entries = store_video_editor_smart_mix_entries!(entry);
            for seg in &transcribed_clone {
                entries.push(seg.clone().into());
            }
            global_store!(ui).set_video_editor_smart_mix(entry);
        });

        if is_cancelled() || is_stale(inc_index) {
            return;
        }

        // Phase 2: Visual recognition
        let visuals = match recognize_visuals(&ui_weak, &visual_track_indices, &setting, inc_index)
        {
            Some(descriptions) => descriptions,
            None => return,
        };

        if is_cancelled() || is_stale(inc_index) {
            return;
        }

        let visuals_clone = visuals.clone();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }

            let entry = global_store!(ui).get_video_editor_smart_mix();
            let items = store_video_editor_smart_mix_visual_items!(entry);
            for (vis, _) in &visuals_clone {
                items.push(vis.clone().into());
            }
            global_store!(ui).set_video_editor_smart_mix(entry);
        });

        if is_cancelled() || is_stale(inc_index) {
            return;
        }

        // Phase 3: AI matching
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_mix();
            entry.progress_type = UISmartMixProgressType::AiMatching;
            entry.progress = 0.0;
            global_store!(ui).set_video_editor_smart_mix(entry);
        });

        let match_results = match ai_match_visuals(
            &ui_weak,
            &transcribed,
            &visuals,
            setting.reuse_media,
            setting.sequential_match,
            setting.must_cover_all_audio,
            inc_index,
        ) {
            Ok(results) => results,
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("AI matching failed")));
                return;
            }
        };

        if is_cancelled() || is_stale(inc_index) {
            return;
        }

        let match_items: Vec<SmartMixMatchItemData> = match_results
            .iter()
            .map(|m| {
                let filename = visuals
                    .get(m.visual_index)
                    .map(|(v, _)| v.filename.clone())
                    .unwrap_or_default();
                SmartMixMatchItemData {
                    audio_index: m.audio_index as i32,
                    visual_index: m.visual_index as i32,
                    source_offset_ms: m.source_offset_ms as i32,
                    duration_ms: m.duration_ms as i32,
                    filename,
                }
            })
            .collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_mix();
            let items = store_video_editor_smart_mix_match_items!(entry);
            for item in &match_items {
                items.push(item.clone().into());
            }
            entry.progress_type = UISmartMixProgressType::Finished;
            entry.progress = 1.0;
            global_store!(ui).set_video_editor_smart_mix(entry);
        });
    });
}

fn find_tracks() -> (Option<usize>, Vec<usize>) {
    let mut audio_track_idx: Option<usize> = None;
    let mut visual_track_indices: Vec<usize> = Vec::new();

    _ = with_history_manager(|state| {
        for (idx, track) in state.tracks_manager.iter().enumerate() {
            match track {
                Track::Audio(_) if audio_track_idx.is_none() => {
                    audio_track_idx = Some(idx);
                }
                Track::Video(_) | Track::Image(_) => {
                    visual_track_indices.push(idx);
                }
                _ => {}
            }
        }
    });

    (audio_track_idx, visual_track_indices)
}

fn find_tracks_from_selected(ui: &AppWindow) -> (Option<usize>, Vec<usize>) {
    let selected = global_store!(ui).get_video_editor_selected_tracks_index();
    let mut audio_track_idx: Option<usize> = None;
    let mut visual_track_indices: Vec<usize> = Vec::new();

    _ = with_history_manager(|state| {
        for sel in selected.iter() {
            let idx = sel.index as usize;
            if let Some(track) = state.tracks_manager.get(idx) {
                match track {
                    Track::Audio(_) if audio_track_idx.is_none() => {
                        audio_track_idx = Some(idx);
                    }
                    Track::Video(_) | Track::Image(_) => {
                        visual_track_indices.push(idx);
                    }
                    _ => {}
                }
            }
        }
    });

    (audio_track_idx, visual_track_indices)
}

fn transcribe_audio(
    ui_weak: &Weak<AppWindow>,
    audio_track_idx: usize,
    asr_setting: &UISettingTranscribe,
    inc_index: u64,
) -> Option<Vec<SmartMixEntryData>> {
    let segments = with_history_manager(|state| {
        state
            .tracks_manager
            .get(audio_track_idx)
            .map(|track| track.segments().to_vec())
            .unwrap_or_default()
    });

    if segments.is_empty() {
        toast::async_toast_warn(ui_weak.clone(), tr("Audio track has no segments"));
        return None;
    }

    // Collect all audio data from segments
    let mut all_samples: Vec<f32> = Vec::new();
    let mut total_duration = Duration::ZERO;

    for segment in &segments {
        if segment.metadata.audios.is_empty() {
            continue;
        }

        let audio_meta = segment.metadata.audios.first().unwrap();
        let result = extract_segment_audio(
            &segment.metadata.path,
            audio_meta.index,
            segment,
            segment.timeline_offset,
            segment.duration,
            audio_meta.channels,
            audio_meta.sample_rate,
            fun_ast_nano::INPUT_AUDIO_CHANNELS as u16,
            fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE,
        );

        match result {
            Ok(seg_samples) => {
                let samples: Vec<f32> = seg_samples
                    .samples
                    .into_iter()
                    .map(|s| s.unwrap_or(0.0))
                    .collect();
                total_duration += segment.duration;
                all_samples.extend_from_slice(&samples);
            }
            Err(e) => log::warn!("Failed to extract audio from segment: {e}"),
        }

        if is_cancelled() || is_stale(inc_index) {
            return None;
        }
    }

    if all_samples.is_empty() {
        toast::async_toast_warn(ui_weak.clone(), tr("No audio data available"));
        return None;
    }

    // VAD
    let mono = to_mono(&all_samples, fun_ast_nano::INPUT_AUDIO_CHANNELS as u16);
    let speech_regions = match detect_voice_segments(&mono, fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE) {
        Ok(regions) => regions,
        Err(e) => {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {e}", tr("VAD detection failed")),
            );
            return None;
        }
    };

    if speech_regions.is_empty() {
        toast::async_toast_warn(ui_weak.clone(), tr("No speech detected"));
        return None;
    }

    if is_cancelled() || is_stale(inc_index) {
        return None;
    }

    // Load ASR model
    let model_config = FunASRModelConfig::default()
        .with_model_weights(asr_setting.model_path.to_string())
        .with_tokenizer_path(asr_setting.model_tokenizer_path.to_string());

    let mut model = match FunAsrNanoGenerateModel::new(model_config, None, None) {
        Ok(m) => m,
        Err(e) => {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {e}", tr("Failed to load transcribe model")),
            );
            return None;
        }
    };

    // Transcribe each speech segment
    let audio_config = AudioConfig::default()
        .with_sample_rate(fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE)
        .with_channel(fun_ast_nano::INPUT_AUDIO_CHANNELS as u16)
        .with_duration(total_duration)
        .with_samples(all_samples.clone());

    let mut transcribed_segments: Vec<SmartMixEntryData> = Vec::new();
    let total_segments = speech_regions.len();

    for (seg_idx, (start_ms, end_ms)) in speech_regions.iter().enumerate() {
        if is_cancelled() || is_stale(inc_index) {
            return None;
        }

        let start_sample = (*start_ms as f64 / 1000.0
            * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
            .round() as usize;
        let end_sample = (*end_ms as f64 / 1000.0 * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
            .round() as usize;
        let end_sample = end_sample.min(audio_config.samples.len());

        if start_sample >= end_sample {
            continue;
        }

        let seg_audio = audio_config.samples[start_sample..end_sample].to_vec();
        let seg_duration = Duration::from_secs_f64(
            seg_audio.len() as f64
                / (fun_ast_nano::INPUT_AUDIO_CHANNELS as f64
                    * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64),
        );

        let seg_audio_config = AudioConfig::default()
            .with_sample_rate(fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE)
            .with_channel(fun_ast_nano::INPUT_AUDIO_CHANNELS as u16)
            .with_duration(seg_duration)
            .with_samples(seg_audio);

        let request = fun_ast_nano::TranscriptionRequest::default()
            .with_audio_config(seg_audio_config)
            .with_prompt(Some(DEFAULT_ASR_PROMPT.to_string()))
            .with_max_tokens(512);

        let text = match model.generate(request, None, |_chunk| Ok(())) {
            Ok(response) => {
                let text = fast2s::convert(&response.text);
                text
            }
            Err(FunAsrError::TranscribeCancelled) => return None,
            Err(e) => {
                log::warn!("Transcribe segment {} failed: {e}", seg_idx);
                String::new()
            }
        };

        transcribed_segments.push(SmartMixEntryData {
            start_ms: *start_ms as i32,
            end_ms: *end_ms as i32,
            text,
        });

        let progress = (seg_idx + 1) as f32 / total_segments as f32;
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_mix();
            entry.progress_type = UISmartMixProgressType::TranscribingAudio;
            entry.progress = progress;
            global_store!(ui).set_video_editor_smart_mix(entry);
        });
    }

    Some(transcribed_segments)
}

fn recognize_visuals(
    ui_weak: &Weak<AppWindow>,
    visual_track_indices: &[usize],
    setting: &UISmartMixSetting,
    inc_index: u64,
) -> Option<Vec<(SmartMixVisualItemData, u64)>> {
    let model_dir = get_model_dir(setting);
    let precision = match setting.vl_precision.as_str() {
        "fp32" => Precision::Fp32,
        "fp16" => Precision::Fp16,
        "q4" => Precision::Q4,
        "q8" => Precision::Q8,
        _ => Precision::Fp16,
    };
    let max_tokens = if setting.vl_max_tokens <= 0 {
        512
    } else {
        setting.vl_max_tokens as usize
    };
    let prompt = if setting.vl_prompt.is_empty() {
        DEFAULT_VL_PROMPT.to_string()
    } else {
        setting.vl_prompt.to_string()
    };
    let video_sample_fps = if setting.video_sample_fps <= 0 {
        1.0
    } else {
        setting.video_sample_fps as f32
    };
    let max_resolution = parse_resolution(setting.vl_resolution.as_str());

    let mut model = match LfmVlModel::load(Path::new(&model_dir), precision) {
        Ok(m) => m,
        Err(e) => {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {e}", tr("Failed to load VL model")),
            );
            return None;
        }
    };

    let tokenizer_path = Path::new(&model_dir).join("tokenizer.json");
    let tokenizer = match LfmTokenizer::from_file(&tokenizer_path) {
        Ok(t) => t,
        Err(e) => {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {e}", tr("Failed to load VL tokenizer")),
            );
            return None;
        }
    };

    // Collect all visual segments
    let all_visual_segments: Vec<(usize, usize, Arc<Segment>, bool)> =
        with_history_manager(|state| {
            let mut result = Vec::new();
            for &track_idx in visual_track_indices {
                if let Some(track) = state.tracks_manager.get(track_idx) {
                    let is_image = matches!(track, Track::Image(_));
                    for (seg_idx, segment) in track.segments().iter().enumerate() {
                        result.push((track_idx, seg_idx, segment.clone(), is_image));
                    }
                }
            }
            result
        });

    if all_visual_segments.is_empty() {
        toast::async_toast_warn(ui_weak.clone(), tr("No visual segments found"));
        return None;
    }

    let total_visual = all_visual_segments.len();
    let mut visual_descriptions: Vec<(SmartMixVisualItemData, u64)> = Vec::new();

    for (i, (track_idx, seg_idx, segment, is_image)) in all_visual_segments.iter().enumerate() {
        if is_cancelled() || is_stale(inc_index) {
            return None;
        }

        let description = if *is_image {
            // Image: directly load and recognize
            recognize_image(
                &segment.metadata.path,
                &mut model,
                &tokenizer,
                &prompt,
                max_tokens,
                max_resolution,
            )
        } else {
            // Video: sample frames and recognize
            recognize_video(
                segment,
                &mut model,
                &tokenizer,
                &prompt,
                max_tokens,
                video_sample_fps,
                max_resolution,
            )
        };

        let duration_ms = segment.duration.as_millis() as i32;
        let original_duration_ms = segment.original_duration.as_millis() as u64;

        visual_descriptions.push((
            SmartMixVisualItemData {
                track_index: *track_idx as i32,
                segment_index: *seg_idx as i32,
                is_image: *is_image,
                filename: segment
                    .metadata
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                description: description.unwrap_or_default(),
                duration_ms,
            },
            original_duration_ms,
        ));

        let progress = (i + 1) as f32 / total_visual as f32;
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_mix();
            entry.progress_type = UISmartMixProgressType::RecognizingVisual;
            entry.progress = progress;
            global_store!(ui).set_video_editor_smart_mix(entry);
        });
    }

    Some(visual_descriptions)
}

fn recognize_image(
    path: &Path,
    model: &mut LfmVlModel,
    tokenizer: &LfmTokenizer,
    prompt: &str,
    max_tokens: usize,
    max_resolution: (u32, u32),
) -> Result<String, String> {
    let img = image::open(path).map_err(|e| format!("Failed to open image: {e}"))?;
    let img = resize_dynamic_image_if_needed(img, max_resolution);
    generate(model, tokenizer, &img, prompt, max_tokens)
        .map_err(|e| format!("VL inference failed: {e}"))
}

fn recognize_video(
    segment: &Arc<Segment>,
    model: &mut LfmVlModel,
    tokenizer: &LfmTokenizer,
    prompt: &str,
    max_tokens: usize,
    sample_fps: f32,
    max_resolution: (u32, u32),
) -> Result<String, String> {
    let video_meta = segment
        .metadata
        .first_video()
        .ok_or("No video stream in segment")?;

    let source_fps = video_meta.fps;
    if source_fps <= 0.0 || sample_fps <= 0.0 {
        return Err("Invalid FPS values".to_string());
    }

    // Calculate frame interval: how many source frames between samples
    let frame_interval = (source_fps / sample_fps).max(1.0) as usize;

    // Calculate total frames in this segment
    let start_frame = segment
        .source_frame_range()
        .map(|r| r.start.frame_index())
        .unwrap_or(0);
    let total_frames = segment.frame_count().unwrap_or(1);

    let mut descriptions: Vec<String> = Vec::new();
    let mut frame_idx = start_frame;

    while frame_idx < start_frame + total_frames {
        if let Ok(frames) = segment.extract_video(frame_idx, 1) {
            for video_image in frames {
                if let VideoImage::Image { buffer, .. } = video_image {
                    // Convert RgbaImage to DynamicImage
                    let img = image::DynamicImage::ImageRgba8(buffer);
                    let img = resize_dynamic_image_if_needed(img, max_resolution);
                    match generate(model, tokenizer, &img, prompt, max_tokens) {
                        Ok(desc) => {
                            if !desc.is_empty() {
                                descriptions.push(desc);
                            }
                        }
                        Err(e) => {
                            log::warn!("VL inference failed for frame {}: {e}", frame_idx);
                        }
                    }
                }
            }
        }

        frame_idx += frame_interval;
    }

    if descriptions.is_empty() {
        Ok(String::new())
    } else {
        Ok(descriptions.join("; "))
    }
}

fn ai_match_visuals(
    ui_weak: &Weak<AppWindow>,
    transcribed: &[SmartMixEntryData],
    visuals: &[(SmartMixVisualItemData, u64)],
    reuse_media: bool,
    sequential_match: bool,
    must_cover_all_audio: bool,
    inc_index: u64,
) -> anyhow::Result<Vec<AiMatchResult>> {
    let setting = config::all().ai_model.clone();
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        return Err(anyhow::anyhow!("AI model not configured"));
    }

    let multi_visual_rule = if reuse_media {
        "5. One audio segment can match multiple visual segments (e.g., a long narration with multiple images)."
    } else {
        "5. One audio segment can match multiple DIFFERENT visual segments, but each visual must be distinct (no repeating the same visual_index). A single visual can cover multiple consecutive audio segments as one continuous block."
    };
    let reuse_rule = if reuse_media {
        "6. A visual segment CAN be reused by multiple audio segments at different time offsets. The same visual_index may appear in multiple result entries."
    } else if must_cover_all_audio {
        "6. NO REUSE: Each visual_index can only appear ONCE in the result. Since all audio MUST be covered, each visual must cover consecutive audio segments from its starting position until the next visual is a better content match. If you find that visuals are clustered at the front of the audio timeline (e.g., all visuals assigned to the first few audio segments, leaving later segments uncovered), you MUST re-adjust: shorten earlier visuals' coverage and advance later visuals to cover later audio segments. Check whether later audio segments have better content matches with later visuals, and shift visual boundaries accordingly. The goal is: every audio segment is covered, and visuals are spread across the entire audio timeline, not crammed at the beginning."
    } else {
        "6. NO REUSE: Each visual segment can only appear ONCE in the result, as a single continuous time block. One visual can span across multiple consecutive audio segments (e.g., visual_index 0 covering audio 0, 1, 2 as one continuous entry). But the same visual_index MUST NOT appear in multiple separate/disconnected entries. Every visual_index in the output must be unique."
    };
    let passage_rule = if reuse_media {
        "9. IMPORTANT: Audio segments are chronologically ordered. Consecutive audio segments with the same or related topic form a continuous passage. When a continuous passage is discussing a topic, ALL segments in that passage should use the SAME visual. Do NOT switch visuals in the middle of a continuous passage just because a short segment has fewer words or a slightly different phrasing. Visual switches should only happen at genuine topic boundaries, not within a continuous flow of speech."
    } else if must_cover_all_audio {
        "9. Audio segments are chronologically ordered. Since every audio segment MUST be covered, each visual covers consecutive audio segments until the next visual is a better content match. IMPORTANT: If visuals end up clustered at the front of the audio (later audio segments have no visual), you must re-adjust — look at later audio segments and find where later visuals match better, then move visual boundaries forward so that later visuals cover later audio segments. Do NOT leave any audio segment uncovered."
    } else {
        "9. Audio segments are chronologically ordered. Consecutive audio segments about the same topic should use the SAME visual as one continuous block spanning those segments. But you MUST NOT reuse a visual that has already been used in a previous separate block — each visual_index can only appear once in the output. Choose the next best matching unused visual for new topic blocks."
    };
    let sequential_rule = if sequential_match {
        if reuse_media {
            "10. SEQUENTIAL MATCH: Visual segments must be matched to audio segments in order. The first audio segment must use the first available visual, the second audio segment must use the next visual, and so on. Do not skip or reorder visuals."
        } else if must_cover_all_audio {
            "10. SEQUENTIAL MATCH WITH NO REUSE AND MUST COVER: Assign visuals to audio segments in order. Each visual covers consecutive audio segments until the next visual is a better content match. CRITICAL: If visuals are clustered at the front of the audio (all consumed by early segments, leaving later segments uncovered), you MUST re-adjust — check later audio segments for better matches with later visuals, and shift visual boundaries so that each visual covers a portion of the later audio too. The entire audio timeline must be covered with no gaps. Do NOT assign one visual per audio segment at the front."
        } else {
            "10. SEQUENTIAL MATCH WITH NO REUSE: Assign visuals to audio segments strictly in order — audio 0 gets visual 0, audio 1 gets visual 1, audio 2 gets visual 2, etc. Each visual is consumed after use and cannot be assigned again. If consecutive audio segments share a topic, they can share the same visual as one continuous block (consuming that visual once). If there are more audio segments than visuals, leave the remaining audio segments unmatched."
        }
    } else {
        "10. Visual segments can be matched to audio segments in any order based on semantic relevance."
    };
    let cover_rule = if must_cover_all_audio {
        if reuse_media {
            "11. MUST COVER ALL AUDIO: Every audio segment must be matched with at least one visual segment. You cannot leave any audio segment without a visual match. If there are not enough visuals, reuse visuals to cover all audio segments."
        } else {
            "11. MUST COVER ALL AUDIO (HIGHEST PRIORITY): The ENTIRE audio timeline from the first audio segment to the last MUST be covered by visuals with no gaps. This rule overrides all others. If your initial matching leaves later audio segments uncovered (visuals clustered at the front), you MUST re-adjust: look at later audio segments, find where later visuals match better, and move visual boundaries so that visuals are spread across the entire audio timeline. Zero uncovered segments is mandatory."
        }
    } else {
        "11. Audio segments without a good visual match can be left unmatched."
    };
    let prompt = format!(
        r##"You are a smart video editing assistant. Given transcribed audio segments and visual (image/video) descriptions, create the best matching plan to produce a mixed video.

Rules:
1. Match audio content with the most semantically relevant visual content.
2. You do NOT need to match every visual segment. Only use visuals that are semantically relevant to the audio. If a visual does not fit any audio, omit it.
3. Video segments have a maximum duration limited by their original_duration_ms. The source_offset_ms + duration_ms for a video must not exceed original_duration_ms.
4. Image segments can be stretched to any duration (duration_ms can be any value).
{multi_visual_rule}
{reuse_rule}
7. The total matched duration for each audio segment should approximately equal (end_ms - start_ms).
8. Sort results by audio_index, then by source_offset_ms.
{passage_rule}
{sequential_rule}
{cover_rule}

CRITICAL CONSTRAINT CHECK: Before outputting, verify that every visual_index in your result array is unique (no duplicate visual_index values). If you find duplicates, remove the weaker match and reassign to an unused visual or leave unmatched.

Only output the JSON array, no additional text.

<Input format>
{{
  "audio_segments": [{{"index": 0, "start_ms": 0, "end_ms": 5000, "text": "..."}}, ...],
  "visual_segments": [{{"index": 0, "description": "...", "duration_ms": 10000, "is_image": true, "original_duration_ms": 10000}}, ...]
}}
</Input format>

<Output format>
[{{"audio_index": 0, "visual_index": 0, "source_offset_ms": 0, "duration_ms": 5000}}, ...]
</Output format>
"##
    );

    let input = serde_json::json!({
        "audio_segments": transcribed.iter().enumerate().map(|(i, seg)| AiAudioSegment {
            index: i,
            start_ms: seg.start_ms as u64,
            end_ms: seg.end_ms as u64,
            text: seg.text.clone(),
        }).collect::<Vec<_>>(),
        "visual_segments": visuals.iter().enumerate().map(|(i, (vis, orig_dur))| AiVisualSegment {
            index: i,
            description: vis.description.clone(),
            duration_ms: *orig_dur,
            is_image: vis.is_image,
        }).collect::<Vec<_>>(),
    });

    let question = serde_json::to_string(&input)?;

    // We're on a std::thread (no tokio runtime), so create one for the AI matching.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create tokio runtime: {e}"))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
    let request_config = APIConfig {
        api_base_url: setting.api_base_url,
        api_model: setting.model_name,
        api_key: setting.api_key,
        temperature: None,
    };

    let ui_weak_clone = ui_weak.clone();
    rt.spawn(async move {
        let chat_config = ChatConfig { tx };
        let chat = Chat::new(prompt, question, chat_config, request_config, vec![]);
        if let Err(e) = chat.start().await {
            toast::async_toast_warn(
                ui_weak_clone,
                format!("{}: {e}", tr("Start AI matching failed")),
            );
        }
    });

    // Collect streaming response on our single-thread runtime
    let resp = rt.block_on(async {
        let mut resp = String::new();
        while let Some(item) = rx.recv().await {
            if is_cancelled() || is_stale(inc_index) {
                return String::new();
            }
            if let Some(text) = item.text {
                resp.push_str(&text);
            }
        }
        resp
    });

    let resp = resp
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    log::debug!("AI match response: {resp}");

    let results: Vec<AiMatchResult> = serde_json::from_str(&resp).map_err(|e| {
        anyhow::anyhow!("Failed to parse AI response as JSON: {e}. Response: {resp}")
    })?;

    // Merge consecutive entries that should be a single segment.
    // - Image: same visual_index + same source_offset_ms → merge (images can stretch)
    // - Video: same visual_index + contiguous source_offset → merge
    let mut merged: Vec<AiMatchResult> = Vec::new();
    for result in results {
        if let Some(last) = merged.last_mut()
            && last.visual_index == result.visual_index
        {
            let is_image = visuals
                .get(result.visual_index)
                .map(|(v, _)| v.is_image)
                .unwrap_or(false);

            if is_image && last.source_offset_ms == result.source_offset_ms {
                last.duration_ms += result.duration_ms;
                continue;
            }

            if !is_image && last.source_offset_ms + last.duration_ms == result.source_offset_ms {
                last.duration_ms += result.duration_ms;
                continue;
            }
        }
        merged.push(result);
    }

    Ok(merged)
}

fn video_editor_smart_mix_cancel(ui: &AppWindow) {
    if let Some(ref stop_sig) = SMART_MIX_CACHE.lock().unwrap().stop_sig {
        stop_sig.store(true, Ordering::Relaxed);
    }
    bump_inc_index();

    let mut entry = global_store!(ui).get_video_editor_smart_mix();
    entry.progress_type = UISmartMixProgressType::Cancelled;
    global_store!(ui).set_video_editor_smart_mix(entry);
}

fn video_editor_smart_mix_retry_ai_matching(ui: &AppWindow) {
    reset_smart_mix_stop_sig();

    let entry = global_store!(ui).get_video_editor_smart_mix();
    let audio_entries = store_video_editor_smart_mix_entries!(entry);
    let transcribed: Vec<SmartMixEntryData> = audio_entries.iter().map(|e| e.into()).collect();

    let entry = global_store!(ui).get_video_editor_smart_mix();
    let visual_items = store_video_editor_smart_mix_visual_items!(entry);
    let visuals: Vec<(SmartMixVisualItemData, u64)> = visual_items
        .iter()
        .map(|v| {
            let data: SmartMixVisualItemData = v.into();
            let orig_dur = data.duration_ms as u64; // use duration_ms as original_duration_ms
            (data, orig_dur)
        })
        .collect();

    if transcribed.is_empty() || visuals.is_empty() {
        toast_warn!(
            ui,
            tr("No audio or visual data to retry. Please run smart mix first.")
        );
        return;
    }

    // Clear match items and set progress
    let mut entry = global_store!(ui).get_video_editor_smart_mix();
    entry.match_items = ModelRc::new(VecModel::from_slice(&[]));
    entry.progress_type = UISmartMixProgressType::AiMatching;
    entry.progress = 0.0;
    global_store!(ui).set_video_editor_smart_mix(entry);

    let inc_index = bump_inc_index();
    let setting = global_store!(ui).get_video_editor_smart_mix_setting();
    let ui_weak = ui.as_weak();

    std::thread::spawn(move || {
        let match_results = match ai_match_visuals(
            &ui_weak,
            &transcribed,
            &visuals,
            setting.reuse_media,
            setting.sequential_match,
            setting.must_cover_all_audio,
            inc_index,
        ) {
            Ok(results) => results,
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("AI matching failed")));
                return;
            }
        };

        if is_cancelled() || is_stale(inc_index) {
            return;
        }

        let match_items: Vec<SmartMixMatchItemData> = match_results
            .iter()
            .map(|m| {
                let filename = visuals
                    .get(m.visual_index)
                    .map(|(v, _)| v.filename.clone())
                    .unwrap_or_default();
                SmartMixMatchItemData {
                    audio_index: m.audio_index as i32,
                    visual_index: m.visual_index as i32,
                    source_offset_ms: m.source_offset_ms as i32,
                    duration_ms: m.duration_ms as i32,
                    filename,
                }
            })
            .collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if is_stale(inc_index) {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_mix();
            let items = store_video_editor_smart_mix_match_items!(entry);
            for item in &match_items {
                items.push(item.clone().into());
            }
            entry.progress_type = UISmartMixProgressType::Finished;
            entry.progress = 1.0;
            global_store!(ui).set_video_editor_smart_mix(entry);
        });
    });
}

fn video_editor_smart_mix_apply(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_smart_mix();
    let match_items = store_video_editor_smart_mix_match_items!(entry);

    if match_items.row_count() == 0 {
        toast_warn!(ui, tr("No match results to apply"));
        return;
    }

    // Collect match data
    let match_data: Vec<(i32, i32, i32, i32)> = match_items
        .iter()
        .map(|m| {
            (
                m.audio_index,
                m.visual_index,
                m.source_offset_ms,
                m.duration_ms,
            )
        })
        .collect();

    // Collect visual info
    let entry = global_store!(ui).get_video_editor_smart_mix();
    let visual_items = store_video_editor_smart_mix_visual_items!(entry);
    let visual_data: Vec<(i32, i32, bool, i32)> = visual_items
        .iter()
        .map(|v| (v.track_index, v.segment_index, v.is_image, v.duration_ms))
        .collect();

    // Collect audio info
    let entry = global_store!(ui).get_video_editor_smart_mix();
    let audio_entries = store_video_editor_smart_mix_entries!(entry);
    let audio_data: Vec<(i32, i32)> = audio_entries
        .iter()
        .map(|a| (a.start_ms, a.end_ms))
        .collect();

    // First, collect all original segment metadata we'll need
    let original_segments: HashMap<(usize, usize), Arc<Segment>> = with_history_manager(|state| {
        let mut result = HashMap::new();
        for visual_idx in match_data.iter().map(|(_, vi, _, _)| *vi) {
            if let Some(&(track_idx, seg_idx, _, _)) = visual_data.get(visual_idx as usize) {
                let key = (track_idx as usize, seg_idx as usize);
                if !result.contains_key(&key)
                    && let Some(track) = state.tracks_manager.get(key.0)
                    && let Some(segment) = track.segments().get(key.1)
                {
                    result.insert(key, segment.clone());
                }
            }
        }
        result
    });

    let result = with_history_manager(|state| {
        let mut batch_command = BatchCommand::new("Smart mix apply".to_string());

        let mut visual_track_indices: BTreeSet<usize> = BTreeSet::new();
        for &(track_idx, _, _, _) in &visual_data {
            visual_track_indices.insert(track_idx as usize);
        }

        // Remove all existing segments from visual tracks (in reverse order)
        for &track_idx in &visual_track_indices {
            if let Some(track) = state.tracks_manager.get(track_idx) {
                let seg_count = track.segments().len();
                for i in (0..seg_count).rev() {
                    batch_command.add_command(Box::new(RemoveSegmentCommand::new(
                        track_idx, i, false, // don't shift timeline, we'll re-add all
                    )));
                    batch_command.add_extra_affected_segment(
                        AffectedSegment::with_both_thumbnails(track_idx, i),
                    );
                }
            }
        }

        // Build a timeline-ordered list by computing each entry's timeline position.
        // match_data is already sorted by audio_index then source_offset_ms (from AI output).
        // Merging was already done in ai_match_visuals, so each entry is a distinct segment.
        let mut audio_offset_accum: HashMap<i32, u64> = HashMap::new(); // audio_idx -> accumulated duration
        for &(audio_idx, visual_idx, source_offset_ms, duration_ms) in &match_data {
            let audio_start_ms = audio_data
                .get(audio_idx as usize)
                .map(|(s, _)| *s)
                .unwrap_or(0) as u64;

            let offset_in_audio = audio_offset_accum.entry(audio_idx).or_insert(0);
            let timeline_start_ms = audio_start_ms + *offset_in_audio;
            *offset_in_audio += duration_ms as u64;

            if let Some(&(track_idx, seg_idx, is_image, _)) = visual_data.get(visual_idx as usize) {
                let key = (track_idx as usize, seg_idx as usize);
                if let Some(orig_seg) = original_segments.get(&key) {
                    let duration = Duration::from_millis(duration_ms as u64);
                    let timeline_offset = Duration::from_millis(timeline_start_ms);
                    let source_offset = if is_image {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(source_offset_ms as u64)
                    };

                    let new_segment = Arc::new(Segment::new_with_source_offset(
                        timeline_offset,
                        source_offset,
                        duration,
                        1.0, // playback_speed
                        orig_seg.global_speed,
                        orig_seg.metadata.clone(),
                    ));

                    batch_command.add_command(Box::new(AddSegmentCommand::new(
                        track_idx as usize,
                        new_segment,
                    )));
                }
            }
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            toast_success!(ui, tr("Smart mix applied successfully"));
        }
        Err(e) => toast_warn!(ui, format!("{}: {e}", tr("Failed to apply smart mix"))),
    }
}

fn video_editor_smart_mix_remove_all(ui: &AppWindow) {
    let mut entry = global_store!(ui).get_video_editor_smart_mix();
    entry.audio_entries = ModelRc::new(VecModel::from_slice(&[]));
    entry.visual_items = ModelRc::new(VecModel::from_slice(&[]));
    entry.match_items = ModelRc::new(VecModel::from_slice(&[]));
    entry.progress = 0.0;
    entry.progress_type = UISmartMixProgressType::None;
    global_store!(ui).set_video_editor_smart_mix(entry);
}

fn video_editor_smart_mix_choose_vl_model_dir(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let index = index as usize;

    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose VL model directory"))
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_smart_mix_setting();
            let dir: slint::SharedString = dirpath.to_string_lossy().to_string().into();
            set_model_dir_by_index(&mut setting, index, dir);
            global_store!(ui).set_video_editor_smart_mix_setting(setting.clone());
            save_smart_mix_config(SmartMixConfigData::from(setting));
        });
    });
}

fn video_editor_smart_mix_update_config(ui: &AppWindow, config: UISmartMixSetting) {
    global_store!(ui).set_video_editor_smart_mix_setting(config.clone());
    save_smart_mix_config(SmartMixConfigData::from(config));
}

fn video_editor_smart_mix_model_start_download(ui: &AppWindow, index: i32) {
    let index = index as usize;
    if index >= 4 {
        return;
    }

    let precision = match index {
        0 => Precision::Fp32,
        1 => Precision::Fp16,
        2 => Precision::Q4,
        3 => Precision::Q8,
        _ => Precision::Fp16,
    };

    // Add shared config files (saved to model root, not onnx/)
    let shared_filenames = Precision::shared_config_filenames();
    let shared_urls = Precision::shared_config_urls();
    let config_urls: Vec<(String, String, bool)> = shared_urls
        .into_iter()
        .zip(shared_filenames.iter().map(|f| f.to_string()))
        .map(|(url, name)| (url, name, false)) // false = save to model root
        .collect();

    // Get download URLs and corresponding filenames (includes .onnx_data files)
    let download_urls = precision.download_urls();
    let filenames = precision.all_download_filenames();
    let onnx_urls: Vec<(String, String, bool)> = download_urls
        .into_iter()
        .zip(filenames.iter().map(|f| f.to_string()))
        .map(|(url, name)| (url, name, true)) // true = save to onnx/ subdirectory
        .collect();

    let urls: Vec<(String, String, bool)> = config_urls.into_iter().chain(onnx_urls).collect();
    let ui_weak = ui.as_weak();
    let precision_name = precision.to_name().to_string();

    // Set up download cancel signal for this precision
    {
        let mut cache = SMART_MIX_CACHE.lock().unwrap();
        if let Some(sig) = cache.download_stop_sigs.get(&index) {
            sig.store(true, Ordering::Relaxed);
        }
        cache
            .download_stop_sigs
            .insert(index, Arc::new(AtomicBool::new(false)));
    }
    let cancel_sig = SMART_MIX_CACHE
        .lock()
        .unwrap()
        .download_stop_sigs
        .get(&index)
        .cloned()
        .unwrap();

    tokio::spawn(async move {
        let Some(base_dir) = picker_directory(ui_weak.clone(), &tr("Choose model save directory"))
        else {
            return;
        };

        // Create model subdirectory: base_dir/LFM2.5-VL-450M-{precision}/
        let model_dir_name = format!("LFM2.5-VL-450M-{precision_name}");
        let model_dir = base_dir.join(&model_dir_name);
        if let Err(e) = std::fs::create_dir_all(&model_dir) {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Failed to create directory")),
            );
            return;
        }

        // Create onnx subdirectory inside model dir
        let onnx_dir = model_dir.join("onnx");
        if let Err(e) = std::fs::create_dir_all(&onnx_dir) {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Failed to create directory")),
            );
            return;
        }

        let total_files = urls.len();

        // Set model dir in settings
        let model_dir_str = model_dir.to_string_lossy().to_string();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_smart_mix_setting();
            set_model_dir_by_index(&mut setting, index, model_dir_str.into());
            global_store!(ui).set_video_editor_smart_mix_setting(setting.clone());
            save_smart_mix_config(SmartMixConfigData::from(setting));
        });

        for (file_index, (url, filename, is_onnx)) in urls.iter().enumerate() {
            // Check cancel signal before each file
            if cancel_sig.load(Ordering::Relaxed) {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if let Some(mut item) =
                        store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
                    {
                        item.state = UIDownloaderState::Cancelled;
                        store_video_editor_smart_mix_models_downloader!(ui)
                            .set_row_data(index, item);
                    }
                });
                return;
            }

            let save_path = if *is_onnx {
                onnx_dir.join(filename)
            } else {
                model_dir.join(filename)
            };

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if let Some(mut item) =
                    store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
                {
                    item.state = UIDownloaderState::Downloading;
                    item.progress = file_index as f32 / total_files as f32;
                    store_video_editor_smart_mix_models_downloader!(ui).set_row_data(index, item);
                }
            });

            let dl = downloader::Downloader::new(url.clone(), save_path.clone());
            // Connect cancel signal to downloader
            let dl_cancel = dl.cancel_sig();
            let cancel_sig_clone = cancel_sig.clone();
            // Spawn a task that watches our cancel signal and triggers the downloader's cancel
            let cancel_handle = tokio::spawn(async move {
                // Poll our cancel signal
                while !cancel_sig_clone.load(Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                dl_cancel.store(true, Ordering::Relaxed);
            });

            let result = dl
                .start(|_downloaded: u64, _total: u64, _progress: f32| {})
                .await;

            cancel_handle.abort();

            match result {
                Ok(DownloadState::Finsished) => {
                    let progress = (file_index + 1) as f32 / total_files as f32;
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
                        {
                            item.progress = progress;
                            store_video_editor_smart_mix_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                    });
                }
                Ok(DownloadState::Cancelled) => {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
                        {
                            item.state = UIDownloaderState::Cancelled;
                            store_video_editor_smart_mix_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                    });
                    return;
                }
                Ok(DownloadState::Incompleted) | Err(_) => {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
                        {
                            item.state = UIDownloaderState::Failed;
                            store_video_editor_smart_mix_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                    });
                    return;
                }
            }
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Some(mut item) =
                store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Finished;
                item.progress = 1.0;
                store_video_editor_smart_mix_models_downloader!(ui).set_row_data(index, item);
            }
            toast_success!(ui, tr("Model downloaded successfully"));
        });
    });
}

fn video_editor_smart_mix_model_cancel_download(ui: &AppWindow, index: i32) {
    {
        let cache = SMART_MIX_CACHE.lock().unwrap();
        if let Some(sig) = cache.download_stop_sigs.get(&(index as usize)) {
            sig.store(true, Ordering::Relaxed);
        }
    }

    let index = index as usize;
    if let Some(mut item) = store_video_editor_smart_mix_models_downloader!(ui).row_data(index)
        && item.state == UIDownloaderState::Downloading
    {
        item.state = UIDownloaderState::Cancelled;
        store_video_editor_smart_mix_models_downloader!(ui).set_row_data(index, item);
    }
}

fn get_model_dir(config: &UISmartMixSetting) -> String {
    get_model_dir_by_index(config, precision_to_index(&config.vl_precision) as usize)
}

fn get_model_dir_by_index(config: &UISmartMixSetting, index: usize) -> String {
    match index {
        0 => config.vl_fp32_model_dir.to_string(),
        1 => config.vl_fp16_model_dir.to_string(),
        2 => config.vl_q4_model_dir.to_string(),
        3 => config.vl_q8_model_dir.to_string(),
        _ => config.vl_fp16_model_dir.to_string(),
    }
}

fn set_model_dir_by_index(config: &mut UISmartMixSetting, index: usize, dir: SharedString) {
    match index {
        0 => config.vl_fp32_model_dir = dir,
        1 => config.vl_fp16_model_dir = dir,
        2 => config.vl_q4_model_dir = dir,
        3 => config.vl_q8_model_dir = dir,
        _ => config.vl_fp16_model_dir = dir,
    }
}

fn parse_resolution(resolution: &str) -> (u32, u32) {
    match resolution {
        "480p" => (854, 480),
        "720p" => (1280, 720),
        "1080p" => (1920, 1080),
        _ => (0, 0), // "origin" — no resizing
    }
}

fn resize_dynamic_image_if_needed(
    img: image::DynamicImage,
    max_resolution: (u32, u32),
) -> image::DynamicImage {
    let (max_w, max_h) = max_resolution;
    if max_w == 0 || max_h == 0 {
        return img;
    }

    let (w, h) = (img.width(), img.height());
    if w <= max_w && h <= max_h {
        return img;
    }

    let rgba_img = img.to_rgba8();
    match resize_rgba_image_contain(rgba_img, max_w, max_h, false) {
        Ok(resized) => ImageRgba8(resized),
        Err(_) => img, // fallback: return original on error
    }
}

fn save_smart_mix_config(config: SmartMixConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize smart mix config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, SMART_MIX_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, SMART_MIX_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save smart mix config: {:?}", e);
            }
        }
    });
}

async fn load_smart_mix_config() -> Option<SmartMixConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, SMART_MIX_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}
