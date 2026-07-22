use crate::{
    db::{SpeakersConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory,
        share_screen::picker_file,
        toast::{self, async_toast_info, async_toast_warn},
        tr::tr,
        video_editor::{playlist::import_file_to_playlist, project::SPEAKERS_CONFIG_ID},
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, AudioPlayerStatus as UIAudioPlayerStatus, Downloader as UIDownloader,
        DownloaderState as UIDownloaderState, VideoEditorSpeakersConfig as UISpeakersConfig,
    },
};
use anyhow::Result;
use audio_utils::loader::load_audio_file_and_convert;
use rodio::{ChannelCount, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer};
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel, Weak};
use speakrs::{ExecutionMode, ModelManager, OwnedDiarizationPipeline, segment::merge_segments};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicI32, AtomicU32, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
struct AudioData {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

const SPEAKRS_MODELS_URL: &str = "https://huggingface.co/avencera/speakrs-models";
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "wav", "mp3", "flac", "ogg", "m4a", "aac", "wma",
];

static IMPORTED_AUDIO_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
static IMPORTED_AUDIO_DATA: Mutex<Option<AudioData>> = Mutex::new(None);
static IMPORTED_AUDIO_16K: Mutex<Option<Vec<f32>>> = Mutex::new(None);
static SPLIT_RESULT_CACHE: Mutex<Option<BTreeMap<String, AudioData>>> = Mutex::new(None);
static AUDIO_DEVICE: OnceLock<Arc<MixerDeviceSink>> = OnceLock::new();
static CURRENT_PLAYER: OnceLock<Mutex<Option<Arc<Player>>>> = OnceLock::new();
static CURRENT_PLAYING_INDEX: AtomicI32 = AtomicI32::new(-1);
static INC_INDEX: AtomicU32 = AtomicU32::new(0);
static PROCESS_INC_INDEX: AtomicU32 = AtomicU32::new(0);

#[macro_export]
macro_rules! store_video_editor_speakers_audio_player_status {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_speakers_audio_player_status()
            .as_any()
            .downcast_ref::<VecModel<UIAudioPlayerStatus>>()
            .expect(
                "We know we set a VecModel<UIAudioPlayerStatus> earlier for video editor speakers",
            )
    };
}

#[macro_export]
macro_rules! store_video_editor_speakers_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_speakers_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect(
                "We know we set a VecModel<UIDownloader> earlier for video editor speakers models",
            )
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_speakers_import_audio, ui);
    logic_cb!(video_editor_speakers_process_audio, ui);
    logic_cb!(video_editor_speakers_export_all_speakers, ui);
    logic_cb!(video_editor_speakers_export_speaker, ui, speaker_name);
    logic_cb!(video_editor_speakers_remove_all, ui);
    logic_cb!(video_editor_speakers_choose_export_dir, ui);
    logic_cb!(video_editor_speakers_choose_models_dir, ui);
    logic_cb!(video_editor_speakers_update_config, ui, config);
    logic_cb_pure!(video_editor_speakers_setting_is_valid, ui);
    logic_cb!(video_editor_speakers_play_audio, ui, index);
    logic_cb!(video_editor_speakers_stop_audio, ui, index);
    logic_cb!(
        video_editor_speakers_audio_player_sound_changed,
        ui,
        index,
        sound
    );
    logic_cb!(
        video_editor_speakers_audio_player_sound_released,
        ui,
        index,
        sound
    );
    logic_cb!(
        video_editor_speakers_audio_player_progress_released,
        ui,
        index,
        value
    );
    logic_cb!(video_editor_speakers_model_start_download, ui, index);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = vec![UIDownloader {
        url: SPEAKRS_MODELS_URL.into(),
        filename: "speakrs-models".into(),
        state: UIDownloaderState::UnStart,
        progress: 0.0,
    }];
    store_video_editor_speakers_models_downloader!(ui).set_vec(downloaders);

    let device =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open default audio device");
    AUDIO_DEVICE
        .set(Arc::new(device))
        .ok()
        .expect("AUDIO_DEVICE already initialized");
    CURRENT_PLAYER
        .set(Mutex::new(None))
        .ok()
        .expect("CURRENT_PLAYER already initialized");

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_speakers_config()
            .await
            .unwrap_or_else(|| SpeakersConfigData {
                id: SPEAKERS_CONFIG_ID.to_string(),
                merge_gap: 0.5,
                timeline_mode: false,
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_speakers_config(config.into());
        });
    });
}

