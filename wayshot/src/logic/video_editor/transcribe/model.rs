use super::audio_player::{
    self, MAX_WAVE_FORM_SAMPLE_COUNTS, extract_audio_samples, get_current_audio_config,
};
use crate::{
    config,
    db::{TranscribeConfigData, VIDEO_EDITOR_TABLE},
    global_logic, global_store,
    logic::{
        recorder::picker_directory,
        toast,
        tr::tr,
        video_editor::{
            self,
            project::TRANSCRIBE_CONFIG_ID,
            track,
            vad::{detect_voice_segments, to_mono},
        },
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, SettingTranscribe as UISettingTranscribe, SmartMixEntry as UISmartMixEntry,
        Subtitle as UISubtitle, Transcribe as UITranscribe,
        TranscribeProgressType as UITranscribeProgressType,
    },
    toast_info, toast_success, toast_warn,
};
use anyhow::{Result, anyhow};
use audio_utils::{
    audio::{downsample_audio, max_sound_wave_amplitude},
    loader::AudioConfig,
};
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use fun_ast_nano::{FunASRModelConfig, FunAsrError, FunAsrNanoGenerateModel};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
};
use video_utils::subtitle::{
    Subtitle as ExportSubtitle, chinese_numbers_to_primitive_numbers, ms_to_srt_timestamp,
    save_as_srt, split_subtitle, srt_timestamp_to_ms, valid_srt_timestamp,
};

const DEFAULT_PROMPT: &str = "Transcribe audio to text.";
static TRANSCRIBE_CACHE: Lazy<Mutex<TranscribeCache>> =
    Lazy::new(|| Mutex::new(TranscribeCache::default()));

#[derive(Default, Clone)]
struct TranscribeCache {
    transcribe_stop_sig: Option<Arc<AtomicBool>>,
    ai_correction_stop_sig: Option<Arc<AtomicBool>>,
    inc_index: u64,
}

#[derive(serde::Serialize)]
struct InputData {
    correct_text: String,
    transcribed: Vec<InputSubtitle>,
}

#[derive(serde::Serialize)]
struct InputSubtitle {
    index: usize,
    text: String,
}

#[derive(serde::Deserialize)]
struct OutputSubtitle {
    index: usize,
    correction: String,
}

#[macro_export]
macro_rules! store_video_editor_transcribe_subtitles {
    ($transcribe: expr) => {
        $transcribe
            .subtitles
            .as_any()
            .downcast_ref::<VecModel<UISubtitle>>()
            .expect("We know we set a VecModel<UISubtitle> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(file_exist, ui, file);
    logic_cb!(dir_exist, ui, file);
    logic_cb!(is_valid_subtitle_timestamp, ui, timestamp);
    logic_cb!(ms_to_srt_timestamp_ui, ui, ms);

    logic_cb!(video_editor_transcribe_setting_is_valid, ui);
    logic_cb!(video_editor_transcribe_start, ui);
    logic_cb!(video_editor_transcribe_export_subtitles, ui);
    logic_cb!(video_editor_transcribe_cancel_progress, ui, ty);
    logic_cb!(video_editor_transcribe_update_setting, ui, setting);

    logic_cb!(video_editor_transcribe_subtitles_remove_all, ui);
    logic_cb!(video_editor_transcribe_subtitles_clear, ui);
    logic_cb!(video_editor_transcribe_subtitles_correction, ui);
    logic_cb!(
        video_editor_transcribe_subtitles_correction_with_text,
        ui,
        text
    );
    logic_cb!(video_editor_transcribe_subtitles_accept_correction, ui);
    logic_cb!(video_editor_transcribe_subtitles_remove_correction, ui);
    logic_cb!(
        video_editor_transcribe_subtitles_adjust_overlap_timestamp,
        ui
    );
    logic_cb!(video_editor_transcribe_subtitles_to_lowercase, ui);
    logic_cb!(video_editor_transcribe_subtitles_to_simple_chinese, ui);
    logic_cb!(video_editor_transcribe_subtitles_to_primitive_numbers, ui);
    logic_cb!(video_editor_transcribe_subtitles_remove_separator, ui);
    logic_cb!(
        video_editor_transcribe_subtitles_replace_text,
        ui,
        old_text,
        new_text
    );
    logic_cb!(
        video_editor_transcribe_subtitles_update_playing_index,
        ui,
        progress
    );

    logic_cb!(video_editor_transcribe_subtitle_update, ui, index, text);
    logic_cb!(
        video_editor_transcribe_subtitle_accept_correction,
        ui,
        index
    );
    logic_cb!(video_editor_transcribe_subtitle_split, ui, index);
    logic_cb!(video_editor_transcribe_subtitle_merge_above, ui, index);
    logic_cb!(video_editor_transcribe_subtitle_insert_above, ui, index);
    logic_cb!(video_editor_transcribe_subtitle_insert_below, ui, index);
    logic_cb!(video_editor_transcribe_subtitle_remove, ui, index);
    logic_cb!(video_editor_transcribe_import_from_smart_mix, ui);
}

fn inner_init(ui: &AppWindow) {
    let mut transcribe = UITranscribe::default();
    transcribe.playing_index = -1;
    transcribe.subtitles = ModelRc::new(VecModel::from_slice(&[]));
    global_store!(ui).set_video_editor_transcribe(transcribe);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_transcribe_config()
            .await
            .unwrap_or_else(TranscribeConfigData::default);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_transcribe_setting(config.into());
        });
    });
}

