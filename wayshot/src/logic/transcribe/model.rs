use crate::{
    config,
    db::{TRANSCRIBE_TABLE as DB_TABLE, Transcribe},
    global_store,
    logic::{
        recorder::picker_directory,
        share_screen::picker_file,
        toast,
        tr::tr,
        transcribe::audio_player::{self, get_current_audio_config},
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, FileType as UIFileType, Subtitle as UISubtitle, Transcribe as UITranscribe,
        TranscribeProgressType as UITranscribeProgressType,
    },
    store_transcribe_subtitle_audio_samples, toast_info, toast_success, toast_warn,
};

use anyhow::Result;
use audio_utils::{
    audio::{AudioConfig, AudioSegment, gen_audio_segments, load_audio_file},
    vad::VadConfig,
};
use fun_ast_nano::{FunASRModelConfig, FunAsrError, FunAsrNanoGenerateModel};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use video_utils::subtitle::{
    Subtitle as ExportSubtitle, ms_to_srt_timestamp, save_as_srt, split_subtitle_into_two,
    srt_timestamp_to_ms, valid_srt_timestamp,
};

const TRANSCRIBE_ID: &str = "transcribe_id";
static TRANSCRIBE_CACHE: Lazy<Mutex<TranscribeCache>> =
    Lazy::new(|| Mutex::new(TranscribeCache::default()));

crate::db_remove_all!(DB_TABLE);
crate::db_add!(DB_TABLE, Transcribe);
crate::db_update!(DB_TABLE, Transcribe);
crate::db_select!(DB_TABLE, Transcribe);

#[derive(Default, Clone)]
struct TranscribeCache {
    transcribe_stop_sig: Option<Arc<AtomicBool>>,
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
    logic_cb!(transcribe_init, ui);
    logic_cb!(transcribe_new, ui);
    logic_cb!(transcribe_start, ui);
    logic_cb!(transcribe_import_file, ui);
    logic_cb!(transcribe_export_subtitles, ui);
    logic_cb!(transcribe_refresh_subtitles, ui);
    logic_cb!(transcribe_cancel_progress, ui, ty);

    logic_cb!(transcribe_subtitles_correction, ui);
    logic_cb!(transcribe_subtitles_remove_correction, ui);
    logic_cb!(transcribe_subtitles_adjust_overlap_timestamp, ui);
    logic_cb!(transcribe_subtitles_to_lowercase, ui);
    logic_cb!(transcribe_subtitles_to_simple_chinese, ui);
    logic_cb!(transcribe_subtitles_remove_separator, ui);

    logic_cb!(transcribe_subtitle_update, ui, index, subtitle);
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

    let transcribe = global_store!(ui).get_transcribe();
    store_transcribe_subtitles!(transcribe).set_vec(vec![]);

    db_select(ui.as_weak(), TRANSCRIBE_ID, |ui, entry| {
        let mut entry: UITranscribe = entry.into();
        entry.is_file_exist = PathBuf::from(&entry.file_path).exists();
        if entry.is_file_exist {
            match load_audio_file(&entry.file_path) {
                Ok(audio_config) => {
                    entry.media_duration_ms = audio_config.duration.as_millis() as f32;
                    set_store_subtitles(&entry, audio_config);
                }
                Err(e) => {
                    audio_player::set_current_audio_config(None);
                    log::warn!("load {} failed: {e}", entry.file_path);
                }
            }
        }
        global_store!(ui).set_transcribe(entry);
    });
}

fn set_store_subtitles(entry: &UITranscribe, audio_config: AudioConfig) {
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

    gen_audio_segments(&audio_config, &mut audio_segments);
    audio_player::set_current_audio_config(Some(audio_config));

    audio_segments.into_iter().for_each(|item| {
        if let Some(mut subtitle) = store_transcribe_subtitles!(entry).row_data(item.index as usize)
        {
            subtitle.audio_wave_amplitude = audio_player::get_sound_wave_amplitude(&item.samples);
            subtitle.audio_samples = ModelRc::new(VecModel::from_slice(&item.samples));
            store_transcribe_subtitles!(entry).set_row_data(item.index as usize, subtitle);
        }
    });
}