fn save_speakers_config(config: SpeakersConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize speakers config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, SPEAKERS_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, SPEAKERS_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save speakers config: {:?}", e);
            }
        }
    });
}

async fn load_speakers_config() -> Option<SpeakersConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, SPEAKERS_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_speakers_import_audio(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let sound =
        (global_store!(ui).get_video_editor_ui_state().preview_volume / 100.0).clamp(0.0, 1.0);

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Select audio"),
            &tr("Audio Files"),
            &AUDIO_EXTENSIONS,
        ) else {
            return;
        };

        async_toast_info(
            ui_weak.clone(),
            tr("Please wait for loading audio file finished"),
        );

        let audio_config = match load_audio_file_and_convert(&filepath, 2, 44100) {
            Ok(config) => config,
            Err(e) => {
                async_toast_warn(ui_weak, format!("{}: {e}", tr("Failed to read audio file")));
                return;
            }
        };

        let audio = AudioData {
            samples: audio_config.samples,
            sample_rate: audio_config.sample_rate,
            channels: audio_config.channel,
        };
        let duration_ms = audio_config.duration.as_millis() as i32;

        let audio_16k_config = match load_audio_file_and_convert(&filepath, 1, 16000) {
            Ok(config) => config,
            Err(e) => {
                async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("Failed to convert audio for processing")),
                );
                return;
            }
        };

        *IMPORTED_AUDIO_PATH.lock().unwrap() = Some(filepath.clone());
        *IMPORTED_AUDIO_DATA.lock().unwrap() = Some(audio);
        *IMPORTED_AUDIO_16K.lock().unwrap() = Some(audio_16k_config.samples);

        let filename = filepath
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("source")
            .to_string();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let status = UIAudioPlayerStatus {
                name: filename.into(),
                sound,
                progress: 0.0,
                duration_ms,
                is_playing: false,
                is_processing: false,
            };
            store_video_editor_speakers_audio_player_status!(ui).set_vec(vec![status]);
        });
    });
}

