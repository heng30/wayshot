use super::audio_player;
use crate::{
    config, global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        share_screen::picker_file,
        toast,
        tr::tr,
        video_editor::{
            command::{sync_and_refresh, with_history_manager},
            track::is_track_locked,
            transcribe::save_transcribe_config,
            vad::{detect_voice_segments, to_mono},
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        SmartClip as UISmartClip, SmartClipEntry as UISmartClipEntry,
        SmartClipProgressType as UISmartClipProgressType,
    },
    toast_info, toast_success, toast_warn,
};
use audio_utils::{
    loader::AudioConfig,
    vad::{VadConfig, trim_leading_silence},
};
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use fun_ast_nano::{FunASRModelConfig, FunAsrError, FunAsrNanoGenerateModel};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
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
        segment::{RemoveSegmentCommand, SplitSegmentCommand},
    },
    tracks::audio_track::extract_segment_audio,
};

const DEFAULT_PROMPT: &str = "Transcribe audio to text.";

#[derive(Default)]
struct SmartClipCache {
    stop_sig: Option<Arc<AtomicBool>>,
    ai_split_stop_sig: Option<Arc<AtomicBool>>,
    inc_index: u64,
    audio_config: Option<AudioConfig>,
}

static SMART_CLIP_CACHE: Lazy<Mutex<SmartClipCache>> =
    Lazy::new(|| Mutex::new(SmartClipCache::default()));

#[macro_export]
macro_rules! store_video_editor_smart_clip_entries {
    ($smart_clip: expr) => {
        $smart_clip
            .entries
            .as_any()
            .downcast_ref::<VecModel<UISmartClipEntry>>()
            .expect("We know we set a VecModel<UISmartClipEntry> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_smart_clip_models_dowloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_smart_clip_models_dowloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for smart clip models")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_smart_clip_start, ui);
    logic_cb!(video_editor_smart_clip_cancel, ui, ty);
    logic_cb!(video_editor_smart_clip_toggle_entry, ui, index);
    logic_cb!(video_editor_smart_clip_toggle_all, ui, kept);
    logic_cb!(video_editor_smart_clip_remove_unkept, ui);
    logic_cb!(video_editor_smart_clip_ai_split, ui);
    logic_cb!(video_editor_smart_clip_ai_split_with_reference, ui, text);
    logic_cb!(video_editor_smart_clip_play, ui);
    logic_cb!(video_editor_smart_clip_play_entry, ui, index);
    logic_cb!(video_editor_smart_clip_play_from_progress, ui, progress);
    logic_cb!(video_editor_smart_clip_stop, ui);
    logic_cb!(video_editor_smart_clip_remove_all, ui);
    logic_cb!(video_editor_smart_clip_apply, ui);
    logic_cb_pure!(video_editor_smart_clip_setting_is_valid, ui);
    logic_cb!(video_editor_smart_clip_choose_model_path, ui, index);
    logic_cb!(video_editor_smart_clip_model_start_download, ui, index, url);
    logic_cb!(
        video_editor_smart_clip_model_cancel_download,
        ui,
        index,
        url
    );
    logic_cb!(
        video_editor_smart_clip_audio_player_sound_changed,
        ui,
        sound
    );
    logic_cb!(
        video_editor_smart_clip_audio_player_sound_released,
        ui,
        sound
    );
    logic_cb!(
        video_editor_smart_clip_audio_player_speed_changed,
        ui,
        speed
    );
    logic_cb!(
        video_editor_smart_clip_audio_player_progress_pressed,
        ui,
        value
    );
    logic_cb!(
        video_editor_smart_clip_audio_player_progress_released,
        ui,
        value
    );
}

