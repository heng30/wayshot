use crate::{
    config,
    db::{TRANSCRIBE_TABLE as DB_TABLE, Transcribe},
    global_logic, global_store,
    logic::{
        recorder::picker_directory,
        share_screen::picker_file,
        toast,
        tr::tr,
        transcribe::audio_player::{
            self, MAX_WAVE_FORM_SAMPLE_COUNTS, extract_audio_samples, get_current_audio_config,
        },
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, ConfirmDialogSetting as UIConfirmDialogSetting, FileType as UIFileType,
        Subtitle as UISubtitle, Transcribe as UITranscribe,
        TranscribeProgressType as UITranscribeProgressType,
    },
    toast_info, toast_success, toast_warn,
};
use anyhow::{Result, anyhow};
use audio_utils::{
    audio::{downsample_audio, max_sound_wave_amplitude},
    loader::{AudioConfig, AudioSegment, gen_audio_segments},
    vad::VadConfig,
};
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use fun_ast_nano::{FunASRModelConfig, FunAsrError, FunAsrNanoGenerateModel, load_audio_file};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::Duration,
};
use video_utils::subtitle::{
    Subtitle as ExportSubtitle, chinese_numbers_to_primitive_numbers, ms_to_srt_timestamp,
    save_as_srt, split_subtitle, srt_timestamp_to_ms, valid_srt_timestamp,
};

const TRANSCRIBE_ID: &str = "transcribe_id";
const DEFAULT_PROMPT: &str = "Transcribe audio to text.";
static TRANSCRIBE_CACHE: Lazy<Mutex<TranscribeCache>> =
    Lazy::new(|| Mutex::new(TranscribeCache::default()));

crate::db_remove_all!(DB_TABLE);
crate::db_add!(DB_TABLE, Transcribe);
crate::db_update!(DB_TABLE, Transcribe);
crate::db_select!(DB_TABLE, Transcribe);

#[derive(Default, Clone)]
struct TranscribeCache {
    transcribe_stop_sig: Option<Arc<AtomicBool>>,
    ai_correction_stop_sig: Option<Arc<AtomicBool>>,
    inc_index: u64,
}

#[macro_export]
macro_rules! store_transcribe_subtitles {
    ($transcribe: expr) => {
        $transcribe
            .subtitles
            .as_any()
            .downcast_ref::<VecModel<UISubtitle>>()
            .expect("We know we set a VecModel<UISubtitle> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    logic_cb!(file_exist, ui, file);
    logic_cb!(is_valid_subtitle_timestamp, ui, timestamp);
    logic_cb!(ms_to_srt_timestamp_ui, ui, ms);

    logic_cb!(transcribe_init, ui);
    logic_cb!(transcribe_new, ui);
    logic_cb!(transcribe_start, ui);
    logic_cb!(transcribe_import_file, ui);
    logic_cb!(transcribe_export_subtitles, ui);
    logic_cb!(transcribe_refresh_subtitles, ui);
    logic_cb!(transcribe_cancel_progress, ui, ty);

    logic_cb!(transcribe_subtitles_recovery, ui);
    logic_cb!(transcribe_subtitles_remove_all, ui);
    logic_cb!(transcribe_subtitles_correction, ui);
    logic_cb!(transcribe_subtitles_accept_correction, ui);
    logic_cb!(transcribe_subtitles_remove_correction, ui);
    logic_cb!(transcribe_subtitles_adjust_overlap_timestamp, ui);
    logic_cb!(transcribe_subtitles_to_lowercase, ui);
    logic_cb!(transcribe_subtitles_to_simple_chinese, ui);
    logic_cb!(transcribe_subtitles_to_primitive_numbers, ui);
    logic_cb!(transcribe_subtitles_remove_separator, ui);
    logic_cb!(transcribe_subtitles_replace_text, ui, old_text, new_text);
    logic_cb!(transcribe_subtitles_update_playng_index, ui, progress);

    logic_cb!(transcribe_subtitle_update, ui, index, text);
    logic_cb!(transcribe_subtitle_accept_correction, ui, index);
    logic_cb!(transcribe_subtitle_split, ui, index);
    logic_cb!(transcribe_subtitle_merge_above, ui, index);
    logic_cb!(transcribe_subtitle_insert_above, ui, index);
    logic_cb!(transcribe_subtitle_insert_below, ui, index);
    logic_cb!(transcribe_subtitle_remove, ui, index);
}

pub fn transcribe_init(ui: &AppWindow) {
    let config = config::all().transcribe;
    global_store!(ui).set_transcribe_setting(config.into());

    let mut transcribe = UITranscribe::default();
    transcribe.playing_index = -1;
    transcribe.subtitles = ModelRc::new(VecModel::from_slice(&[]));
    global_store!(ui).set_transcribe(transcribe);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        if sqldb::entry::is_exist(DB_TABLE, TRANSCRIBE_ID)
            .await
            .is_ok()
        {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_transcribe_can_recovered(true);
            });
        }
    });
}