fn file_exist(_ui: &AppWindow, file: SharedString) -> bool {
    cutil::fs::file_exist(file)
}

fn is_valid_subtitle_timestamp(_ui: &AppWindow, timestamp: SharedString) -> bool {
    valid_srt_timestamp(&timestamp)
}

fn reset_transcribe_stop_sig() {
    let mut cache = TRANSCRIBE_CACHE.lock().unwrap();
    if let Some(stop_sig) = cache.transcribe_stop_sig.take() {
        stop_sig.store(true, Ordering::Relaxed);
    }
    cache.transcribe_stop_sig = Some(Arc::new(AtomicBool::new(false)));
}

fn transcribe_new(ui: &AppWindow) {
    db_remove_all(ui.as_weak());
    reset_transcribe_stop_sig();

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
        _ => UIFileType::Audio,
    }
}

fn transcribe_start(ui: &AppWindow) {
    let mut entry = global_store!(ui).get_transcribe();
    let filepath = PathBuf::from(&entry.file_path);

    if filepath.exists() {
        if !entry.is_file_exist {
            entry.is_file_exist = true;
            global_store!(ui).set_transcribe(entry);
        }
    } else {
        toast_warn!(ui, format!("No found {}", filepath.display()));
        return;
    }

    if let Err(e) = inner_trancribe_start(&ui, filepath) {
        toast_warn!(ui, format!("Start transcribe failed: {e}"));
    }
}

fn inner_trancribe_start(ui: &AppWindow, filepath: PathBuf) -> Result<()> {
    let ui_weak = ui.as_weak();
    let setting = global_store!(ui).get_transcribe_setting();
    let stop_sig = TRANSCRIBE_CACHE.lock().unwrap().transcribe_stop_sig.clone();

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
            .with_prompt(Some("Transcribe the audio to text.".to_string()))
            .with_max_tokens(512);

        let result = model.generate(request, Some(vad_config), move |chunk| {
            if let Some(ref stop_sig) = stop_sig
                && stop_sig.load(Ordering::Relaxed)
            {
                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_transcribe();
                    entry.progress_type = UITranscribeProgressType::Cancelled;
                    global_store!(ui).set_transcribe(entry);
                });
                return Err(FunAsrError::TranscribeCancelled);
            }

            if !chunk.is_finished {
                if let Some(seg_info) = chunk.segment_info {
                    let start_timestamp =
                        ms_to_srt_timestamp(seg_info.segment_start_ms as u64).into();
                    let end_timestamp = ms_to_srt_timestamp(seg_info.segment_end_ms as u64).into();

                    _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                        let subtitle = UISubtitle {
                            start_timestamp,
                            end_timestamp,
                            original_text: chunk.text.into(),
                            correction_text: Default::default(),
                            audio_wave_amplitude: 1.0, // placeholder
                            audio_samples: ModelRc::new(VecModel::from_slice(&[])), // placeholder
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

                    // set audio_wave_amplitude and audio_samples
                    set_store_subtitles(&entry, value);

                    global_store!(ui).set_transcribe(entry.clone());
                    db_add(ui.as_weak(), entry.into());
                });
            }
            Ok(())
        });

        match result {
            Err(FunAsrError::TranscribeCancelled) => {
                toast::async_toast_info(ui_weak_clone, "Transcription cancelled".to_string())
            }
            Err(e) => toast::async_toast_warn(ui_weak_clone, format!("Transcription failed: {e}")),
            _ => toast::async_toast_success(ui_weak_clone, "Transcription successful".to_string()),
        }
    });

    Ok(())
}

fn transcribe_import_file(ui: &AppWindow) {
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
            let mut entry = global_store!(ui).get_transcribe();
            entry.file_path = filepath.to_string_lossy().to_string().into();
            entry.is_file_exist = true;
            global_store!(ui).set_transcribe(entry);
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
            set_store_subtitles(&entry, audio_config);
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

            let mut entry = global_store!(ui).get_transcribe();
            entry.progress_type = UITranscribeProgressType::Cancelled;
            global_store!(ui).set_transcribe(entry);
        }
        _ => {
            todo!()
        }
    }
}