fn inner_init(ui: &AppWindow) {
    audio_player::init(ui);

    let mut smart_clip = UISmartClip::default();
    smart_clip.entries = ModelRc::new(VecModel::from_slice(&[]));
    global_store!(ui).set_video_editor_smart_clip(smart_clip);

    let downloaders = fun_ast_nano::Model::all_models()
        .into_iter()
        .map(|m| UIDownloader {
            url: m.download_url().to_string().into(),
            filename: m.to_filename().to_string().into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect::<Vec<_>>();
    store_video_editor_smart_clip_models_dowloader!(ui).set_vec(downloaders);
}

fn reset_smart_clip_stop_sig() {
    let mut cache = SMART_CLIP_CACHE.lock().unwrap();
    if let Some(stop_sig) = cache.stop_sig.take() {
        stop_sig.store(true, Ordering::Relaxed);
    }
    cache.stop_sig = Some(Arc::new(AtomicBool::new(false)));
}

fn video_editor_smart_clip_start(ui: &AppWindow) {
    reset_smart_clip_stop_sig();

    if !video_editor_smart_clip_setting_is_valid(ui) {
        global_store!(ui).set_video_editor_smart_clip_is_show_setting_dialog(true);
        return;
    }

    global_store!(ui).set_video_editor_smart_clip_playing_index(-1);
    let track_idx = global_store!(ui).get_video_editor_smart_clip_segment_track_index() as usize;
    let seg_idx = global_store!(ui).get_video_editor_smart_clip_segment_index() as usize;

    if is_track_locked(ui, track_idx as i32) {
        toast_warn!(ui, tr("Cannot segment in a locked track"));
        return;
    }

    let segment = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if seg_idx < track.segments().len() {
                Some(track.segments()[seg_idx].clone())
            } else {
                None
            }
        })
    });

    let Some(segment) = segment else {
        toast_warn!(ui, tr("Segment not found"));
        return;
    };

    if segment.metadata.audios.is_empty() {
        toast_warn!(ui, tr("Segment has no audio"));
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_smart_clip();
    entry.progress = 0.0;
    entry.progress_type = UISmartClipProgressType::ImportingAudio;
    entry.entries = ModelRc::new(VecModel::from_slice(&[]));
    entry.media_duration_ms = segment.duration.as_millis() as f32;
    global_store!(ui).set_video_editor_smart_clip(entry);
    global_store!(ui).set_video_editor_smart_clip_audio_player_progress(0.0);
    global_store!(ui).set_video_editor_smart_clip_audio_player_is_playing(false);

    toast_info!(ui, tr("Importing audio, please wait..."));

    let setting = global_store!(ui).get_video_editor_transcribe_setting();
    let ui_weak = ui.as_weak();
    let stop_sig = {
        let cache = SMART_CLIP_CACHE.lock().unwrap();
        cache.stop_sig.clone()
    };
    let inc_index = {
        let mut cache = SMART_CLIP_CACHE.lock().unwrap();
        cache.inc_index += 1;
        cache.inc_index
    };

    std::thread::spawn(move || {
        // Step 1: Extract audio from segment
        let audio_meta = segment.metadata.audios.first().unwrap();
        let samples_result = extract_segment_audio(
            &segment.metadata.path,
            audio_meta.index,
            &segment,
            segment.timeline_offset,
            segment.duration,
            audio_meta.channels,
            audio_meta.sample_rate,
            fun_ast_nano::INPUT_AUDIO_CHANNELS as u16,
            fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE,
        );

        let segment_samples = match samples_result {
            Ok(s) => s,
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("Audio extraction failed")));
                return;
            }
        };

        let raw_samples: Vec<f32> = segment_samples
            .samples
            .into_iter()
            .map(|s| s.unwrap_or(0.0))
            .collect();

        if raw_samples.is_empty() {
            toast::async_toast_warn(ui_weak, tr("No audio data available"));
            return;
        }

        if let Some(ref sig) = stop_sig
            && sig.load(Ordering::Relaxed)
        {
            return;
        }

        // Step 2: VAD - detect voice segments
        let mono = to_mono(&raw_samples, fun_ast_nano::INPUT_AUDIO_CHANNELS as u16);
        let speech_regions =
            match detect_voice_segments(&mono, fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE) {
                Ok(regions) => regions,
                Err(e) => {
                    toast::async_toast_warn(
                        ui_weak,
                        format!("{}: {e}", tr("VAD detection failed")),
                    );
                    return;
                }
            };

        if speech_regions.is_empty() {
            toast::async_toast_warn(ui_weak, tr("No speech detected"));
            return;
        }

        if let Some(ref sig) = stop_sig
            && sig.load(Ordering::Relaxed)
        {
            return;
        }

        let keep_leading_silence_ms = setting.keep_leading_silence_ms.max(0) as u64;
        let extended_regions: Vec<(u64, u64)> = speech_regions
            .iter()
            .enumerate()
            .map(|(i, &(start_ms, end_ms))| {
                let prev_end = if i > 0 { speech_regions[i - 1].1 } else { 0 };
                let extended_start = start_ms
                    .saturating_sub(keep_leading_silence_ms)
                    .max(prev_end);
                (extended_start, end_ms)
            })
            .collect();

        // Trim fine-grained leading silence within each extended region,
        // but preserve at most keep_leading_silence_ms of silence from the speech onset.
        let mut trimmed_regions: Vec<(u64, u64)> = Vec::new();
        let vad_config =
            VadConfig::default().with_sample_rate(fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE);

        for (start_ms, end_ms) in &extended_regions {
            let start_sample = (*start_ms as f64 / 1000.0
                * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
                .round() as usize;
            let end_sample = (*end_ms as f64 / 1000.0
                * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
                .round() as usize;

            let end_sample = end_sample.min(mono.len());
            if start_sample >= end_sample {
                trimmed_regions.push((*start_ms, *end_ms));
                continue;
            }

            let segment_audio = &mono[start_sample..end_sample];
            let new_start = trim_leading_silence(
                segment_audio,
                *start_ms,
                &vad_config,
                keep_leading_silence_ms,
            );

            trimmed_regions.push((new_start, *end_ms));
        }

        if let Some(ref sig) = stop_sig
            && sig.load(Ordering::Relaxed)
        {
            return;
        }

        // Step 3: Store AudioConfig for playback
        let duration = Duration::from_secs_f64(
            raw_samples.len() as f64
                / (fun_ast_nano::INPUT_AUDIO_CHANNELS as f64
                    * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64),
        );
        let audio_config = AudioConfig::default()
            .with_sample_rate(fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE)
            .with_channel(fun_ast_nano::INPUT_AUDIO_CHANNELS as u16)
            .with_duration(duration)
            .with_samples(raw_samples);

        {
            let mut cache = SMART_CLIP_CACHE.lock().unwrap();
            cache.audio_config = Some(audio_config.clone());
        }

        // Update progress: transcribing
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != SMART_CLIP_CACHE.lock().unwrap().inc_index {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_clip();
            entry.progress_type = UISmartClipProgressType::Transcribing;
            entry.progress = 0.0;
            global_store!(ui).set_video_editor_smart_clip(entry);
        });

        if let Some(ref sig) = stop_sig
            && sig.load(Ordering::Relaxed)
        {
            return;
        }

        // Step 4: Load model and transcribe each segment
        let model_config = FunASRModelConfig::default()
            .with_model_weights(setting.model_path.to_string())
            .with_tokenizer_path(setting.model_tokenizer_path.to_string());

        let mut model = match FunAsrNanoGenerateModel::new(model_config, None, None) {
            Ok(m) => m,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("Failed to load transcribe model")),
                );
                return;
            }
        };

        let total_segments = trimmed_regions.len();
        for (seg_idx, (start_ms, end_ms)) in trimmed_regions.iter().enumerate() {
            let start_ms = *start_ms;
            let end_ms = *end_ms;
            if let Some(ref sig) = stop_sig
                && sig.load(Ordering::Relaxed)
            {
                return;
            }

            // Check inc_index to ensure this isn't a stale operation
            if inc_index != SMART_CLIP_CACHE.lock().unwrap().inc_index {
                return;
            }

            // Extract audio samples for this segment
            let start_sample = (start_ms as f64 / 1000.0
                * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
                .round() as usize;
            let end_sample = (end_ms as f64 / 1000.0 * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
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
                .with_prompt(Some(DEFAULT_PROMPT.to_string()))
                .with_max_tokens(512);

            let result = model.generate(request, None, |_chunk| Ok(()));
            let text = match result {
                Ok(response) => response.text,
                Err(FunAsrError::TranscribeCancelled) => return,
                Err(e) => {
                    log::warn!("Transcribe segment {} failed: {e}", seg_idx);
                    String::new()
                }
            };

            // Convert traditional Chinese to simplified
            let text = fast2s::convert(&text);

            _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                if inc_index != SMART_CLIP_CACHE.lock().unwrap().inc_index {
                    return;
                }

                let ui_entry = UISmartClipEntry {
                    start_ms: start_ms as i32,
                    end_ms: end_ms as i32,
                    text: text.into(),
                    is_kept: true,
                };

                let mut entry = global_store!(ui).get_video_editor_smart_clip();
                entry.progress_type = UISmartClipProgressType::Transcribing;
                entry.progress = (seg_idx + 1) as f32 / total_segments as f32;
                store_video_editor_smart_clip_entries!(entry).push(ui_entry);
                global_store!(ui).set_video_editor_smart_clip(entry);
            });
        }

        if inc_index != SMART_CLIP_CACHE.lock().unwrap().inc_index {
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != SMART_CLIP_CACHE.lock().unwrap().inc_index {
                return;
            }
            let mut entry = global_store!(ui).get_video_editor_smart_clip();
            entry.progress_type = UISmartClipProgressType::Finished;
            entry.progress = 1.0;
            global_store!(ui).set_video_editor_smart_clip(entry);
        });
    });
}