fn file_exist(_ui: &AppWindow, file: SharedString) -> bool {
    cutil::fs::file_exist(file)
}

fn is_valid_subtitle_timestamp(_ui: &AppWindow, timestamp: SharedString) -> bool {
    valid_srt_timestamp(&timestamp)
}

fn ms_to_srt_timestamp_ui(_ui: &AppWindow, ms: f32) -> SharedString {
    ms_to_srt_timestamp(ms as u64).into()
}

fn reset_transcribe_stop_sig() {
    let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
    if let Some(stop_sig) = cache.transcribe_stop_sig.take() {
        stop_sig.store(true, Ordering::Relaxed);
    }
    cache.transcribe_stop_sig = Some(Arc::new(AtomicBool::new(false)));
}

fn transcribe_new(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose media file"),
            &tr("e.g. mp4, mp3, wav"),
            &[],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let entry = UITranscribe {
                id: TRANSCRIBE_ID.to_string().into(),
                file_path: filepath.to_string_lossy().to_string().into(),
                is_file_exist: true,
                file_type: get_file_type(&filepath),
                subtitles: ModelRc::new(VecModel::from_slice(&[])),
                playing_index: -1,
                ..Default::default()
            };
            global_store!(ui).set_transcribe(entry);
            global_store!(ui).set_transcribe_audio_player_progress(0.0);
            global_store!(ui).set_transcribe_audio_player_is_playing(false);
        });
    });
}

#[inline]
fn get_file_type(file: impl AsRef<Path>) -> UIFileType {
    match file.as_ref().extension() {
        Some(e) => {
            let e = e.to_string_lossy().to_lowercase();
            if e == "mkv" || e == "mp4" {
                UIFileType::Video
            } else {
                UIFileType::Audio
            }
        }
        _ => UIFileType::None,
    }
}

fn transcribe_start(ui: &AppWindow) {
    db_remove_all(ui.as_weak());
    reset_transcribe_stop_sig();

    let mut entry = global_store!(ui).get_transcribe();
    let filepath = PathBuf::from(&entry.file_path);

    if !filepath.exists() {
        toast_warn!(ui, format!("No found {}", filepath.display()));
        return;
    }

    entry.progress = 0.0;
    entry.progress_type = UITranscribeProgressType::Transcribe;
    entry.subtitles = ModelRc::new(VecModel::from_slice(&[]));
    entry.playing_index = -1;
    entry.is_file_exist = true;
    global_store!(ui).set_transcribe(entry);

    global_store!(ui).set_transcribe_can_recovered(false);
    global_store!(ui).set_transcribe_audio_player_progress(0.0);
    global_store!(ui).set_transcribe_audio_player_is_playing(false);

    if let Err(e) = inner_transcribe_start(&ui, filepath) {
        toast_warn!(ui, format!("Start transcribe failed: {e}"));
    }
}