fn file_exist(_ui: &AppWindow, file: SharedString) -> bool {
    cutil::fs::file_exist(file)
}

fn dir_exist(_ui: &AppWindow, dir: SharedString) -> bool {
    Path::new(&dir).exists()
}

fn is_valid_subtitle_timestamp(_ui: &AppWindow, timestamp: SharedString) -> bool {
    valid_srt_timestamp(&timestamp)
}

fn ms_to_srt_timestamp_ui(_ui: &AppWindow, ms: f32) -> SharedString {
    ms_to_srt_timestamp(ms as u64).into()
}

fn video_editor_transcribe_setting_is_valid(ui: &AppWindow) -> bool {
    let setting = global_store!(ui).get_video_editor_transcribe_setting();
    cutil::fs::file_exist(&setting.model_path)
        && cutil::fs::file_exist(&setting.model_tokenizer_path)
}

fn video_editor_transcribe_update_setting(ui: &AppWindow, setting: UISettingTranscribe) {
    global_store!(ui).set_video_editor_transcribe_setting(setting.clone());
    save_transcribe_config(TranscribeConfigData::from(setting));
}

fn reset_video_editor_transcribe_stop_sig() {
    let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
    if let Some(stop_sig) = cache.transcribe_stop_sig.take() {
        stop_sig.store(true, Ordering::Relaxed);
    }
    cache.transcribe_stop_sig = Some(Arc::new(AtomicBool::new(false)));
}

fn video_editor_transcribe_start(ui: &AppWindow) {
    reset_video_editor_transcribe_stop_sig();

    let audio_config = get_current_audio_config();
    if audio_config.is_none() {
        toast_warn!(
            ui,
            tr(
                "No audio data available. Please export audio from timeline or import a file first."
            )
        );
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_transcribe();
    entry.progress = 0.0;
    entry.progress_type = UITranscribeProgressType::Transcribe;
    entry.subtitles = ModelRc::new(VecModel::from_slice(&[]));
    entry.playing_index = -1;
    global_store!(ui).set_video_editor_transcribe(entry);

    global_store!(ui).set_video_editor_transcribe_audio_player_progress(0.0);
    global_store!(ui).set_video_editor_transcribe_audio_player_is_playing(false);

    if let Err(e) = inner_video_editor_transcribe_start(&ui) {
        toast_warn!(ui, format!("{}: {e}", tr("Start transcribe failed")));
    }
}

fn inner_video_editor_transcribe_start(ui: &AppWindow) -> Result<()> {
    let audio_config =
        get_current_audio_config().ok_or_else(|| anyhow!("No audio data available"))?;

    let ui_weak = ui.as_weak();
    let setting = global_store!(ui).get_video_editor_transcribe_setting();
    let (stop_sig, inc_index) = {
        let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
        cache.inc_index += 1;
        (cache.transcribe_stop_sig.clone(), cache.inc_index)
    };

    thread::spawn(move || {
        let ui_weak_clone = ui_weak.clone();

        // Step 1: VAD - detect voice segments
        let mono = to_mono(&audio_config.samples, audio_config.channel);
        let speech_regions = match detect_voice_segments(&mono, audio_config.sample_rate) {
            Ok(regions) => regions,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("VAD detection failed")),
                );
                return;
            }
        };

        if speech_regions.is_empty() {
            toast::async_toast_warn(ui_weak.clone(), tr("No speech detected"));
            return;
        }

        if let Some(ref stop_sig) = stop_sig
            && stop_sig.load(Ordering::Relaxed)
        {
            return;
        }

        // Step 2: Load model
        let config = FunASRModelConfig::default()
            .with_model_weights(setting.model_path.to_string())
            .with_tokenizer_path(setting.model_tokenizer_path.to_string());

        log::info!("Loading transcribe model: {config:?}");

        let mut model = match FunAsrNanoGenerateModel::new(config, None, None) {
            Ok(model) => model,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("New transcribe model failed")),
                );
                return;
            }
        };

        // Step 3: Transcribe each speech segment
        let total_segments = speech_regions.len();
        let audio_samples_len = audio_config.samples.len();

        for (seg_idx, (start_ms, end_ms)) in speech_regions.iter().enumerate() {
            if let Some(ref stop_sig) = stop_sig
                && stop_sig.load(Ordering::Relaxed)
            {
                return;
            }

            if inc_index != TRANSCRIBE_CACHE.lock().unwrap().inc_index {
                return;
            }

            let start_sample = (*start_ms as f64 / 1000.0
                * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
                .round() as usize;
            let end_sample = (*end_ms as f64 / 1000.0
                * fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE as f64)
                .round() as usize;
            let end_sample = end_sample.min(audio_samples_len);

            if start_sample >= end_sample {
                continue;
            }

            let seg_audio = audio_config.samples[start_sample..end_sample].to_vec();
            let seg_duration = std::time::Duration::from_secs_f64(
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
                Ok(response) => fast2s::convert(&response.text),
                Err(FunAsrError::TranscribeCancelled) => return,
                Err(e) => {
                    log::warn!("Transcribe segment {} failed: {e}", seg_idx);
                    String::new()
                }
            };

            let start_ms_val = *start_ms;
            let end_ms_val = *end_ms;
            let text_clone = text.clone();
            let ui_weak_ref = ui_weak.clone();
            let audio_config_clone = audio_config.clone();

            _ = ui_weak_ref.upgrade_in_event_loop(move |ui| {
                let start_timestamp = ms_to_srt_timestamp(start_ms_val as u64).into();
                let end_timestamp = ms_to_srt_timestamp(end_ms_val as u64).into();

                let samples = extract_audio_samples(
                    &audio_config_clone,
                    start_ms_val as u64,
                    end_ms_val as u64,
                );
                let samples = downsample_audio(&samples, MAX_WAVE_FORM_SAMPLE_COUNTS as usize);
                let amplitude = max_sound_wave_amplitude(&samples);

                let subtitle = UISubtitle {
                    start_timestamp,
                    end_timestamp,
                    original_text: text_clone.into(),
                    correction_text: Default::default(),
                    audio_wave_amplitude: amplitude,
                    audio_samples: ModelRc::new(VecModel::from_slice(&samples)),
                    is_timestamp_overlap: false,
                };

                let mut entry = global_store!(ui).get_video_editor_transcribe();
                entry.progress_type = UITranscribeProgressType::Transcribe;
                entry.progress = (seg_idx + 1) as f32 / total_segments as f32;
                store_video_editor_transcribe_subtitles!(entry).push(subtitle);
                global_store!(ui).set_video_editor_transcribe(entry);
            });
        }

        if inc_index != TRANSCRIBE_CACHE.lock().unwrap().inc_index {
            return;
        }

        // Finished
        let value = audio_config.clone();
        _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
            let mut entry = global_store!(ui).get_video_editor_transcribe();
            entry.progress_type = UITranscribeProgressType::Finished;
            entry.progress = 1.0;
            entry.media_duration_ms = value.duration.as_millis() as f32;
            global_store!(ui).set_video_editor_transcribe(entry);
        });

        if inc_index != TRANSCRIBE_CACHE.lock().unwrap().inc_index {
            return;
        }

        // Handle cancellation that might have been set during the loop
        if let Some(ref stop_sig) = stop_sig
            && stop_sig.load(Ordering::Relaxed)
        {
            _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                let mut entry = global_store!(ui).get_video_editor_transcribe();
                entry.progress_type = UITranscribeProgressType::Cancelled;
                global_store!(ui).set_video_editor_transcribe(entry);
            });
        }
    });

    Ok(())
}