fn video_editor_smart_clip_cancel(ui: &AppWindow, ty: UISmartClipProgressType) {
    match ty {
        UISmartClipProgressType::ImportingAudio | UISmartClipProgressType::Transcribing => {
            if let Some(ref stop_sig) = SMART_CLIP_CACHE.lock().unwrap().stop_sig {
                stop_sig.store(true, Ordering::Relaxed);
            }
            // Increment inc_index so stale transcription results are discarded
            {
                let mut cache = SMART_CLIP_CACHE.lock().unwrap();
                cache.inc_index += 1;
            }
        }
        UISmartClipProgressType::AiSplitting => {
            if let Some(ref stop_sig) = SMART_CLIP_CACHE.lock().unwrap().ai_split_stop_sig {
                stop_sig.store(true, Ordering::Relaxed);
            }
        }
        _ => {}
    }

    let mut entry = global_store!(ui).get_video_editor_smart_clip();
    entry.progress_type = UISmartClipProgressType::Cancelled;
    global_store!(ui).set_video_editor_smart_clip(entry);
}

fn video_editor_smart_clip_toggle_entry(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);

    if let Some(mut item) = entries.row_data(index) {
        item.is_kept = !item.is_kept;
        entries.set_row_data(index, item);
    }

    update_audio_duration(ui);
}