fn inner_transcribe_start(ui: &AppWindow, filepath: PathBuf) -> Result<()> {
    let ui_weak = ui.as_weak();
    let setting = global_store!(ui).get_transcribe_setting();
    let (stop_sig, inc_index) = {
        let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
        cache.inc_index += 1;
        (cache.transcribe_stop_sig.clone(), cache.inc_index)
    };

    thread::spawn(move || {
        let ui_weak_clone = ui_weak.clone();
        let config = FunASRModelConfig::default()
            .with_model_weights(setting.model_path.to_string())
            .with_tokenizer_path(setting.model_tokenizer_path.to_string());

        log::info!("Loading transcribe model: {config:?}");

        let vad_config = VadConfig::default()
            .with_min_silence_duration_ms(setting.mini_silent_period_duration.max(50) as u32);

        let audio_config = match load_audio_file(&filepath) {
            Ok(audio_config) => audio_config,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("load file `{}` failed: {e}", filepath.display()),
                );
                return;
            }
        };

        audio_player::set_current_audio_config(Some(audio_config.clone()));

        let mut model = match FunAsrNanoGenerateModel::new(config, None, None) {
            Ok(model) => model,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("New transcribe model failed: {e}"),
                );
                return;
            }
        };

        let request = fun_ast_nano::TranscriptionRequest::default()
            .with_audio_config(audio_config.clone())
            .with_prompt(Some(DEFAULT_PROMPT.to_string()))
            .with_max_tokens(512);

        let result = model.generate(request, Some(vad_config), move |chunk| {
            if let Some(ref stop_sig) = stop_sig
                && stop_sig.load(Ordering::Relaxed)
            {
                return Err(FunAsrError::TranscribeCancelled);
            }

            if !chunk.is_finished {
                if chunk.text.trim().is_empty() {
                    return Ok(());
                }

                if let Some(seg_info) = chunk.segment_info {
                    let start_timestamp =
                        ms_to_srt_timestamp(seg_info.segment_start_ms as u64).into();
                    let end_timestamp = ms_to_srt_timestamp(seg_info.segment_end_ms as u64).into();

                    let samples = extract_audio_samples(
                        &audio_config,
                        seg_info.segment_start_ms as u64,
                        seg_info.segment_end_ms as u64,
                    );
                    let samples = downsample_audio(&samples, MAX_WAVE_FORM_SAMPLE_COUNTS as usize);
                    let amplitude = max_sound_wave_amplitude(&samples);

                    _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                        let subtitle = UISubtitle {
                            start_timestamp,
                            end_timestamp,
                            original_text: chunk.text.into(),
                            correction_text: Default::default(),
                            audio_wave_amplitude: amplitude,
                            audio_samples: ModelRc::new(VecModel::from_slice(&samples)),
                            is_timestamp_overlap: false,
                        };

                        let mut entry = global_store!(ui).get_transcribe();
                        entry.progress_type = UITranscribeProgressType::Transcribe;
                        entry.progress =
                            seg_info.current_segment as f32 / seg_info.total_segments as f32;
                        store_transcribe_subtitles!(entry).push(subtitle);
                        global_store!(ui).set_transcribe(entry);
                    });
                }
            } else {
                let value = audio_config.clone();
                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_transcribe();
                    entry.progress_type = UITranscribeProgressType::Finished;
                    entry.progress = 1.0;
                    entry.media_duration_ms = value.duration.as_millis() as f32;

                    global_store!(ui).set_transcribe(entry.clone());
                    db_add(ui.as_weak(), entry.into());
                });
            }
            Ok(())
        });

        if inc_index != TRANSCRIBE_CACHE.lock().unwrap().inc_index {
            return;
        }

        match result {
            Err(FunAsrError::TranscribeCancelled) => {
                _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_transcribe();
                    entry.progress_type = UITranscribeProgressType::Cancelled;
                    global_store!(ui).set_transcribe(entry);
                });
            }
            Err(e) => {
                _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_transcribe();
                    entry.progress_type = UITranscribeProgressType::Failed;
                    global_store!(ui).set_transcribe(entry);
                    toast_warn!(ui, format!("transcribe failed: {e}"));
                });
            }
            _ => (),
        }
    });

    Ok(())
}

fn set_store_subtitles(entry: &UITranscribe, audio_config: &AudioConfig) {
    let mut audio_segments = entry
        .subtitles
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let start_timestamp = srt_timestamp_to_ms(&item.start_timestamp);
            let end_timestamp = srt_timestamp_to_ms(&item.end_timestamp);

            if start_timestamp.is_err() || end_timestamp.is_err() {
                None
            } else {
                Some(AudioSegment {
                    index: index as u32,
                    start_timestamp: Duration::from_millis(start_timestamp.unwrap()),
                    end_timestamp: Duration::from_millis(end_timestamp.unwrap()),
                    samples: vec![],
                })
            }
        })
        .collect::<Vec<_>>();

    gen_audio_segments(audio_config, &mut audio_segments);

    audio_segments.into_iter().for_each(|item| {
        if let Some(mut subtitle) = store_transcribe_subtitles!(entry).row_data(item.index as usize)
        {
            let samples = downsample_audio(&item.samples, MAX_WAVE_FORM_SAMPLE_COUNTS as usize);
            subtitle.audio_wave_amplitude = max_sound_wave_amplitude(&samples);
            subtitle.audio_samples = ModelRc::new(VecModel::from_slice(&samples));
            store_transcribe_subtitles!(entry).set_row_data(item.index as usize, subtitle);
        }
    });
}