fn transcribe_subtitles_correction(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
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

    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        match ai_correct_subtitles(&subtitles_to_correct).await {
            Ok(corrections) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let entry = global_store!(ui).get_transcribe();
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

                    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
                    db_update(ui.as_weak(), entry.into());
                    toast_success!(ui, "AI correction completed".to_string());
                });
            }
            Err(e) => toast::async_toast_warn(ui_weak, format!("AI correction failed: {e}")),
        }
    });
}

// TODO:
async fn ai_correct_subtitles(_subtitles: &[(usize, String)]) -> Result<HashMap<usize, String>> {
    Ok(HashMap::new())
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
    toast_success!(ui, "Remove subtitle corrections successfully".to_string());
}

fn transcribe_subtitles_adjust_overlap_timestamp(ui: &AppWindow) {
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
    let total = subtitles.row_count();

    for index in 0..total {
        if index == total - 1 {
            break;
        }

        let current = subtitles.row_data(index).unwrap();
        let mut next = subtitles.row_data(index + 1).unwrap();

        let current_end_ms = srt_timestamp_to_ms(&current.end_timestamp);
        let next_start_ms = srt_timestamp_to_ms(&next.start_timestamp);

        if let (Ok(end_ms), Ok(next_ms)) = (current_end_ms, next_start_ms) {
            if end_ms > next_ms {
                next.start_timestamp = ms_to_srt_timestamp(end_ms).into();
                store_transcribe_subtitles!(entry).set_row_data(index + 1, next);
            }
        }
    }

    db_add(ui.as_weak(), entry.into());
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
            subtitle.original_text = result.into();
            subtitle
        })
        .collect::<Vec<_>>();

    store_transcribe_subtitles!(entry).set_vec(updated_subtitles);
    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Remove separators successfully".to_string());
}

fn transcribe_subtitle_update(ui: &AppWindow, index: i32, subtitle: UISubtitle) {
    let entry = global_store!(ui).get_transcribe();
    store_transcribe_subtitles!(entry).set_row_data(index as usize, subtitle);
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

    let Some((first_part, second_part)) = split_subtitle_into_two(
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
    store_transcribe_subtitle_audio_samples!(prev).set_vec(samples);

    store_transcribe_subtitles!(entry).set_row_data(index - 1, prev);
    store_transcribe_subtitles!(entry).remove(index);

    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Merge subtitle successfully");
}

fn transcribe_subtitle_insert_above(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    let new_subtitle = if index == 0 {
        let first = subtitles.row_data(0).unwrap();
        UISubtitle {
            start_timestamp: "00:00:00,000".into(),
            end_timestamp: first.start_timestamp.clone(),
            original_text: "Click to edit".to_string().into(),
            correction_text: Default::default(),
            audio_samples: ModelRc::new(VecModel::from_slice(&[])),
            audio_wave_amplitude: 1.0,
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
        }
    };

    store_transcribe_subtitles!(entry).insert(index, new_subtitle);
    db_update(ui.as_weak(), entry.into());
    toast_success!(ui, "Insert subtitle successfully");
}

fn transcribe_subtitle_insert_below(ui: &AppWindow, index: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);

    let new_subtitle = if index == subtitles.row_count() - 1 {
        let last = subtitles.row_data(subtitles.row_count() - 1).unwrap();
        UISubtitle {
            start_timestamp: last.end_timestamp.clone(),
            end_timestamp: last.end_timestamp.clone(),
            original_text: "Click to edit".into(),
            correction_text: Default::default(),
            audio_samples: ModelRc::new(VecModel::from_slice(&[])),
            audio_wave_amplitude: 1.0,
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
        }
    };

    store_transcribe_subtitles!(entry).insert(index + 1, new_subtitle);
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