fn video_editor_smart_clip_toggle_all(ui: &AppWindow, kept: bool) {
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);

    let updated: Vec<UISmartClipEntry> = entries
        .iter()
        .map(|mut e| {
            e.is_kept = kept;
            e
        })
        .collect();

    entries.set_vec(updated);
    update_audio_duration(ui);
}

fn video_editor_smart_clip_remove_unkept(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);

    let kept: Vec<UISmartClipEntry> = entries.iter().filter(|e| e.is_kept).collect();

    entries.set_vec(kept);
    update_audio_duration(ui);
    toast_success!(ui, tr("Removed unchecked entries"));
}

#[derive(serde::Serialize)]
struct InputSegment {
    index: usize,
    text: String,
}

#[derive(serde::Deserialize)]
struct OutputSegment {
    index: usize,
    keep: bool,
}

#[derive(serde::Serialize)]
struct SmartSplitWithRefInput {
    reference: String,
    segments: Vec<InputSegment>,
}

fn video_editor_smart_clip_ai_split(ui: &AppWindow) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, tr("Please setup AI model and try again."));
        return;
    }

    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);
    let segments: Vec<(usize, String)> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.text.to_string()))
        .collect();

    if segments.is_empty() {
        toast_info!(ui, tr("No segments to split"));
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_smart_clip();
    entry.progress = 0.0;
    entry.progress_type = UISmartClipProgressType::AiSplitting;
    global_store!(ui).set_video_editor_smart_clip(entry);

    let stop_sig = Arc::new(AtomicBool::new(false));
    {
        let mut cache = SMART_CLIP_CACHE.lock().unwrap();
        if let Some(sig) = cache.stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        if let Some(sig) = cache.ai_split_stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.ai_split_stop_sig = Some(stop_sig.clone());
    }

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        match ai_smart_split(ui_weak.clone(), segments, stop_sig.clone()).await {
            Ok(results) => {
                if stop_sig.load(Ordering::Relaxed) {
                    return;
                }

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let entry = global_store!(ui).get_video_editor_smart_clip();
                    let entries = store_video_editor_smart_clip_entries!(entry);

                    let updated: Vec<UISmartClipEntry> = entries
                        .iter()
                        .enumerate()
                        .map(|(i, mut e)| {
                            if let Some(r) = results.get(&i) {
                                e.is_kept = *r;
                            }
                            e
                        })
                        .collect();

                    entries.set_vec(updated);

                    let mut entry = global_store!(ui).get_video_editor_smart_clip();
                    entry.progress_type = UISmartClipProgressType::Finished;
                    entry.progress = 1.0;
                    global_store!(ui).set_video_editor_smart_clip(entry);
                });
            }
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("AI smart split failed")));
            }
        }
    });
}

fn video_editor_smart_clip_ai_split_with_reference(ui: &AppWindow, text: SharedString) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, tr("Please setup AI model and try again"));
        return;
    }

    if text.is_empty() {
        toast_info!(ui, tr("Please enter reference text"));
        return;
    }

    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);
    let segments: Vec<(usize, String)> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.text.to_string()))
        .collect();

    if segments.is_empty() {
        toast_info!(ui, tr("No segments to split"));
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_smart_clip();
    entry.progress = 0.0;
    entry.progress_type = UISmartClipProgressType::AiSplitting;
    global_store!(ui).set_video_editor_smart_clip(entry);

    let stop_sig = Arc::new(AtomicBool::new(false));
    {
        let mut cache = SMART_CLIP_CACHE.lock().unwrap();
        if let Some(sig) = cache.stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        if let Some(sig) = cache.ai_split_stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.ai_split_stop_sig = Some(stop_sig.clone());
    }

    let ui_weak = ui.as_weak();
    let reference = text.to_string();
    tokio::spawn(async move {
        match ai_smart_split_with_reference(ui_weak.clone(), reference, segments, stop_sig.clone())
            .await
        {
            Ok(results) => {
                if stop_sig.load(Ordering::Relaxed) {
                    return;
                }

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let entry = global_store!(ui).get_video_editor_smart_clip();
                    let entries = store_video_editor_smart_clip_entries!(entry);

                    let updated: Vec<UISmartClipEntry> = entries
                        .iter()
                        .enumerate()
                        .map(|(i, mut e)| {
                            if let Some(r) = results.get(&i) {
                                e.is_kept = *r;
                            }
                            e
                        })
                        .collect();

                    entries.set_vec(updated);

                    let mut entry = global_store!(ui).get_video_editor_smart_clip();
                    entry.progress_type = UISmartClipProgressType::Finished;
                    entry.progress = 1.0;
                    global_store!(ui).set_video_editor_smart_clip(entry);
                });
            }
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("AI smart split with reference failed")),
                );
            }
        }
    });
}