fn transcribe_import_file(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(file_path) = picker_file(
            ui_weak.clone(),
            &tr("Choose media file"),
            &tr("e.g. mp4, mp3, wav"),
            &[],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut entry = global_store!(ui).get_transcribe();
            entry.file_path = file_path.to_string_lossy().to_string().into();
            entry.is_file_exist = true;
            global_store!(ui).set_transcribe(entry.clone());
            db_update(ui.as_weak(), entry.into());

            let ui_weak = ui.as_weak();
            std::thread::spawn(move || match load_audio_file(&file_path) {
                Ok(audio_config) => {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let mut entry = global_store!(ui).get_transcribe();
                        entry.media_duration_ms = audio_config.duration.as_millis() as f32;
                        set_store_subtitles(&entry, &audio_config);
                        global_store!(ui).set_transcribe(entry);

                        audio_player::set_current_audio_config(Some(audio_config));
                    });
                }
                Err(e) => {
                    audio_player::set_current_audio_config(None);
                    log::warn!("load `{}` failed: {e}", file_path.display());
                }
            });
        });
    });
}

fn transcribe_export_subtitles(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let filename = format!("{}.srt", cutil::fs::file_name_without_ext(&entry.file_path));

    let Some(items) = get_export_subtitles(ui) else {
        toast_warn!(ui, "Contain invalid `srt` timestamp".to_string());
        return;
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(path) = picker_directory(ui_weak.clone(), &tr("Export Subtitle"), &filename)
        else {
            return;
        };

        let path = path.join(filename);
        match save_as_srt(&items, path) {
            Err(e) => toast::async_toast_warn(ui_weak, format!("Export subtitle failed: {e}")),
            _ => toast::async_toast_success(ui_weak, "Export subtitle successfully".to_string()),
        }
    });
}

fn get_export_subtitles(ui: &AppWindow) -> Option<Vec<ExportSubtitle>> {
    let mut items = vec![];
    let entry = global_store!(ui).get_transcribe();

    for (index, item) in store_transcribe_subtitles!(entry).iter().enumerate() {
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

fn transcribe_refresh_subtitles(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let filepath = PathBuf::from(&entry.file_path);

    if !filepath.exists() {
        toast_warn!(ui, format!("File not found: {}", filepath.display()));
        return;
    }

    match get_current_audio_config() {
        Some(audio_config) => {
            set_store_subtitles(&entry, &audio_config);
            audio_player::set_current_audio_config(Some(audio_config));
            toast_success!(ui, "Refresh subtitles successfully".to_string());
        }
        None => toast_warn!(
            ui,
            format!("Refresh subtitles failed. No cache audio samples")
        ),
    }
}

fn transcribe_cancel_progress(ui: &AppWindow, ty: UITranscribeProgressType) {
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
        _ => {
            todo!()
        }
    }

    let mut entry = global_store!(ui).get_transcribe();
    entry.progress_type = UITranscribeProgressType::Cancelled;
    global_store!(ui).set_transcribe(entry);
}

fn transcribe_subtitles_recovery(ui: &AppWindow) {
    db_select(ui.as_weak(), TRANSCRIBE_ID, true, |ui, entry| {
        let mut entry: UITranscribe = entry.into();
        let subtitles_counts = entry.subtitles.row_count();
        let is_file_exist = PathBuf::from(&entry.file_path).exists();
        let file_path = entry.file_path.clone();

        entry.playing_index = -1;
        entry.is_file_exist = is_file_exist;
        global_store!(ui).set_transcribe(entry);

        if !is_file_exist && subtitles_counts > 0 {
            ui.global::<UIConfirmDialogSetting>().invoke_set(
                true,
                tr("Warning").into(),
                tr("No found media file. Import file or not?").into(),
                "transcribe-import-file".into(),
                "".into(),
            );
        }

        if !is_file_exist {
            return;
        }

        let ui_weak = ui.as_weak();
        std::thread::spawn(move || match load_audio_file(&file_path) {
            Ok(audio_config) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_transcribe();
                    entry.media_duration_ms = audio_config.duration.as_millis() as f32;
                    set_store_subtitles(&entry, &audio_config);
                    global_store!(ui).set_transcribe(entry);

                    audio_player::set_current_audio_config(Some(audio_config));
                });
            }
            Err(e) => {
                audio_player::set_current_audio_config(None);
                log::warn!("load `{}` failed: {e}", file_path);
            }
        });
    });
}