fn video_editor_transcribe_export_subtitles(ui: &AppWindow) {
    let filename = "subtitles.srt";

    let Some(items) = get_export_subtitles(ui) else {
        toast_warn!(ui, tr("Contain invalid srt timestamp"));
        return;
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(path) = picker_directory(ui_weak.clone(), &tr("Export Subtitle")) else {
            return;
        };

        let path = path.join(filename);
        match save_as_srt(&items, path.clone()) {
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("Export subtitle failed")))
            }
            Ok(_) => {
                toast::async_toast_success(ui_weak.clone(), tr("Export subtitle successfully"));
                tokio::spawn(async move {
                    video_editor::playlist::import_file_to_playlist(ui_weak, path, None).await;
                });
            }
        }
    });
}

fn get_export_subtitles(ui: &AppWindow) -> Option<Vec<ExportSubtitle>> {
    let mut items = vec![];
    let entry = global_store!(ui).get_video_editor_transcribe();

    for (index, item) in store_video_editor_transcribe_subtitles!(entry)
        .iter()
        .enumerate()
    {
        let item = ExportSubtitle {
            index: index as u32,
            start_timestamp: srt_timestamp_to_ms(&item.start_timestamp).ok()?,
            end_timestamp: srt_timestamp_to_ms(&item.end_timestamp).ok()?,
            text: item.original_text.into(),
        };

        items.push(item);
    }

    Some(items)
}

fn video_editor_transcribe_cancel_progress(ui: &AppWindow, ty: UITranscribeProgressType) {
    match ty {
        UITranscribeProgressType::Transcribe => {
            if let Some(ref stop_sig) = TRANSCRIBE_CACHE.lock().unwrap().transcribe_stop_sig {
                stop_sig.store(true, Ordering::Relaxed);
            }
        }
        UITranscribeProgressType::CorrectSubtitles => {
            if let Some(ref stop_sig) = TRANSCRIBE_CACHE.lock().unwrap().ai_correction_stop_sig {
                stop_sig.store(true, Ordering::Relaxed);
            }
        }
        UITranscribeProgressType::ImportingAudio => track::cancel_audio_export(),
        _ => {}
    }

    let mut entry = global_store!(ui).get_video_editor_transcribe();
    entry.progress_type = UITranscribeProgressType::Cancelled;
    global_store!(ui).set_video_editor_transcribe(entry);
}