async fn ai_smart_split(
    ui_weak: Weak<AppWindow>,
    segments: Vec<(usize, String)>,
    stop_sig: Arc<AtomicBool>,
) -> anyhow::Result<std::collections::HashMap<usize, bool>> {
    let prompt = r#"You are a voice clip assistant. Given a list of transcribed speech segments from a voiceover, decide which segments should be kept and which should be removed.

Remove segments that are:
- Filler words (um, uh, ah, etc.)
- Repetitions or stuttering
- Off-topic or irrelevant content
- False starts or incomplete sentences

Keep segments that contain meaningful, coherent content.

Only output the JSON array, no additional text.

<Input format>
[{"index": 0, "text": "segment text"}, {"index": 1, "text": "segment text"}, ...]
</Input format>

<Output format>
[{"index": 0, "keep": true}, {"index": 1, "keep": false}, ...]
</Output format>
"#;

    let input: Vec<InputSegment> = segments
        .into_iter()
        .map(|(index, text)| InputSegment { index, text })
        .collect();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
    let question = serde_json::to_string(&input)?;
    let model_config = config::all().ai_model.clone();
    let request_config = APIConfig {
        api_base_url: model_config.api_base_url,
        api_model: model_config.model_name,
        api_key: model_config.api_key,
        temperature: None,
    };

    tokio::spawn(async move {
        let chat_config = ChatConfig { tx };
        let chat = Chat::new(prompt, question, chat_config, request_config, vec![]);
        if let Err(e) = chat.start().await {
            toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("Start AI split failed")));
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if stop_sig.load(Ordering::Relaxed) {
            return Ok(std::collections::HashMap::new());
        }

        if let Some(ref text) = item.text {
            resp.push_str(text);
        }
    }

    let resp = resp
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    log::debug!("AI smart split response: {resp}");

    let output: Vec<OutputSegment> = serde_json::from_str(&resp).map_err(|e| {
        anyhow::anyhow!("Failed to parse AI response as JSON: {e}. Response: {resp}")
    })?;

    let results: std::collections::HashMap<usize, bool> = output
        .into_iter()
        .map(|item| (item.index, item.keep))
        .collect();

    Ok(results)
}

async fn ai_smart_split_with_reference(
    ui_weak: Weak<AppWindow>,
    reference: String,
    segments: Vec<(usize, String)>,
    stop_sig: Arc<AtomicBool>,
) -> anyhow::Result<std::collections::HashMap<usize, bool>> {
    let prompt = r#"You are a voice clip assistant. The user has provided a reference text (e.g., a script or outline). Given a list of transcribed speech segments, decide which segments should be kept based on whether they match the reference content.

Keep segments that contain content matching or closely related to the reference text.
Remove segments that are:
- Filler words, repetitions, or stuttering
- Off-topic content not related to the reference
- False starts or incomplete sentences

Only output the JSON array, no additional text.

<Input format>
{
  "reference": "The reference text provided by user...",
  "segments": [{"index": 0, "text": "transcribed text 0"}, {"index": 1, "text": "transcribed text 1"}, ...]
}
</Input format>

<Output format>
[{"index": 0, "keep": true}, {"index": 1, "keep": false}, ...]
</Output format>
"#;

    let input = SmartSplitWithRefInput {
        reference,
        segments: segments
            .into_iter()
            .map(|(index, text)| InputSegment { index, text })
            .collect(),
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
    let question = serde_json::to_string(&input)?;
    let model_config = config::all().ai_model.clone();
    let request_config = APIConfig {
        api_base_url: model_config.api_base_url,
        api_model: model_config.model_name,
        api_key: model_config.api_key,
        temperature: None,
    };

    tokio::spawn(async move {
        let chat_config = ChatConfig { tx };
        let chat = Chat::new(prompt, question, chat_config, request_config, vec![]);
        if let Err(e) = chat.start().await {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Start AI split with reference failed")),
            );
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if stop_sig.load(Ordering::Relaxed) {
            return Ok(std::collections::HashMap::new());
        }

        if let Some(ref text) = item.text {
            resp.push_str(text);
        }
    }

    let resp = resp
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    log::debug!("AI smart split with reference response: {resp}");

    let output: Vec<OutputSegment> = serde_json::from_str(&resp).map_err(|e| {
        anyhow::anyhow!("Failed to parse AI response as JSON: {e}. Response: {resp}")
    })?;

    let results: std::collections::HashMap<usize, bool> = output
        .into_iter()
        .map(|item| (item.index, item.keep))
        .collect();

    Ok(results)
}

fn collect_checked_segments(ui: &AppWindow) -> (Vec<(u64, u64)>, u64) {
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);

    let checked_segments: Vec<(u64, u64)> = entries
        .iter()
        .filter(|e| e.is_kept)
        .map(|e| (e.start_ms as u64, e.end_ms as u64))
        .collect();

    let total_duration_ms: u64 = checked_segments
        .iter()
        .map(|(s, e)| e.saturating_sub(*s))
        .sum();

    global_store!(ui).set_video_editor_smart_clip_audio_duration_ms(total_duration_ms as f32);

    (checked_segments, total_duration_ms)
}

fn collect_all_entries_info(ui: &AppWindow) -> Vec<audio_player::EntryInfo> {
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| (i, e.start_ms as u64, e.end_ms as u64, e.is_kept))
        .collect()
}