fn transcribe_subtitles_remove_all(ui: &AppWindow) {
    global_store!(ui).set_transcribe(UITranscribe::default());
    global_store!(ui).set_transcribe_can_recovered(false);
    db_remove_all(ui.as_weak());
}

fn transcribe_subtitles_correction(ui: &AppWindow) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, "Please setup AI model and try again.".to_string());
        return;
    }

    let mut entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
        toast_info!(ui, "All subtitles already have corrections or are empty");
        return;
    }

    entry.progress = 0.0;
    entry.progress_type = UITranscribeProgressType::CorrectSubtitles;
    global_store!(ui).set_transcribe(entry);

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
                        let mut entry = global_store!(ui).get_transcribe();
                        let subtitles = store_transcribe_subtitles!(entry);

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

                        store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
                        global_store!(ui).set_transcribe(entry.clone());
                        db_update(ui.as_weak(), entry.into());
                    });
                }
                Err(e) => toast::async_toast_warn(
                    ui_weak,
                    format!(" Chunck[{chunk_index}] AI correctionfailed: {e}"),
                ),
            }
        });
    }
}

async fn ai_correct_subtitles(
    ui_weak: Weak<AppWindow>,
    subtitles: Vec<(usize, String)>,
    stop_sig: Arc<AtomicBool>,
) -> Result<HashMap<usize, String>> {
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
            toast::async_toast_warn(ui_weak, format!("Start AI correction failed: {e}"));
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

fn transcribe_subtitles_accept_correction(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

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

    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    db_update(ui.as_weak(), entry.into());
}

fn transcribe_subtitles_remove_correction(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.correction_text = SharedString::default();
            subtitle
        })
        .collect::<Vec<_>>();

    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    db_update(ui.as_weak(), entry.into());
}

fn transcribe_subtitles_adjust_overlap_timestamp(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
                store_transcribe_subtitles!(entry).set_row_data(index, current);

                next.start_timestamp = ms_to_srt_timestamp(end_ms).into();
                next.is_timestamp_overlap = false;
                store_transcribe_subtitles!(entry).set_row_data(index + 1, next);
            }
        }
    }

    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, &tr("Adjust overlap timestamp successfully"));
}

fn transcribe_subtitles_to_lowercase(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.original_text = subtitle.original_text.to_lowercase().into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Convert to lowercase successfully");
}

fn transcribe_subtitles_to_simple_chinese(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            subtitle.original_text = fast2s::convert(&subtitle.original_text).into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Convert to simplified Chinese successfully");
}

fn transcribe_subtitles_to_primitive_numbers(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    store_transcribe_subtitles!(entry)
        .iter()
        .enumerate()
        .for_each(|(index, mut subtitle)| {
            let converted_text = chinese_numbers_to_primitive_numbers(&subtitle.original_text);
            subtitle.original_text = converted_text.into();
            store_transcribe_subtitles!(entry).set_row_data(index, subtitle);
            toast_success!(ui, "Convert to primitive numbers successfully");
        });
}

fn transcribe_subtitles_remove_separator(ui: &AppWindow) {
    let separators = [',', '，', '。'];
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    let updated_subtitles = subtitles
        .iter()
        .map(|mut subtitle| {
            let mut result = subtitle.original_text.to_string();
            for sep in &separators {
                result = result.replace(*sep, " ");
            }
            subtitle.original_text = result.trim().to_string().into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Remove separators successfully".to_string());
}

fn transcribe_subtitles_replace_text(
    ui: &AppWindow,
    old_text: SharedString,
    new_text: SharedString,
) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry)
        .iter()
        .map(|mut entry| {
            entry.original_text = entry
                .original_text
                .replace(old_text.as_str(), new_text.as_str())
                .into();
            entry
        })
        .collect::<Vec<_>>();

    store_transcribe_subtitles!(entry).set_vec(subtitles);
    toast_success!(ui, "Replace content of subtitles successfully");

    db_update(ui.as_weak(), entry.into());
}

fn transcribe_subtitles_update_playng_index(ui: &AppWindow, progress: f32) {
    let mut entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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

    global_store!(ui).set_transcribe(entry);
}

fn transcribe_subtitle_update(ui: &AppWindow, index: i32, text: SharedString) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let mut subtitle = store_transcribe_subtitles!(entry).row_data(index).unwrap();

    subtitle.original_text = text;
    store_transcribe_subtitles!(entry).set_row_data(index, subtitle);
    toast_success!(ui, "Update subtitle successfully");

    db_update(ui.as_weak(), entry.into());
}