fn video_editor_transcribe_subtitles_remove_all(ui: &AppWindow) {
    global_store!(ui).set_video_editor_transcribe(UITranscribe::default());
    audio_player::set_current_audio_config(None);
}

fn video_editor_transcribe_subtitles_clear(ui: &AppWindow) {
    let mut entry = global_store!(ui).get_video_editor_transcribe();
    store_video_editor_transcribe_subtitles!(entry).set_vec(vec![]);
    entry.playing_index = -1;
    entry.progress = 0.0;
    entry.progress_type = UITranscribeProgressType::None;
    global_store!(ui).set_video_editor_transcribe(entry);
}

fn video_editor_transcribe_subtitles_correction(ui: &AppWindow) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, tr("Please setup AI model and try again."));
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let subtitles_to_correct = subtitles
        .iter()
        .enumerate()
        .filter_map(|(index, sub)| {
            if sub.correction_text.is_empty() && !sub.original_text.is_empty() {
                Some((index, sub.original_text.to_string()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if subtitles_to_correct.is_empty() {
        toast_info!(
            ui,
            tr("All subtitles already have corrections or are empty")
        );
        return;
    }

    entry.progress = 0.0;
    entry.progress_type = UITranscribeProgressType::CorrectSubtitles;
    global_store!(ui).set_video_editor_transcribe(entry);

    let stop_sig = Arc::new(AtomicBool::new(false));
    {
        let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
        if let Some(sig) = cache.transcribe_stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.ai_correction_stop_sig = Some(stop_sig.clone());
    }

    let total_subtitles_count = subtitles_to_correct.len();
    let finished_subtitles_count = Arc::new(AtomicU32::new(0));

    for (chunk_index, chunk) in subtitles_to_correct.chunks(10).enumerate() {
        let ui_weak = ui.as_weak();
        let chunk = chunk.to_vec();
        let stop_sig_clone = stop_sig.clone();
        let finished_subtitles_count_clone = finished_subtitles_count.clone();

        tokio::spawn(async move {
            match ai_correct_subtitles(ui_weak.clone(), chunk, stop_sig_clone.clone()).await {
                Ok(corrections) => {
                    if stop_sig_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let mut entry = global_store!(ui).get_video_editor_transcribe();
                        let subtitles = store_video_editor_transcribe_subtitles!(entry);

                        let updated_subtitles = subtitles
                            .iter()
                            .enumerate()
                            .map(|(index, mut subtitle)| {
                                if let Some(correction) = corrections.get(&index) {
                                    subtitle.correction_text = correction.clone().into();
                                }
                                subtitle
                            })
                            .collect::<Vec<_>>();

                        let counts = finished_subtitles_count_clone
                            .fetch_add(corrections.len() as u32, Ordering::Relaxed);

                        entry.progress = (counts + corrections.len() as u32) as f32
                            / total_subtitles_count as f32;

                        if entry.progress == 1.0 {
                            entry.progress_type = UITranscribeProgressType::Finished;
                        }

                        store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
                        global_store!(ui).set_video_editor_transcribe(entry);
                    });
                }
                Err(e) => toast::async_toast_warn(
                    ui_weak,
                    format!(
                        "{}[{}] {}: {e}",
                        tr("Chunk"),
                        chunk_index,
                        tr("AI correction failed")
                    ),
                ),
            }
        });
    }
}

fn video_editor_transcribe_subtitles_correction_with_text(ui: &AppWindow, text: SharedString) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, tr("Please setup AI model and try again"));
        return;
    }

    let mut entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let subtitles_to_correct: Vec<(usize, String)> = subtitles
        .iter()
        .enumerate()
        .filter_map(|(index, sub)| {
            if sub.correction_text.is_empty() && !sub.original_text.is_empty() {
                Some((index, sub.original_text.to_string()))
            } else {
                None
            }
        })
        .collect();

    if subtitles_to_correct.is_empty() {
        toast_info!(ui, tr("No transcribed subtitles to correct"));
        return;
    }

    entry.progress = 0.0;
    entry.progress_type = UITranscribeProgressType::CorrectSubtitles;
    global_store!(ui).set_video_editor_transcribe(entry);

    let stop_sig = Arc::new(AtomicBool::new(false));
    {
        let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
        if let Some(sig) = cache.ai_correction_stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.ai_correction_stop_sig = Some(stop_sig.clone());
    }

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        match ai_correct_subtitles_with_reference(
            ui_weak.clone(),
            text.to_string(),
            subtitles_to_correct,
            stop_sig.clone(),
        )
        .await
        {
            Ok(corrections) => {
                if stop_sig.load(Ordering::Relaxed) {
                    return;
                }

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_video_editor_transcribe();
                    let subtitles = store_video_editor_transcribe_subtitles!(entry);

                    let updated_subtitles = subtitles
                        .iter()
                        .enumerate()
                        .map(|(index, mut subtitle)| {
                            if let Some(correction) = corrections.get(&index) {
                                subtitle.correction_text = correction.clone().into();
                            }
                            subtitle
                        })
                        .collect::<Vec<_>>();

                    entry.progress = 1.0;
                    entry.progress_type = UITranscribeProgressType::Finished;

                    store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
                    global_store!(ui).set_video_editor_transcribe(entry);
                });
            }
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("AI correction with reference failed")),
                );
            }
        }
    });
}