fn update_audio_duration(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);
    let total_duration_ms: u64 = entries
        .iter()
        .filter(|e| e.is_kept)
        .map(|e| (e.end_ms as u64).saturating_sub(e.start_ms as u64))
        .sum();
    global_store!(ui).set_video_editor_smart_clip_audio_duration_ms(total_duration_ms as f32);
}

// Find the segments and offset from a progress value (0-1) within the total concat timeline.
fn segments_from_progress(
    checked_segments: &[(u64, u64)],
    total_duration_ms: u64,
    progress: f32,
) -> (Vec<(u64, u64)>, u64) {
    if total_duration_ms == 0 {
        return (checked_segments.to_vec(), 0);
    }

    let target_ms = (progress.clamp(0.0, 1.0) as f64 * total_duration_ms as f64) as u64;

    let mut cumulative_ms: u64 = 0;
    let mut found = false;
    let mut segments: Vec<(u64, u64)> = Vec::new();

    for (start_ms, end_ms) in checked_segments {
        let seg_dur = end_ms.saturating_sub(*start_ms);
        let seg_end = cumulative_ms + seg_dur;

        if !found && seg_end > target_ms {
            // This segment contains the seek point
            let offset_in_seg = target_ms.saturating_sub(cumulative_ms);
            segments.push((start_ms + offset_in_seg, *end_ms));
            found = true;
        } else if found {
            segments.push((*start_ms, *end_ms));
        }

        cumulative_ms = seg_end;
    }

    if segments.is_empty() {
        (checked_segments.to_vec(), 0)
    } else {
        (segments, target_ms)
    }
}

fn video_editor_smart_clip_play(ui: &AppWindow) {
    let (checked_segments, total_duration_ms) = collect_checked_segments(ui);

    if checked_segments.is_empty() {
        toast_warn!(ui, tr("No segments selected to play"));
        return;
    }

    let audio_config = {
        let cache = SMART_CLIP_CACHE.lock().unwrap();
        match cache.audio_config.clone() {
            Some(config) => config,
            None => {
                toast_warn!(ui, tr("No audio data available"));
                return;
            }
        }
    };

    let all_entries = collect_all_entries_info(ui);
    let current_progress = global_store!(ui).get_video_editor_smart_clip_audio_player_progress();
    let progress = if current_progress >= 1.0 {
        0.0
    } else {
        current_progress
    };

    let (segments_from_seek, offset_ms) =
        segments_from_progress(&checked_segments, total_duration_ms, progress);

    audio_player::play_segments_from_offset(
        ui,
        &audio_config,
        &segments_from_seek,
        total_duration_ms,
        offset_ms,
        all_entries,
        true,
    );
}

fn video_editor_smart_clip_play_entry(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);

    let Some(item) = entries.row_data(index) else {
        toast_warn!(ui, tr("Segment not found"));
        return;
    };

    let single_segment = (item.start_ms as u64, item.end_ms as u64);
    let segment_duration_ms = item.end_ms as u64 - item.start_ms as u64;

    if segment_duration_ms == 0 {
        toast_warn!(ui, tr("Segment has no duration"));
        return;
    }

    let audio_config = {
        let cache = SMART_CLIP_CACHE.lock().unwrap();
        match cache.audio_config.clone() {
            Some(config) => config,
            None => {
                toast_warn!(ui, tr("No audio data available"));
                return;
            }
        }
    };

    audio_player::play_segments_from_offset(
        ui,
        &audio_config,
        &[single_segment],
        segment_duration_ms,
        0,
        vec![],
        false,
    );
}

fn video_editor_smart_clip_stop(ui: &AppWindow) {
    global_store!(ui).set_video_editor_smart_clip_audio_player_is_playing(false);
    global_store!(ui).set_video_editor_smart_clip_playing_index(-1);
    audio_player::stop_audio();
}