fn video_editor_speakers_process_audio(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_speakers_config();
    let models_dir = config.models_dir.to_string();
    let merge_gap = config.merge_gap as f64;
    let timeline_mode = config.timeline_mode;

    if !models_dir.is_empty() {
        let path = PathBuf::from(&models_dir);
        if !path.exists() {
            global_store!(ui).set_video_editor_is_show_speakers_setting_dialog(true);
            crate::toast_warn!(ui, tr("Models directory not found"));
            return;
        }
    }

    let audio_16k = {
        let guard = IMPORTED_AUDIO_16K.lock().unwrap();
        match guard.clone() {
            Some(a) => a,
            None => {
                crate::toast_warn!(ui, tr("No audio imported"));
                return;
            }
        }
    };

    let ui_weak = ui.as_weak();
    global_store!(ui).set_video_editor_speakers_is_progressing(true);
    global_store!(ui).set_video_editor_speakers_progress(0.0);

    let source_meta = store_video_editor_speakers_audio_player_status!(ui)
        .row_data(0)
        .map(|s| (s.name.to_string(), s.sound, s.duration_ms));

    let inc_index = PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;

    tokio::task::spawn_blocking(move || {
        let pipeline = if models_dir.is_empty() {
            match OwnedDiarizationPipeline::from_pretrained(ExecutionMode::Cpu) {
                Ok(p) => p,
                Err(e) => {
                    async_toast_warn(
                        ui_weak.clone(),
                        format!("{}: {e}", tr("Failed to load models (auto-download)")),
                    );
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                            return;
                        }
                        global_store!(ui).set_video_editor_speakers_is_progressing(false);
                    });
                    return;
                }
            }
        } else {
            let dir = PathBuf::from(&models_dir);
            match OwnedDiarizationPipeline::from_dir(dir, ExecutionMode::Cpu) {
                Ok(p) => p,
                Err(e) => {
                    async_toast_warn(
                        ui_weak.clone(),
                        format!("{}: {e}", tr("Failed to load models from directory")),
                    );
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                            return;
                        }
                        global_store!(ui).set_video_editor_speakers_is_progressing(false);
                    });
                    return;
                }
            }
        };

        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
            return;
        }

        _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }
            global_store!(ui).set_video_editor_speakers_progress(0.5);
        });

        let mut pipeline = pipeline;
        let result = match pipeline.run(&audio_16k) {
            Ok(r) => r,
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                        return;
                    }
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Diarization failed")));
                    global_store!(ui).set_video_editor_speakers_is_progressing(false);
                });
                return;
            }
        };

        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
            return;
        }

        let segments = if merge_gap > 0.0 {
            merge_segments(&result.segments, merge_gap)
        } else {
            result.segments.clone()
        };

        let mut speaker_segments: BTreeMap<String, Vec<&speakrs::segment::Segment>> =
            BTreeMap::new();
        for seg in &segments {
            speaker_segments
                .entry(seg.speaker.clone())
                .or_default()
                .push(seg);
        }

        let original_audio = IMPORTED_AUDIO_DATA.lock().unwrap().clone();
        let Some(original) = original_audio else {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                crate::toast_warn!(ui, tr("Original audio data lost"));
                if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }
                global_store!(ui).set_video_editor_speakers_is_progressing(false);
            });
            return;
        };

        let mut speakers_map: BTreeMap<String, AudioData> = BTreeMap::new();
        let silence_between: f64 = 0.3;
        let original_sr = original.sample_rate as f64;
        let silence_samples = (silence_between * original_sr).round() as usize;
        let channels = original.channels as usize;
        let total_samples = original.samples.len();

        for (speaker, segs) in &speaker_segments {
            let speaker_samples = if timeline_mode {
                // Timeline mode: full-length audio with silence fill
                let mut mask = vec![false; total_samples];
                for seg in segs {
                    let start = (seg.start * original_sr).round() as usize * channels;
                    let end = (seg.end * original_sr).round() as usize * channels;
                    let end = end.min(total_samples);
                    for i in (start..end).step_by(channels) {
                        for c in 0..channels {
                            if i + c < total_samples {
                                mask[i + c] = true;
                            }
                        }
                    }
                }

                let mut result: Vec<f32> = Vec::with_capacity(total_samples);
                for i in 0..total_samples {
                    if mask[i] {
                        result.push(original.samples[i]);
                    } else {
                        result.push(0.0);
                    }
                }
                result
            } else {
                // Compact mode: only speaker's segments with silence gaps
                let mut speaker_samples: Vec<f32> = Vec::new();

                for (i, seg) in segs.iter().enumerate() {
                    if i > 0 && silence_samples > 0 {
                        for _ in 0..silence_samples * channels {
                            speaker_samples.push(0.0);
                        }
                    }

                    let start_sample = (seg.start * original_sr).round() as usize * channels;
                    let end_sample = (seg.end * original_sr).round() as usize * channels;
                    let end_sample = end_sample.min(original.samples.len());

                    if start_sample >= original.samples.len() {
                        continue;
                    }

                    speaker_samples.extend_from_slice(&original.samples[start_sample..end_sample]);
                }
                speaker_samples
            };

            speakers_map.insert(
                speaker.clone(),
                AudioData {
                    samples: speaker_samples,
                    sample_rate: original.sample_rate,
                    channels: original.channels,
                },
            );
        }

        *SPLIT_RESULT_CACHE.lock().unwrap() = Some(speakers_map);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }

            let mut speaker_statuses: Vec<UIAudioPlayerStatus> = Vec::new();
            let sound = source_meta.as_ref().map(|(_, s, _)| *s).unwrap_or(1.0);

            if let Some((name, _, duration_ms)) = &source_meta {
                speaker_statuses.push(UIAudioPlayerStatus {
                    name: name.clone().into(),
                    sound,
                    progress: 0.0,
                    duration_ms: *duration_ms,
                    is_playing: false,
                    is_processing: false,
                });
            }

            let cache = SPLIT_RESULT_CACHE.lock().unwrap();
            if let Some(map) = cache.as_ref() {
                for (name, audio) in map {
                    let dur_ms = audio_duration_ms(audio) as i32;
                    speaker_statuses.push(UIAudioPlayerStatus {
                        name: name.into(),
                        sound,
                        progress: 0.0,
                        duration_ms: dur_ms,
                        is_playing: false,
                        is_processing: false,
                    });
                }
            }

            global_store!(ui).set_video_editor_speakers_is_progressing(false);
            global_store!(ui).set_video_editor_speakers_progress(1.0);
            store_video_editor_speakers_audio_player_status!(ui).set_vec(speaker_statuses);
        });
    });
}