fn transcribe_subtitle_accept_correction(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    if let Some(mut subtitle) = subtitles.row_data(index)
        && !subtitle.correction_text.is_empty()
    {
        subtitle.original_text = subtitle.correction_text.clone();
        store_transcribe_subtitles!(entry).set_row_data(index, subtitle);
        db_update(ui.as_weak(), entry.into());
    }
}

fn transcribe_subtitle_split(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitle = store_transcribe_subtitles!(entry).row_data(index).unwrap();

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
        toast_warn!(ui, "split subtitle failed");
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

    store_transcribe_subtitles!(entry).set_row_data(index, current_subtitle);
    store_transcribe_subtitles!(entry).insert(index + 1, next_subtitle);
    global_logic!(ui)
        .invoke_transcribe_sound_wave_update(index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
    global_logic!(ui)
        .invoke_transcribe_sound_wave_update(index as i32 + 1, MAX_WAVE_FORM_SAMPLE_COUNTS);

    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Split subtitle successfully");
}

fn transcribe_subtitle_merge_above(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(&entry);

    if index == 0 {
        return;
    }

    let mut prev = subtitles.row_data(index - 1).unwrap();
    let current = subtitles.row_data(index).unwrap();

    prev.end_timestamp = current.end_timestamp;
    prev.original_text = format!("{}{}", prev.original_text, current.original_text).into();
    prev.correction_text = format!("{}{}", prev.correction_text, current.correction_text).into();
    prev.audio_wave_amplitude = prev.audio_wave_amplitude.max(current.audio_wave_amplitude);

    let mut samples = prev.audio_samples.iter().collect::<Vec<_>>();
    samples.extend_from_slice(&current.audio_samples.iter().collect::<Vec<_>>());
    prev.audio_samples = ModelRc::new(VecModel::from_slice(&samples));

    store_transcribe_subtitles!(entry).set_row_data(index - 1, prev);
    store_transcribe_subtitles!(entry).remove(index);
    global_logic!(ui)
        .invoke_transcribe_sound_wave_update(index as i32 - 1, MAX_WAVE_FORM_SAMPLE_COUNTS);

    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Merge subtitle successfully");
}

fn transcribe_subtitle_insert_above(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

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

    store_transcribe_subtitles!(entry).insert(index, new_subtitle);
    global_logic!(ui)
        .invoke_transcribe_sound_wave_update(index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);

    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Insert subtitle successfully");

    if index == 0 {
        mark_overlapped_timestamp(ui, index);
    }
}

fn transcribe_subtitle_insert_below(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

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

    store_transcribe_subtitles!(entry).insert(index + 1, new_subtitle);
    global_logic!(ui)
        .invoke_transcribe_sound_wave_update(index as i32 + 1, MAX_WAVE_FORM_SAMPLE_COUNTS);

    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Insert subtitle successfully");
}

fn transcribe_subtitle_remove(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();

    store_transcribe_subtitles!(entry).remove(index);
    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Remove subtitle successfully");
}

pub fn mark_overlapped_timestamp(ui: &AppWindow, index: usize) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
        let mut subtitle = store_transcribe_subtitles!(entry)
            .row_data(index - 1)
            .unwrap();

        if current_start_ms < prev_end_ms {
            has_overlap = true;
            subtitle.is_timestamp_overlap = true;
        } else {
            subtitle.is_timestamp_overlap = false;
        }

        store_transcribe_subtitles!(entry).set_row_data(index - 1, subtitle);
    }

    if index < total - 1
        && let Some(next) = subtitles.row_data(index + 1)
        && let Ok(next_start_ms) = srt_timestamp_to_ms(&next.start_timestamp)
    {
        let mut subtitle = store_transcribe_subtitles!(entry)
            .row_data(index + 1)
            .unwrap();

        if current_end_ms > next_start_ms {
            has_overlap = true;
            subtitle.is_timestamp_overlap = true;
        } else {
            subtitle.is_timestamp_overlap = false;
        }

        store_transcribe_subtitles!(entry).set_row_data(index + 1, subtitle);
    }

    let mut subtitle = store_transcribe_subtitles!(entry).row_data(index).unwrap();
    subtitle.is_timestamp_overlap = has_overlap;
    store_transcribe_subtitles!(entry).set_row_data(index, subtitle);
    db_update(ui.as_weak(), entry.into());
}