fn video_editor_smart_clip_remove_all(ui: &AppWindow) {
    let mut entry = global_store!(ui).get_video_editor_smart_clip();
    entry.entries = ModelRc::new(VecModel::from_slice(&[]));
    entry.progress = 0.0;
    entry.progress_type = UISmartClipProgressType::None;
    global_store!(ui).set_video_editor_smart_clip(entry);

    let mut cache = SMART_CLIP_CACHE.lock().unwrap();
    cache.audio_config = None;
}

fn video_editor_smart_clip_apply(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_smart_clip();
    let entries = store_video_editor_smart_clip_entries!(entry);

    let track_idx = global_store!(ui).get_video_editor_smart_clip_segment_track_index() as usize;
    let seg_idx = global_store!(ui).get_video_editor_smart_clip_segment_index() as usize;

    // Collect entry boundaries and keep status
    let entry_data: Vec<(u64, u64, bool)> = entries
        .iter()
        .map(|e| (e.start_ms as u64, e.end_ms as u64, e.is_kept))
        .collect();

    if entry_data.is_empty() {
        toast_warn!(ui, tr("No segments to apply"));
        return;
    }

    // Build the split-and-keep plan:
    // We split at every entry start_ms AND end_ms, creating sub-segments for both
    // voice segments and silence gaps. Then we decide which sub-segments to keep.
    //
    // Example: entries = [(0, 5000, kept), (7000, 12000, not-kept), (15000, 20000, kept)]
    // Timeline: |voice0|silence|voice1|silence|voice2|silance|
    //           |keep  |remove |remove|remove |keep  |remove |
    let segment_duration_ms = global_store!(ui)
        .get_video_editor_smart_clip()
        .media_duration_ms as u64;

    // Collect all unique split points (both start and end of each entry)
    let mut split_points: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    for (start_ms, end_ms, _) in &entry_data {
        if *start_ms > 0 {
            split_points.insert(*start_ms);
        }
        if *end_ms > 0 && *end_ms < segment_duration_ms {
            split_points.insert(*end_ms);
        }
    }

    // Build the list of sub-segments: (start, end, is_kept)
    // A sub-segment is kept if it falls within a kept entry's [start_ms, end_ms]
    let mut sub_segments: Vec<(u64, u64, bool)> = Vec::new();
    let mut boundaries: Vec<u64> = vec![0];
    boundaries.extend(split_points.iter());
    if *boundaries.last().unwrap() < segment_duration_ms {
        boundaries.push(segment_duration_ms);
    }

    for i in 0..boundaries.len() - 1 {
        let sub_start = boundaries[i];
        let sub_end = boundaries[i + 1];
        if sub_start >= sub_end {
            continue;
        }
        // Check if this sub-segment overlaps with any kept entry
        let is_kept = entry_data
            .iter()
            .any(|(es, ee, kept)| *kept && sub_start >= *es && sub_end <= *ee);
        sub_segments.push((sub_start, sub_end, is_kept));
    }

    // Use split_points (only start boundaries, not end) for actual splitting
    let split_only_starts: Vec<u64> = split_points.into_iter().collect();

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(video_editor::Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let mut batch_command = BatchCommand::new("Smart voice clip".to_string());

        // Split the original segment at each boundary (reverse order)
        for &split_ms in split_only_starts.iter().rev() {
            let split_duration = Duration::from_millis(split_ms);
            if split_duration > Duration::ZERO {
                batch_command.add_command(Box::new(SplitSegmentCommand::new(
                    track_idx,
                    seg_idx,
                    split_duration,
                )));
            }
        }

        // After splitting, we have N sub-segments at indices seg_idx..seg_idx+N
        // Mark all sub-segments as affected
        for i in 0..split_only_starts.len() {
            batch_command.add_extra_affected_segment(AffectedSegment::with_both_thumbnails(
                track_idx,
                seg_idx + i,
            ));
        }

        // Remove sub-segments that are NOT kept (in reverse order to preserve indices)
        // After splitting, sub-segment i corresponds to sub_segments[i]
        for (i, (_, _, is_kept)) in sub_segments.iter().enumerate().rev() {
            if !is_kept {
                batch_command.add_command(Box::new(RemoveSegmentCommand::new(
                    track_idx,
                    seg_idx + i,
                    true, // shift_timeline
                )));
            }
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            toast_success!(ui, tr("Smart voice clip applied successfully"));
        }
        Err(e) => toast_warn!(
            ui,
            format!("{}: {e}", tr("Failed to apply smart voice clip"))
        ),
    }
}