fn video_editor_speakers_export_all_speakers(ui: &AppWindow) {
    let speakers = {
        let guard = SPLIT_RESULT_CACHE.lock().unwrap();
        match guard.clone() {
            Some(s) => s,
            None => {
                crate::toast_warn!(ui, tr("No speakers to export"));
                return;
            }
        }
    };

    let config = global_store!(ui).get_video_editor_speakers_config();
    let export_dir = config.export_dir.to_string();
    if export_dir.is_empty() {
        crate::toast_warn!(ui, tr("Please set export directory"));
        return;
    }

    let Some(filepath) = IMPORTED_AUDIO_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No audio path stored"));
        return;
    };

    let filename = filepath
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("output")
        .to_string();

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let export_path = PathBuf::from(&export_dir);
        if let Err(e) = std::fs::create_dir_all(&export_path) {
            toast::async_toast_warn(ui_weak.clone(), format!("{}: {e}", tr("Failed to create export dir")));
            return;
        }

        for (name, speaker_audio) in &speakers {
            let out_path = export_path.join(format!("{}_{}.wav", filename, name));
            if let Err(e) = write_wav_file(&out_path, speaker_audio) {
                toast::async_toast_warn(ui_weak.clone(), format!("{} {}: {e}", tr("Failed to write"), name));
                continue;
            }

            import_file_to_playlist(ui_weak.clone(), out_path, None).await;
        }

        toast::async_toast_success(
            ui_weak,
            format!("{} {}", tr("Exported all speakers to"), export_path.display()),
        );
    });
}

fn video_editor_speakers_export_speaker(ui: &AppWindow, speaker_name: SharedString) {
    let speakers = {
        let guard = SPLIT_RESULT_CACHE.lock().unwrap();
        match guard.as_ref() {
            Some(s) => s.clone(),
            None => {
                crate::toast_warn!(ui, tr("No speakers to export"));
                return;
            }
        }
    };

    let config = global_store!(ui).get_video_editor_speakers_config();
    let export_dir = config.export_dir.to_string();
    if export_dir.is_empty() {
        crate::toast_warn!(ui, tr("Please set export directory"));
        return;
    }

    let Some(filepath) = IMPORTED_AUDIO_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No audio path stored"));
        return;
    };

    let filename = filepath
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("output")
        .to_string();

    let name = speaker_name.to_string();
    let Some(speaker_audio) = speakers.get(&name).cloned() else {
        crate::toast_warn!(ui, format!("{} '{}' {}", tr("Speaker"), name, tr("not found")));
        return;
    };

    let out_path = PathBuf::from(export_dir).join(format!("{}_{}.wav", filename, name));

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let export_path = PathBuf::from(out_path.parent().unwrap_or_else(|| Path::new(".")));
        if let Err(e) = std::fs::create_dir_all(&export_path) {
            toast::async_toast_warn(ui_weak.clone(), format!("{}: {e}", tr("Failed to create export dir")));
            return;
        }

        if let Err(e) = write_wav_file(&out_path, &speaker_audio) {
            toast::async_toast_warn(ui_weak.clone(), format!("{} {}: {e}", tr("Failed to write"), name));
            return;
        }

        import_file_to_playlist(ui_weak.clone(), out_path.clone(), None).await;
        toast::async_toast_success(ui_weak, format!("{} {}", tr("Exported to"), out_path.display()));
    });
}