async fn ai_correct_subtitles(
    ui_weak: Weak<AppWindow>,
    subtitles: Vec<(usize, String)>,
    stop_sig: Arc<AtomicBool>,
) -> Result<HashMap<usize, String>> {
    let prompt = r#"You are a subtitle correction assistant. Please correct the misspelled words in the following statement. Only output the JSON array, no additional text.

<Input format>
[{"index": 1, "text": "text1"}, {"index": 3, "text": "text3"}, ...]
</Input format>

<Output format>
[{"index": 1, "correction": "correction1"}, {"index": 3, "correction": "correction3"}, ...]
</Output format>
"#;

    let input: Vec<InputSubtitle> = subtitles
        .into_iter()
        .map(|(index, text)| InputSubtitle { index, text })
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
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Start AI correction failed")),
            );
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if stop_sig.load(Ordering::Relaxed) {
            return Ok(HashMap::new());
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

    log::debug!("{resp}");

    let output_subtitles: Vec<OutputSubtitle> = serde_json::from_str(&resp)
        .map_err(|e| anyhow!("Failed to parse AI response as JSON: {e}. Response: {resp}"))?;

    let corrections = output_subtitles
        .into_iter()
        .map(|item| (item.index, item.correction))
        .collect();

    Ok(corrections)
}

async fn ai_correct_subtitles_with_reference(
    ui_weak: Weak<AppWindow>,
    correct_text: String,
    subtitles: Vec<(usize, String)>,
    stop_sig: Arc<AtomicBool>,
) -> Result<HashMap<usize, String>> {
    let prompt = r#"You are a subtitle correction assistant. The user has provided the correct/original text as reference. Please match this correct text to the transcribed subtitles (which may have speech recognition errors). Only output the JSON array, no additional text.

<Input format>
{
  "correct_text": "The complete correct text provided by user...",
  "transcribed": [{"index": 0, "text": "transcribed text 1"}, {"index": 1, "text": "transcribed text 2"}, ...]
}
</Input format>

<Output format>
[{"index": 0, "correction": "corrected text for subtitle 0"}, {"index": 1, "correction": "corrected text for subtitle 1"}, ...]
</Output format>

Match the correct text segments to each transcribed subtitle based on content similarity and order. Each correction should be the portion of correct_text without overlap that corresponds to that subtitle segment.
"#;

    let input = InputData {
        correct_text,
        transcribed: subtitles
            .into_iter()
            .map(|(index, text)| InputSubtitle { index, text })
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
                format!("{}: {e}", tr("Start AI correction with reference failed")),
            );
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if stop_sig.load(Ordering::Relaxed) {
            return Ok(HashMap::new());
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

    log::debug!("{resp}");

    let output_subtitles: Vec<OutputSubtitle> = serde_json::from_str(&resp)
        .map_err(|e| anyhow!("Failed to parse AI response as JSON: {e}. Response: {resp}"))?;

    let corrections = output_subtitles
        .into_iter()
        .map(|item| (item.index, item.correction))
        .collect();

    Ok(corrections)
}

fn video_editor_transcribe_subtitles_accept_correction(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            if !subtitle.correction_text.is_empty() {
                subtitle.original_text = subtitle.correction_text.clone();
                subtitle.correction_text = SharedString::default();
            }

            subtitle
        })
        .collect::<Vec<_>>();

    store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
}

fn video_editor_transcribe_subtitles_remove_correction(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.correction_text = SharedString::default();
            subtitle
        })
        .collect::<Vec<_>>();

    store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
}

fn video_editor_transcribe_subtitles_adjust_overlap_timestamp(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let total = subtitles.row_count();

    for index in 0..total {
        if index == total - 1 {
            break;
        }

        let mut current = subtitles.row_data(index).unwrap();
        let mut next = subtitles.row_data(index + 1).unwrap();

        let current_end_ms = srt_timestamp_to_ms(&current.end_timestamp);
        let next_start_ms = srt_timestamp_to_ms(&next.start_timestamp);

        if let (Ok(end_ms), Ok(next_ms)) = (current_end_ms, next_start_ms) {
            if end_ms > next_ms {
                current.is_timestamp_overlap = false;
                store_video_editor_transcribe_subtitles!(entry).set_row_data(index, current);

                next.start_timestamp = ms_to_srt_timestamp(end_ms).into();
                next.is_timestamp_overlap = false;
                store_video_editor_transcribe_subtitles!(entry).set_row_data(index + 1, next);
            }
        }
    }

    toast_success!(ui, &tr("Adjust overlap timestamp successfully"));
}

fn video_editor_transcribe_subtitles_to_lowercase(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.original_text = subtitle.original_text.to_lowercase().into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    toast_success!(ui, tr("Convert to lowercase successfully"));
}

