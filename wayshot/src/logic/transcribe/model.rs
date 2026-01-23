use crate::{
    config,
    db::{TRANSCRIBE_TABLE as DB_TABLE, Transcribe},
    global_logic, global_store,
    logic::{share_screen::picker_file, toast, tr::tr, transcribe::audio_player},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, FileType as UIFileType, Subtitle as UISubtitle, Transcribe as UITranscribe,
        TranscribeProgressType as UITranscribeProgressType,
    },
    toast_warn,
};
use anyhow::Result;
use audio_utils::audio::{AudioConfig, AudioSegment, gen_audio_segments};
use fun_ast_nano::{
    FunASRModelConfig, FunAsrError, FunAsrNanoGenerateModel, VadConfig, load_audio_file,
};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const TRANSCRIBE_ID: &str = "transcribe_id";
static TRANSCRIBE_CACHE: Lazy<Mutex<TranscribeCache>> =
    Lazy::new(|| Mutex::new(TranscribeCache::default()));

#[derive(Default, Clone)]
struct TranscribeCache {
    transcribe_stop_sig: Option<Arc<AtomicBool>>,
}

#[macro_export]
macro_rules! store_transcribe_subtitles {
    ( $ui:expr, $transcribe: expr) => {
        $transcribe
            .subtitles
            .as_any()
            .downcast_ref::<VecModel<UISubtitle>>()
            .expect("We know we set a VecModel<UISubtitle> earlier")
    };
}

#[macro_export]
macro_rules! store_transcribe_subtitle_audio_samples {
    ( $ui:expr, $subtitle: expr) => {
        $subtitle
            .audio_samples
            .as_any()
            .downcast_ref::<VecModel<f32>>()
            .expect("We know we set a VecModel<f32> earlier")
    };
}

crate::db_remove_all!(DB_TABLE);
crate::db_add!(DB_TABLE, Transcribe);
crate::db_update!(DB_TABLE, Transcribe);
crate::db_select!(DB_TABLE, Transcribe);

pub fn init(ui: &AppWindow) {
    logic_cb!(file_exist, ui, file);
    logic_cb!(is_valid_subtitle_timestamp, ui, timestamp);
    logic_cb!(transcribe_init, ui);
    logic_cb!(transcribe_new, ui);

    // callback transcribe-start();
    // callback transcribe-import-file();
    // callback transcribe-export-video();
    // callback transcribe-export-subtitles();
    // callback transcribe-refresh-subtitles();
    // callback transcribe-cancel-progress(ty: TranscribeProgressType);
    // callback transcribe-save-subtitle(index: int);
    // callback transcribe-accept-correction(index: int);
}

pub fn transcribe_init(ui: &AppWindow) {
    let config = config::all().transcribe;
    global_store!(ui).set_transcribe_setting(config.into());

    let transcribe = global_store!(ui).get_transcribe();
    store_transcribe_subtitles!(ui, transcribe).set_vec(vec![]);

    db_select(ui.as_weak(), TRANSCRIBE_ID, |ui, entry| {
        let mut entry: UITranscribe = entry.into();
        entry.is_file_exist = PathBuf::from(&entry.file_path).exists();
        if entry.is_file_exist {
            match load_audio_file(&entry.file_path) {
                Ok(audio_config) => {
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
            let start_timestamp = cutil::time::media_timestamp_to_ms(&item.start_timestamp);
            let end_timestamp = cutil::time::media_timestamp_to_ms(&item.end_timestamp);

            if start_timestamp.is_none() || end_timestamp.is_none() {
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
        if let Some(mut subtitle) =
            store_transcribe_subtitles!(ui, entry).row_data(item.index as usize)
        {
            subtitle.audio_wave_amplitude = audio_player::get_sound_wave_amplitude(&item.samples);
            subtitle.audio_samples = ModelRc::new(VecModel::from_slice(&item.samples));
            store_transcribe_subtitles!(ui, entry).set_row_data(item.index as usize, subtitle);
        }
    });
}

fn file_exist(_ui: &AppWindow, file: SharedString) -> bool {
    cutil::fs::file_exist(file)
}

fn is_valid_subtitle_timestamp(_ui: &AppWindow, timestamp: SharedString) -> bool {
    cutil::time::date_str_to_timestamp(&timestamp).is_ok()
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
            let file_type = get_file_type(&filepath);
            let entry = UITranscribe {
                id: TRANSCRIBE_ID.to_string().into(),
                file_path: filepath.to_string_lossy().to_string().into(),
                is_file_exist: true,
                file_type,
                subtitles: ModelRc::new(VecModel::from_slice(&[])),
                playing_index: -1,
                ..Default::default()
            };
            global_store!(ui).set_transcribe(entry);

            if let Err(e) = inner_trancribe_new(&ui, filepath) {
                toast_warn!(ui, format!("Start transcribe failed: {e}"));
            }
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

fn inner_trancribe_new(ui: &AppWindow, filepath: PathBuf) -> Result<()> {
    let setting = global_store!(ui).get_transcribe_setting();
    let config = FunASRModelConfig::default()
        .with_model_weights(setting.model.model_path.to_string())
        .with_tokenizer_path(setting.model.model_tokenizer_path.to_string());

    let vad_config = VadConfig::default()
        .with_min_silence_duration_ms(setting.model.mini_silent_period_duration.max(50) as u32);

    let audio_config = load_audio_file(filepath)?;

    log::info!("Loading transcribe model: {config:?}");
    let mut model = FunAsrNanoGenerateModel::new(config, None, None)?;

    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_config(audio_config.clone())
        .with_prompt(Some("Transcribe the audio to text.".to_string()))
        .with_max_tokens(512);

    let ui_weak = ui.as_weak();
    let stop_sig = TRANSCRIBE_CACHE.lock().unwrap().transcribe_stop_sig.clone();
    thread::spawn(move || {
        let mut total_tokens = 0;
        let ui_weak_clone = ui_weak.clone();

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
                total_tokens += chunk.num_tokens;

                if let Some(seg_info) = chunk.segment_info {
                    let start_timestamp = cutil::time::seconds_to_media_timestamp_with_ms(
                        seg_info.segment_start_ms as f64 / 1000.0,
                    )
                    .into();
                    let end_timestamp = cutil::time::seconds_to_media_timestamp_with_ms(
                        seg_info.segment_end_ms as f64 / 1000.0,
                    )
                    .into();

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
                        store_transcribe_subtitles!(ui, entry).push(subtitle);
                        global_store!(ui).set_transcribe(entry);
                    });
                }
            } else {
                let value = audio_config.clone();
                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    let mut entry = global_store!(ui).get_transcribe();
                    entry.progress_type = UITranscribeProgressType::Finished;
                    entry.progress = 1.0;

                    // set audio_wave_amplitude and audio_samples
                    set_store_subtitles(&entry, value);

                    global_store!(ui).set_transcribe(entry);
                });
            }
            Ok(())
        });

        match result {
            Ok(_) => {
                toast::async_toast_success(ui_weak_clone, "Transcription successful".to_string());
                log::info!("transcribe spent {total_tokens} tokens");
            }
            Err(FunAsrError::TranscribeCancelled) => {
                toast::async_toast_info(ui_weak_clone, "Transcription cancelled".to_string());
            }
            Err(e) => toast::async_toast_warn(ui_weak_clone, format!("Transcription failed: {e}")),
        }
    });

    Ok(())
}