fn video_editor_smart_clip_setting_is_valid(ui: &AppWindow) -> bool {
    let setting = global_store!(ui).get_video_editor_transcribe_setting();
    cutil::fs::file_exist(&setting.model_path)
        && cutil::fs::file_exist(&setting.model_tokenizer_path)
}

fn video_editor_smart_clip_choose_model_path(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose model or tokenizer"),
            &tr("fun ast model or tokenizer"),
            &["pt", "json"],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath = filepath.to_string_lossy().to_string().into();
            let mut setting = global_store!(ui).get_video_editor_transcribe_setting();
            match index {
                0 => setting.model_path = filepath,
                1 => setting.model_tokenizer_path = filepath,
                _ => log::warn!("Unexpected smart clip model index = {index}"),
            }

            global_store!(ui).set_video_editor_transcribe_setting(setting.clone());
            save_transcribe_config(crate::db::TranscribeConfigData::from(setting));
        });
    });
}

fn video_editor_smart_clip_model_start_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;
    let filename = fun_ast_nano::Model::all_models()[index]
        .to_filename()
        .into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) =
                store_video_editor_smart_clip_models_dowloader!(ui).row_data(index)
            {
                item.progress = progress;
                store_video_editor_smart_clip_models_dowloader!(ui).set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: std::path::PathBuf| {
            if let Some(mut item) =
                store_video_editor_smart_clip_models_dowloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Downloading;
                store_video_editor_smart_clip_models_dowloader!(ui).set_row_data(index, item);
            }

            let filepath = filepath.to_string_lossy().to_string().into();
            let mut setting = global_store!(ui).get_video_editor_transcribe_setting();
            match index {
                0 => setting.model_path = filepath,
                1 => setting.model_tokenizer_path = filepath,
                _ => log::warn!("Unexpected smart clip model index = {index}"),
            }
            global_store!(ui).set_video_editor_transcribe_setting(setting.clone());
            save_transcribe_config(crate::db::TranscribeConfigData::from(setting));
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) =
                store_video_editor_smart_clip_models_dowloader!(ui).row_data(index)
            {
                match result {
                    Ok(downloader::DownloadState::Cancelled) => {
                        item.state = UIDownloaderState::Cancelled
                    }
                    Ok(downloader::DownloadState::Incompleted) => {
                        item.state = UIDownloaderState::Failed
                    }
                    Ok(downloader::DownloadState::Finsished) => {
                        item.state = UIDownloaderState::Finished
                    }
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_video_editor_smart_clip_models_dowloader!(ui).set_row_data(index, item);
            }
        },
    );
}

fn video_editor_smart_clip_model_cancel_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) = store_video_editor_smart_clip_models_dowloader!(ui).row_data(index)
        {
            item.state = UIDownloaderState::Cancelled;
            store_video_editor_smart_clip_models_dowloader!(ui).set_row_data(index, item);
        }
    });
}

fn video_editor_smart_clip_audio_player_sound_changed(_ui: &AppWindow, sound: f32) {
    audio_player::set_volume(sound.clamp(0.0, 1.0));
}

fn video_editor_smart_clip_audio_player_sound_released(ui: &AppWindow, sound: f32) {
    audio_player::set_volume(sound.clamp(0.0, 1.0));
    global_store!(ui).set_video_editor_smart_clip_audio_sound(sound);
}

fn video_editor_smart_clip_play_from_progress(ui: &AppWindow, progress: f32) {
    let (checked_segments, total_duration_ms) = collect_checked_segments(ui);

    if checked_segments.is_empty() {
        toast_warn!(ui, tr("No segments selected to play"));
        return;
    }

    let (segments_from_seek, offset_ms) =
        segments_from_progress(&checked_segments, total_duration_ms, progress);

    let audio_config = {
        let cache = SMART_CLIP_CACHE.lock().unwrap();
        match cache.audio_config.clone() {
            Some(config) => config,
            None => {
                toast_warn!(ui, tr("No audio data available"));
                return;
            }
        }
    };

    let all_entries = collect_all_entries_info(ui);

    audio_player::play_segments_from_offset(
        ui,
        &audio_config,
        &segments_from_seek,
        total_duration_ms,
        offset_ms,
        all_entries,
        true,
    );
}

fn video_editor_smart_clip_audio_player_progress_pressed(ui: &AppWindow, value: f32) {
    audio_player::stop_audio();
    global_store!(ui).set_video_editor_smart_clip_audio_player_is_playing(false);
    global_store!(ui).set_video_editor_smart_clip_audio_player_progress(value);
}

fn video_editor_smart_clip_audio_player_progress_released(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_smart_clip_audio_player_progress(value);
    video_editor_smart_clip_play_from_progress(ui, value);
}

fn video_editor_smart_clip_audio_player_speed_changed(ui: &AppWindow, speed: f32) {
    audio_player::set_speed(speed);
    global_store!(ui).set_video_editor_smart_clip_audio_speed(speed);
}