fn video_editor_transcribe_subtitles_to_simple_chinese(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.original_text = fast2s::convert(&subtitle.original_text).into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    toast_success!(ui, tr("Convert to simplified Chinese successfully"));
}

fn video_editor_transcribe_subtitles_to_primitive_numbers(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    store_video_editor_transcribe_subtitles!(entry)
        .iter()
        .enumerate()
        .for_each(|(index, mut subtitle)| {
            let converted_text = chinese_numbers_to_primitive_numbers(&subtitle.original_text);
            subtitle.original_text = converted_text.into();
            store_video_editor_transcribe_subtitles!(entry).set_row_data(index, subtitle);
            toast_success!(ui, tr("Convert to primitive numbers successfully"));
        });
}

fn video_editor_transcribe_subtitles_remove_separator(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.original_text =
                remove_separators_except_between_digits(subtitle.original_text.as_str())
                    .trim()
                    .to_string()
                    .into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_video_editor_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    toast_success!(ui, tr("Remove separators successfully"));
}

fn remove_separators_except_between_digits(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(chars.len());

    for (index, &ch) in chars.iter().enumerate() {
        if ch == ',' || ch == '，' {
            let prev_is_digit = index > 0 && is_digit_char(chars[index - 1]);
            let next_is_digit = index + 1 < chars.len() && is_digit_char(chars[index + 1]);
            if prev_is_digit && next_is_digit {
                result.push(ch);
            } else {
                result.push(' ');
            }
        } else if ch == '。' || ch == ';' || ch == '；' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }

    result
}

fn is_digit_char(ch: char) -> bool {
    ch.is_ascii_digit() || ('０'..='９').contains(&ch)
}

fn video_editor_transcribe_subtitles_replace_text(
    ui: &AppWindow,
    old_text: SharedString,
    new_text: SharedString,
) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry)
        .iter()
        .map(|mut entry| {
            entry.original_text = entry
                .original_text
                .replace(old_text.as_str(), new_text.as_str())
                .into();
            entry
        })
        .collect::<Vec<_>>();

    store_video_editor_transcribe_subtitles!(entry).set_vec(subtitles);
    toast_success!(ui, tr("Replace content of subtitles successfully"));
}

fn video_editor_transcribe_subtitles_update_playing_index(ui: &AppWindow, progress: f32) {
    let mut entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let row_counts = subtitles.row_count();
    let target_ms = (entry.media_duration_ms * progress) as u64;

    if row_counts == 0 {
        return;
    }

    let subtitle_times: Vec<(usize, u64, u64)> = subtitles
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let start_ms = srt_timestamp_to_ms(&item.start_timestamp).ok()?;
            let end_ms = srt_timestamp_to_ms(&item.end_timestamp).ok()?;
            Some((index, start_ms, end_ms))
        })
        .collect();

    let index = subtitle_times.partition_point(|(_, start, _)| *start <= target_ms);

    if index > 0 {
        let (idx, start_ms, end_ms) = subtitle_times[index - 1];
        if target_ms >= start_ms && target_ms <= end_ms {
            // Target is within this subtitle
            entry.playing_index = idx as i32 * 2;
        } else {
            // Target is in a gap or after the last subtitle
            entry.playing_index = idx as i32 * 2 + 1;
        }
    } else {
        // Target is before the first subtitle
        entry.playing_index = -1;
    }

    global_store!(ui).set_video_editor_transcribe(entry);
}

fn video_editor_transcribe_subtitle_update(ui: &AppWindow, index: i32, text: SharedString) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let mut subtitle = store_video_editor_transcribe_subtitles!(entry)
        .row_data(index)
        .unwrap();

    subtitle.original_text = text;
    store_video_editor_transcribe_subtitles!(entry).set_row_data(index, subtitle);
    toast_success!(ui, tr("Update subtitle successfully"));
}

fn video_editor_transcribe_subtitle_accept_correction(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    if let Some(mut subtitle) = subtitles.row_data(index)
        && !subtitle.correction_text.is_empty()
    {
        subtitle.original_text = subtitle.correction_text.clone();
        store_video_editor_transcribe_subtitles!(entry).set_row_data(index, subtitle);
    }
}