fn video_editor_speakers_remove_all(ui: &AppWindow) {
    if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        current.stop();
        current.clear();
    }

    INC_INDEX.fetch_add(1, Ordering::Relaxed);
    *SPLIT_RESULT_CACHE.lock().unwrap() = None;
    *IMPORTED_AUDIO_DATA.lock().unwrap() = None;
    *IMPORTED_AUDIO_PATH.lock().unwrap() = None;
    *IMPORTED_AUDIO_16K.lock().unwrap() = None;
    CURRENT_PLAYING_INDEX.store(-1, Ordering::Relaxed);

    global_store!(ui).set_video_editor_speakers_is_progressing(false);
    store_video_editor_speakers_audio_player_status!(ui).set_vec(vec![]);
}

fn video_editor_speakers_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose export directory"),
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_speakers_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_speakers_config(config.clone());
            save_speakers_config(config.into());
        });
    });
}

fn video_editor_speakers_choose_models_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose models directory"))
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_speakers_config();
            config.models_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_speakers_config(config.clone());
            save_speakers_config(config.into());
        });
    });
}

fn video_editor_speakers_setting_is_valid(ui: &AppWindow) -> bool {
    let config = global_store!(ui).get_video_editor_speakers_config();
    let models_dir = config.models_dir.to_string();
    if models_dir.is_empty() {
        return true;
    }
    PathBuf::from(&models_dir).exists()
}

fn video_editor_speakers_update_config(ui: &AppWindow, config: UISpeakersConfig) {
    global_store!(ui).set_video_editor_speakers_config(config.clone());
    save_speakers_config(config.into());
}

fn video_editor_speakers_play_audio(ui: &AppWindow, index: i32) {
    video_editor_speakers_stop_audio(ui, CURRENT_PLAYING_INDEX.load(Ordering::Relaxed));

    let idx = index as usize;
    let audio = if idx == 0 {
        IMPORTED_AUDIO_DATA.lock().unwrap().clone()
    } else {
        let Some(item) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx) else {
            return;
        };

        SPLIT_RESULT_CACHE
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.get(item.name.as_str()).cloned())
    };

    let Some(audio) = audio else {
        return;
    };

    let (current_progress, duration_ms) = store_video_editor_speakers_audio_player_status!(ui)
        .row_data(idx)
        .map(|s| (s.progress, s.duration_ms))
        .unwrap_or((0.0, 0));

    let progress = if current_progress >= 1.0 {
        0.0
    } else {
        current_progress
    };

    let source = SamplesBuffer::new(
        ChannelCount::new(audio.channels).unwrap(),
        SampleRate::new(audio.sample_rate as u32).unwrap(),
        audio.samples.clone(),
    );

    let sound = store_video_editor_speakers_audio_player_status!(ui)
        .row_data(idx)
        .unwrap()
        .sound
        .clamp(0.0, 1.0);

    let Some(device) = AUDIO_DEVICE.get() else {
        return;
    };

    let mixer = device.mixer();
    let player = Arc::new(Player::connect_new(&mixer));
    player.set_volume(sound);
    player.append(source);

    if progress > 0.0 && duration_ms > 0 {
        let seek_pos = Duration::from_millis((progress * duration_ms as f32) as u64);
        _ = player.try_seek(seek_pos);
    }

    player.play();

    *CURRENT_PLAYER.get().unwrap().lock().unwrap() = Some(Arc::clone(&player));
    CURRENT_PLAYING_INDEX.store(index, Ordering::Relaxed);

    if let Some(mut s) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx) {
        s.is_playing = true;
        s.progress = progress;
        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
    }

    let inc_index = INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;
    start_progress_thread(ui.as_weak(), inc_index, index);
}

fn video_editor_speakers_stop_audio(ui: &AppWindow, index: i32) {
    let current_playing_index = CURRENT_PLAYING_INDEX.load(Ordering::Relaxed);

    INC_INDEX.fetch_add(1, Ordering::Relaxed);
    CURRENT_PLAYING_INDEX.store(-1, Ordering::Relaxed);

    if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        current.stop();
        current.clear();
    }

    let idx = current_playing_index as usize;
    if let Some(mut s) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx) {
        s.is_playing = false;
        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
    }

    let idx = index as usize;
    if current_playing_index != index
        && let Some(mut s) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx)
    {
        s.is_playing = false;
        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
    }
}

fn video_editor_speakers_audio_player_sound_changed(ui: &AppWindow, index: i32, sound: f32) {
    let idx = index as usize;
    if let Some(mut s) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx) {
        s.sound = sound;
        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
    }
}