fn video_editor_transcribe_subtitle_split(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitle = store_video_editor_transcribe_subtitles!(entry)
        .row_data(index)
        .unwrap();

    let start_timestamp_ms = srt_timestamp_to_ms(&subtitle.start_timestamp);
    let end_timestamp_ms = srt_timestamp_to_ms(&subtitle.end_timestamp);
    if start_timestamp_ms.is_err() || end_timestamp_ms.is_err() {
        toast_warn!(
            ui,
            format!(
                "invalid timestamp: {} -> {}",
                subtitle.start_timestamp, subtitle.end_timestamp
            )
        );
        return;
    }

    let Some((first_part, second_part)) = split_subtitle(
        start_timestamp_ms.unwrap(),
        end_timestamp_ms.unwrap(),
        &subtitle.original_text,
    ) else {
        toast_warn!(ui, tr("split subtitle failed"));
        return;
    };

    let current_subtitle = UISubtitle {
        start_timestamp: ms_to_srt_timestamp(first_part.0).into(),
        end_timestamp: ms_to_srt_timestamp(first_part.1).into(),
        original_text: first_part.2.into(),
        ..Default::default()
    };

    let next_subtitle = UISubtitle {
        start_timestamp: ms_to_srt_timestamp(second_part.0).into(),
        end_timestamp: ms_to_srt_timestamp(second_part.1).into(),
        original_text: second_part.2.into(),
        ..Default::default()
    };

    store_video_editor_transcribe_subtitles!(entry).set_row_data(index, current_subtitle);
    store_video_editor_transcribe_subtitles!(entry).insert(index + 1, next_subtitle);
    global_logic!(ui).invoke_video_editor_transcribe_sound_wave_update(
        index as i32,
        MAX_WAVE_FORM_SAMPLE_COUNTS,
    );
    global_logic!(ui).invoke_video_editor_transcribe_sound_wave_update(
        index as i32 + 1,
        MAX_WAVE_FORM_SAMPLE_COUNTS,
    );

    toast_success!(ui, tr("Split subtitle successfully"));
}

fn video_editor_transcribe_subtitle_merge_above(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(&entry);

    if index == 0 {
        return;
    }

    let mut prev = subtitles.row_data(index - 1).unwrap();
    let current = subtitles.row_data(index).unwrap();

    prev.end_timestamp = current.end_timestamp;
    prev.original_text = format!("{} {}", prev.original_text, current.original_text).into();
    prev.correction_text = if current.correction_text.is_empty() {
        format!("{}{}", prev.correction_text, current.correction_text).into()
    } else {
        format!("{} {}", prev.correction_text, current.correction_text).into()
    };
    prev.audio_wave_amplitude = prev.audio_wave_amplitude.max(current.audio_wave_amplitude);

    let mut samples = prev.audio_samples.iter().collect::<Vec<_>>();
    samples.extend_from_slice(&current.audio_samples.iter().collect::<Vec<_>>());
    prev.audio_samples = ModelRc::new(VecModel::from_slice(&samples));

    store_video_editor_transcribe_subtitles!(entry).set_row_data(index - 1, prev);
    store_video_editor_transcribe_subtitles!(entry).remove(index);
    global_logic!(ui).invoke_video_editor_transcribe_sound_wave_update(
        index as i32 - 1,
        MAX_WAVE_FORM_SAMPLE_COUNTS,
    );

    toast_success!(ui, tr("Merge subtitle successfully"));
}

fn video_editor_transcribe_subtitle_insert_above(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let new_subtitle = if index == 0 {
        let first = subtitles.row_data(0).unwrap();

        let end_timestamp = if first.start_timestamp == "00:00:00,000" {
            match srt_timestamp_to_ms(&first.start_timestamp) {
                Ok(ms) => ms_to_srt_timestamp(ms + 1000).into(),
                _ => first.start_timestamp.clone(),
            }
        } else {
            first.start_timestamp.clone()
        };

        UISubtitle {
            start_timestamp: "00:00:00,000".into(),
            end_timestamp,
            original_text: "Click to edit".to_string().into(),
            correction_text: Default::default(),
            audio_samples: ModelRc::new(VecModel::from_slice(&[])),
            audio_wave_amplitude: 1.0,
            is_timestamp_overlap: false,
        }
    } else {
        let prev = subtitles.row_data(index - 1).unwrap();
        let current = subtitles.row_data(index).unwrap();
        UISubtitle {
            start_timestamp: prev.end_timestamp.clone(),
            end_timestamp: current.start_timestamp.clone(),
            original_text: "Click to edit".to_string().into(),
            correction_text: Default::default(),
            audio_samples: ModelRc::new(VecModel::from_slice(&[])),
            audio_wave_amplitude: 1.0,
            is_timestamp_overlap: false,
        }
    };

    store_video_editor_transcribe_subtitles!(entry).insert(index, new_subtitle);
    global_logic!(ui).invoke_video_editor_transcribe_sound_wave_update(
        index as i32,
        MAX_WAVE_FORM_SAMPLE_COUNTS,
    );

    toast_success!(ui, tr("Insert subtitle successfully"));

    if index == 0 {
        mark_overlapped_timestamp(ui, index);
    }
}

fn video_editor_transcribe_subtitle_insert_below(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);

    let new_subtitle = if index == subtitles.row_count() - 1 {
        let last = subtitles.row_data(subtitles.row_count() - 1).unwrap();
        let end_timestamp = match srt_timestamp_to_ms(&last.end_timestamp) {
            Ok(ms) => ms_to_srt_timestamp(ms + 1000).into(),
            _ => last.end_timestamp.clone(),
        };

        UISubtitle {
            start_timestamp: last.end_timestamp.clone(),
            end_timestamp,
            original_text: "Click to edit".into(),
            correction_text: Default::default(),
            audio_samples: ModelRc::new(VecModel::from_slice(&[])),
            audio_wave_amplitude: 1.0,
            is_timestamp_overlap: false,
        }
    } else {
        let current = subtitles.row_data(index).unwrap();
        let next = subtitles.row_data(index + 1).unwrap();
        UISubtitle {
            start_timestamp: current.end_timestamp.clone(),
            end_timestamp: next.start_timestamp.clone(),
            original_text: "Click to edit".into(),
            correction_text: Default::default(),
            audio_samples: ModelRc::new(VecModel::from_slice(&[])),
            audio_wave_amplitude: 1.0,
            is_timestamp_overlap: false,
        }
    };

    store_video_editor_transcribe_subtitles!(entry).insert(index + 1, new_subtitle);
    global_logic!(ui).invoke_video_editor_transcribe_sound_wave_update(
        index as i32 + 1,
        MAX_WAVE_FORM_SAMPLE_COUNTS,
    );

    toast_success!(ui, tr("Insert subtitle successfully"));
}

fn video_editor_transcribe_subtitle_remove(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();

    store_video_editor_transcribe_subtitles!(entry).remove(index);
    toast_success!(ui, tr("Remove subtitle successfully"));
}

pub fn mark_overlapped_timestamp(ui: &AppWindow, index: usize) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let total = subtitles.row_count();

    if index >= total {
        return;
    }

    let current = subtitles.row_data(index).unwrap();
    let Ok(current_start_ms) = srt_timestamp_to_ms(&current.start_timestamp) else {
        return;
    };
    let Ok(current_end_ms) = srt_timestamp_to_ms(&current.end_timestamp) else {
        return;
    };

    let mut has_overlap = false;

    if index > 0
        && let Some(prev) = subtitles.row_data(index - 1)
        && let Ok(prev_end_ms) = srt_timestamp_to_ms(&prev.end_timestamp)
    {
        let mut subtitle = store_video_editor_transcribe_subtitles!(entry)
            .row_data(index - 1)
            .unwrap();

        if current_start_ms < prev_end_ms {
            has_overlap = true;
            subtitle.is_timestamp_overlap = true;
        } else {
            subtitle.is_timestamp_overlap = false;
        }

        store_video_editor_transcribe_subtitles!(entry).set_row_data(index - 1, subtitle);
    }

    if index < total - 1
        && let Some(next) = subtitles.row_data(index + 1)
        && let Ok(next_start_ms) = srt_timestamp_to_ms(&next.start_timestamp)
    {
        let mut subtitle = store_video_editor_transcribe_subtitles!(entry)
            .row_data(index + 1)
            .unwrap();

        if current_end_ms > next_start_ms {
            has_overlap = true;
            subtitle.is_timestamp_overlap = true;
        } else {
            subtitle.is_timestamp_overlap = false;
        }

        store_video_editor_transcribe_subtitles!(entry).set_row_data(index + 1, subtitle);
    }

    let mut subtitle = store_video_editor_transcribe_subtitles!(entry)
        .row_data(index)
        .unwrap();
    subtitle.is_timestamp_overlap = has_overlap;
    store_video_editor_transcribe_subtitles!(entry).set_row_data(index, subtitle);
}

pub fn save_transcribe_config(config: TranscribeConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize transcribe config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, TRANSCRIBE_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, TRANSCRIBE_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save transcribe config: {:?}", e);
            }
        }
    });
}

pub async fn load_transcribe_config() -> Option<TranscribeConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, TRANSCRIBE_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_transcribe_import_from_smart_mix(ui: &AppWindow) {
    let entry = global_store!(ui).get_video_editor_smart_mix();
    let audio_entries = crate::store_video_editor_smart_mix_entries!(entry);

    if audio_entries.row_count() == 0 {
        toast_warn!(ui, tr("No audio entries in smart mix to import"));
        return;
    }

    let audio_config = get_current_audio_config();

    // Convert SmartMixEntry → Subtitle
    let subtitles: Vec<UISubtitle> = audio_entries
        .iter()
        .map(|e| {
            let (amplitude, samples) = match &audio_config {
                Some(config) => {
                    let raw = extract_audio_samples(config, e.start_ms as u64, e.end_ms as u64);
                    let downsampled = downsample_audio(&raw, MAX_WAVE_FORM_SAMPLE_COUNTS as usize);
                    let amp = max_sound_wave_amplitude(&downsampled);
                    (amp, downsampled)
                }
                None => (0.0, vec![]),
            };

            UISubtitle {
                start_timestamp: ms_to_srt_timestamp(e.start_ms as u64).into(),
                end_timestamp: ms_to_srt_timestamp(e.end_ms as u64).into(),
                original_text: e.text.clone(),
                correction_text: Default::default(),
                audio_wave_amplitude: amplitude,
                audio_samples: ModelRc::new(VecModel::from_slice(&samples)),
                is_timestamp_overlap: false,
            }
        })
        .collect();

    let mut transcribe = global_store!(ui).get_video_editor_transcribe();
    store_video_editor_transcribe_subtitles!(transcribe).set_vec(subtitles);
    transcribe.progress = 1.0;
    transcribe.progress_type = UITranscribeProgressType::Finished;
    global_store!(ui).set_video_editor_transcribe(transcribe);

    toast_success!(ui, tr("Imported transcription from smart mix"));
}