fn video_editor_speakers_audio_player_sound_released(ui: &AppWindow, index: i32, sound: f32) {
    let idx = index as usize;
    if let Some(player) = CURRENT_PLAYER.get().unwrap().lock().unwrap().as_ref() {
        player.set_volume(sound.clamp(0.0, 1.0));
    }

    if let Some(mut s) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx) {
        s.sound = sound;
        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
    }
}

fn video_editor_speakers_audio_player_progress_released(ui: &AppWindow, index: i32, value: f32) {
    let idx = index as usize;
    if let Some(mut s) = store_video_editor_speakers_audio_player_status!(ui).row_data(idx) {
        s.progress = value;
        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
    }

    video_editor_speakers_play_audio(ui, index);
}

fn start_progress_thread(ui_weak: Weak<AppWindow>, inc_index: u32, playing_index: i32) {
    std::thread::spawn(move || {
        loop {
            if inc_index != INC_INDEX.load(Ordering::Relaxed) {
                break;
            }

            let player_guard = CURRENT_PLAYER.get().unwrap().lock().unwrap();
            let Some(player) = player_guard.as_ref() else {
                drop(player_guard);
                continue;
            };

            let pos_ms = player.get_pos().as_millis() as u64;
            let is_empty = player.empty();
            drop(player_guard);

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if inc_index != INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }

                let idx = playing_index as usize;

                if is_empty {
                    if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
                        current.stop();
                        current.clear();
                    }

                    CURRENT_PLAYING_INDEX.store(-1, Ordering::Relaxed);

                    if let Some(mut s) =
                        store_video_editor_speakers_audio_player_status!(ui).row_data(idx)
                    {
                        s.is_playing = false;
                        s.progress = 1.0;
                        store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
                    }
                    return;
                }

                if let Some(mut s) =
                    store_video_editor_speakers_audio_player_status!(ui).row_data(idx)
                    && s.is_playing
                {
                    s.progress = (pos_ms as f64 / s.duration_ms as f64).min(1.0) as f32;
                    store_video_editor_speakers_audio_player_status!(ui).set_row_data(idx, s);
                }
            });

            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

fn audio_duration_ms(audio: &AudioData) -> u64 {
    let channels = audio.channels.max(1) as usize;
    let frame_count = audio.samples.len() / channels;
    frame_count as u64 * 1000 / audio.sample_rate as u64
}

fn write_wav_file(path: &PathBuf, audio: &AudioData) -> Result<()> {
    let spec = hound::WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    let max_val = 32767.0f32;

    for sample in &audio.samples {
        let s = (sample * max_val).clamp(-max_val, max_val);
        writer.write_sample(s as i16)?;
    }

    writer.finalize()?;
    Ok(())
}

fn video_editor_speakers_model_start_download(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let index = index as usize;

    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose model save directory"))
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            crate::toast_info!(
                ui,
                tr("Download models may take some time, please wait patiently")
            );

            if let Some(mut item) =
                store_video_editor_speakers_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Downloading;
                item.progress = 0.0;
                store_video_editor_speakers_models_downloader!(ui).set_row_data(index, item);
            }
        });

        tokio::task::spawn_blocking(move || {
            let result = ModelManager::with_cache_dir(dirpath)
                .and_then(|manager| manager.ensure(ExecutionMode::Cpu));

            match result {
                Ok(models_dir) => {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_speakers_models_downloader!(ui).row_data(index)
                        {
                            item.state = UIDownloaderState::Finished;
                            item.progress = 1.0;
                            store_video_editor_speakers_models_downloader!(ui)
                                .set_row_data(index, item);
                        }

                        let mut config = global_store!(ui).get_video_editor_speakers_config();
                        config.models_dir = models_dir.to_string_lossy().to_string().into();
                        global_store!(ui).set_video_editor_speakers_config(config.clone());
                        save_speakers_config(config.into());
                        crate::toast_success!(ui, tr("Models downloaded successfully"));
                    });
                }
                Err(e) => {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_speakers_models_downloader!(ui).row_data(index)
                        {
                            item.state = UIDownloaderState::Failed;
                            store_video_editor_speakers_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                        crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to download models")));
                    });
                }
            }
        });
    });
}
