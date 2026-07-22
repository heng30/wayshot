use crate::{
    db::{DeepFilterConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory,
        share_screen::picker_file,
        toast::{self, async_toast_info, async_toast_warn},
        tr::tr,
        video_editor::project::DEEP_FILTER_CONFIG_ID,
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, AudioPlayerStatus as UIAudioPlayerStatus, Downloader as UIDownloader,
        DownloaderState as UIDownloaderState,
        VideoEditorDeepFilterConfig as UIDeepFilterConfig,
    },
};
use anyhow::{Result, bail};
use audio_utils::loader::load_audio_file_and_convert;
use deepfilter_rs::{DeepFilterStream, Model, SAMPLE_RATE};
use rodio::{ChannelCount, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer};
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel, Weak};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
    },
    time::Duration,
};

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "wav", "mp3", "flac", "ogg", "m4a", "aac", "wma",
];

static IMPORTED_AUDIO_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
static IMPORTED_AUDIO_DATA: Mutex<Option<Vec<f32>>> = Mutex::new(None);
static DENOISED_AUDIO_DATA: Mutex<Option<Vec<f32>>> = Mutex::new(None);
static AUDIO_DEVICE: OnceLock<Arc<MixerDeviceSink>> = OnceLock::new();
static CURRENT_PLAYER: OnceLock<Mutex<Option<Arc<Player>>>> = OnceLock::new();
static PROCESS_CANCEL: AtomicBool = AtomicBool::new(false);
static CURRENT_PLAYING_INDEX: AtomicI32 = AtomicI32::new(-1);
static INC_INDEX: AtomicU32 = AtomicU32::new(0);
static PROCESS_INC_INDEX: AtomicU32 = AtomicU32::new(0);

#[macro_export]
macro_rules! store_video_editor_deep_filter_audio_player_status {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_deep_filter_audio_player_status()
            .as_any()
            .downcast_ref::<VecModel<UIAudioPlayerStatus>>()
            .expect("We know we set a VecModel<UIAudioPlayerStatus> earlier for video editor deep filter")
    };
}

#[macro_export]
macro_rules! store_video_editor_deep_filter_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_deep_filter_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for video editor deep filter models")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_deep_filter_import_audio, ui);
    logic_cb!(video_editor_deep_filter_process_audio, ui);
    logic_cb!(video_editor_deep_filter_cancel_process_audio, ui);
    logic_cb!(video_editor_deep_filter_export_audio, ui);
    logic_cb!(video_editor_deep_filter_remove_all, ui);
    logic_cb!(video_editor_deep_filter_choose_export_dir, ui);
    logic_cb!(video_editor_deep_filter_choose_model_dir, ui, index);
    logic_cb!(video_editor_deep_filter_update_config, ui, config);
    logic_cb!(
        video_editor_deep_filter_model_start_download,
        ui,
        index,
        url
    );
    logic_cb!(
        video_editor_deep_filter_model_cancel_download,
        ui,
        index,
        url
    );
    logic_cb_pure!(video_editor_deep_filter_setting_is_valid, ui);
    logic_cb!(video_editor_deep_filter_play_audio, ui, index);
    logic_cb!(video_editor_deep_filter_stop_audio, ui, index);
    logic_cb!(
        video_editor_deep_filter_audio_player_sound_changed,
        ui,
        index,
        sound
    );
    logic_cb!(
        video_editor_deep_filter_audio_player_sound_released,
        ui,
        index,
        sound
    );
    logic_cb!(
        video_editor_deep_filter_audio_player_progress_released,
        ui,
        index,
        value
    );
}

fn inner_init(ui: &AppWindow) {
    let downloaders: Vec<UIDownloader> = Model::all_models()
        .iter()
        .map(|m| UIDownloader {
            url: m.download_url().into(),
            filename: m.folder_name().into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect();
    store_video_editor_deep_filter_models_downloader!(ui).set_vec(downloaders);

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
        let config = load_deep_filter_config()
            .await
            .unwrap_or_else(|| DeepFilterConfigData {
                id: DEEP_FILTER_CONFIG_ID.to_string(),
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_deep_filter_config(config.into());
        });
    });
}

fn save_deep_filter_config(config: DeepFilterConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize deep filter config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, DEEP_FILTER_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, DEEP_FILTER_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save deep filter config: {:?}", e);
            }
        }
    });
}

async fn load_deep_filter_config() -> Option<DeepFilterConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, DEEP_FILTER_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_deep_filter_import_audio(ui: &AppWindow) {
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

        // DeepFilter requires 48kHz mono
        let audio_config = match load_audio_file_and_convert(&filepath, 1, SAMPLE_RATE as u32) {
            Ok(config) => config,
            Err(e) => {
                async_toast_warn(ui_weak, format!("{}: {e}", tr("Failed to read audio file")));
                return;
            }
        };

        let duration_ms = audio_config.duration.as_millis() as i32;

        *IMPORTED_AUDIO_PATH.lock().unwrap() = Some(filepath.clone());
        *IMPORTED_AUDIO_DATA.lock().unwrap() = Some(audio_config.samples.clone());
        // Clear any previous denoised result
        *DENOISED_AUDIO_DATA.lock().unwrap() = None;

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
            store_video_editor_deep_filter_audio_player_status!(ui).set_vec(vec![status]);
        });
    });
}

fn video_editor_deep_filter_process_audio(ui: &AppWindow) {
    let model_dir = match get_and_check_model_setting(ui) {
        Ok(p) => p,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_deep_filter_setting_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let audio = {
        let guard = IMPORTED_AUDIO_DATA.lock().unwrap();
        match guard.clone() {
            Some(a) => a,
            None => {
                crate::toast_warn!(ui, tr("No audio imported"));
                return;
            }
        }
    };

    let ui_weak = ui.as_weak();
    global_store!(ui).set_video_editor_deep_filter_is_progressing(true);
    global_store!(ui).set_video_editor_deep_filter_progress(0.0);

    let source_meta = store_video_editor_deep_filter_audio_player_status!(ui)
        .row_data(0)
        .map(|s| (s.name.to_string(), s.sound, s.duration_ms));

    let inc_index = PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;
    PROCESS_CANCEL.store(false, Ordering::Relaxed);

    tokio::task::spawn_blocking(move || {
        let mut stream = match DeepFilterStream::with_threads(&model_dir, 4) {
            Ok(s) => s,
            Err(e) => {
                async_toast_warn(ui_weak.clone(), format!("{}: {e}", tr("Failed to load model")));
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_deep_filter_is_progressing(false);
                });
                return;
            }
        };

        if let Err(e) = stream.warmup() {
            log::warn!("DeepFilter warmup failed: {e}");
        }

        let delay_samples = stream.delay_samples();
        let total_samples = audio.len();
        let chunk_size = SAMPLE_RATE; // Process 1 second at a time for progress reporting

        let mut output = Vec::with_capacity(total_samples);
        let mut processed = 0usize;

        for chunk in audio.chunks(chunk_size) {
            if PROCESS_CANCEL.load(Ordering::Relaxed)
                || inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed)
            {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if inc_index == PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                        global_store!(ui).set_video_editor_deep_filter_is_progressing(false);
                    }
                });
                return;
            }

            match stream.process(chunk) {
                Ok(mut enhanced) => output.append(&mut enhanced),
                Err(e) => {
                    async_toast_warn(
                        ui_weak.clone(),
                        format!("{}: {e}", tr("Failed to denoise audio")),
                    );
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if inc_index == PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                            global_store!(ui).set_video_editor_deep_filter_is_progressing(false);
                        }
                    });
                    return;
                }
            }

            processed += chunk.len();
            let progress = processed as f32 / total_samples as f32;

            let ui_weak_progress = ui_weak.clone();
            _ = ui_weak_progress.upgrade_in_event_loop(move |ui| {
                if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }
                global_store!(ui).set_video_editor_deep_filter_progress(progress);
            });
        }

        // Flush remaining samples
        if let Ok(mut remaining) = stream.flush() {
            output.append(&mut remaining);
        }

        // Trim algorithmic delay from start of output (to match Python's pad=True behavior)
        let trimmed = if output.len() > delay_samples {
            output[delay_samples..].to_vec()
        } else {
            output
        };

        // Adjust output length to match input (in case of slight differences)
        let trimmed = if trimmed.len() > total_samples {
            trimmed[..total_samples].to_vec()
        } else {
            trimmed
        };

        *DENOISED_AUDIO_DATA.lock().unwrap() = Some(trimmed.clone());

        let denoised_duration_ms = (trimmed.len() as f64 / SAMPLE_RATE as f64 * 1000.0) as i32;

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }

            let mut statuses: Vec<UIAudioPlayerStatus> = Vec::new();
            let sound = source_meta.as_ref().map(|(_, s, _)| *s).unwrap_or(1.0);

            if let Some((name, _, duration_ms)) = &source_meta {
                statuses.push(UIAudioPlayerStatus {
                    name: name.clone().into(),
                    sound,
                    progress: 0.0,
                    duration_ms: *duration_ms,
                    is_playing: false,
                    is_processing: false,
                });
            }

            statuses.push(UIAudioPlayerStatus {
                name: tr("Denoised").into(),
                sound,
                progress: 0.0,
                duration_ms: denoised_duration_ms,
                is_playing: false,
                is_processing: false,
            });

            global_store!(ui).set_video_editor_deep_filter_is_progressing(false);
            store_video_editor_deep_filter_audio_player_status!(ui).set_vec(statuses);
        });
    });
}

fn video_editor_deep_filter_cancel_process_audio(ui: &AppWindow) {
    PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed);
    PROCESS_CANCEL.store(true, Ordering::Relaxed);
    global_store!(ui).set_video_editor_deep_filter_is_progressing(false);
}

fn video_editor_deep_filter_export_audio(ui: &AppWindow) {
    let denoised = {
        let guard = DENOISED_AUDIO_DATA.lock().unwrap();
        match guard.clone() {
            Some(d) => d,
            None => {
                crate::toast_warn!(ui, tr("No denoised audio"));
                return;
            }
        }
    };

    let config = global_store!(ui).get_video_editor_deep_filter_config();
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

    let out_path = PathBuf::from(&export_dir).join(format!("{}_denoised.wav", filename));

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let export_path = PathBuf::from(out_path.parent().unwrap_or_else(|| Path::new(".")));
        if let Err(e) = std::fs::create_dir_all(&export_path) {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {e}", tr("Failed to create export dir")),
            );
            return;
        }

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = match hound::WavWriter::create(&out_path, spec) {
            Ok(w) => w,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("Failed to write denoised audio")),
                );
                return;
            }
        };

        let max_val = 32767.0f32;
        for sample in &denoised {
            let s = (sample * max_val).clamp(-max_val, max_val);
            if writer.write_sample(s as i16).is_err() {
                break;
            }
        }

        _ = writer.finalize();

        crate::logic::video_editor::playlist::import_file_to_playlist(ui_weak.clone(), out_path.clone(), None).await;
        toast::async_toast_success(
            ui_weak,
            format!("{} {}", tr("Exported to"), out_path.display()),
        );
    });
}

fn video_editor_deep_filter_remove_all(ui: &AppWindow) {
    if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        current.stop();
        current.clear();
    }

    INC_INDEX.fetch_add(1, Ordering::Relaxed);
    *DENOISED_AUDIO_DATA.lock().unwrap() = None;
    *IMPORTED_AUDIO_DATA.lock().unwrap() = None;
    *IMPORTED_AUDIO_PATH.lock().unwrap() = None;
    PROCESS_CANCEL.store(false, Ordering::Relaxed);
    CURRENT_PLAYING_INDEX.store(-1, Ordering::Relaxed);

    global_store!(ui).set_video_editor_deep_filter_is_progressing(false);
    store_video_editor_deep_filter_audio_player_status!(ui).set_vec(vec![]);
}

fn video_editor_deep_filter_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose export directory"),
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_deep_filter_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_deep_filter_config(config.clone());
            save_deep_filter_config(config.into());
        });
    });
}

fn video_editor_deep_filter_choose_model_dir(ui: &AppWindow, _index: i32) {
    let ui_weak = ui.as_weak();
    let config = global_store!(ui).get_video_editor_deep_filter_config();
    let index = config.selected_model_index as usize;
    let title = tr("Choose DeepFilter model directory");

    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &title) else {
            return;
        };

        // Validate that the directory contains a config.ini (basic check for valid model)
        if !dirpath.join("config.ini").exists() {
            toast::async_toast_warn(
                ui_weak,
                tr("Invalid model directory: config.ini not found").to_string(),
            );
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_deep_filter_config();
            let dir: SharedString = dirpath.to_string_lossy().to_string().into();
            set_model_dir_by_index(&mut config, index, dir);
            global_store!(ui).set_video_editor_deep_filter_config(config.clone());
            save_deep_filter_config(config.into());

            if let Some(mut item) =
                store_video_editor_deep_filter_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::UnStart;
                store_video_editor_deep_filter_models_downloader!(ui).set_row_data(index, item);
            }
        });
    });
}

fn video_editor_deep_filter_model_start_download(ui: &AppWindow, index: i32, _url: SharedString) {
    let index = index as usize;

    // DeepFilter models are folder-based (multiple .onnx files + config.ini).
    // Automatic download is not supported; users download manually from GitHub
    // and select the local directory via "Choose Model" button.
    // Mark as "finished" since the browser button already handles opening the URL.
    if let Some(mut item) = store_video_editor_deep_filter_models_downloader!(ui).row_data(index) {
        item.state = UIDownloaderState::Finished;
        store_video_editor_deep_filter_models_downloader!(ui).set_row_data(index, item);
    }
}

fn video_editor_deep_filter_model_cancel_download(ui: &AppWindow, index: i32, _url: SharedString) {
    let index = index as usize;

    if let Some(mut item) = store_video_editor_deep_filter_models_downloader!(ui).row_data(index) {
        item.state = UIDownloaderState::Cancelled;
        store_video_editor_deep_filter_models_downloader!(ui).set_row_data(index, item);
    }
}

fn video_editor_deep_filter_setting_is_valid(ui: &AppWindow) -> bool {
    get_and_check_model_setting(ui).is_ok()
}

fn video_editor_deep_filter_update_config(ui: &AppWindow, config: UIDeepFilterConfig) {
    global_store!(ui).set_video_editor_deep_filter_config(config.clone());
    save_deep_filter_config(config.into());
}

fn video_editor_deep_filter_play_audio(ui: &AppWindow, index: i32) {
    video_editor_deep_filter_stop_audio(ui, CURRENT_PLAYING_INDEX.load(Ordering::Relaxed));

    let idx = index as usize;
    let audio = if idx == 0 {
        IMPORTED_AUDIO_DATA.lock().unwrap().clone()
    } else {
        DENOISED_AUDIO_DATA.lock().unwrap().clone()
    };

    let Some(audio) = audio else {
        return;
    };

    let (current_progress, duration_ms) = store_video_editor_deep_filter_audio_player_status!(ui)
        .row_data(idx)
        .map(|s| (s.progress, s.duration_ms))
        .unwrap_or((0.0, 0));

    let progress = if current_progress >= 1.0 {
        0.0
    } else {
        current_progress
    };

    // DeepFilter audio is always 48kHz mono
    let source = SamplesBuffer::new(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(SAMPLE_RATE as u32).expect("valid sample rate"),
        audio,
    );

    let sound = store_video_editor_deep_filter_audio_player_status!(ui)
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

    if let Some(mut s) = store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx) {
        s.is_playing = true;
        s.progress = progress;
        store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
    }

    let inc_index = INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;
    start_progress_thread(ui.as_weak(), inc_index, index);
}

fn video_editor_deep_filter_stop_audio(ui: &AppWindow, index: i32) {
    let current_playing_index = CURRENT_PLAYING_INDEX.load(Ordering::Relaxed);

    INC_INDEX.fetch_add(1, Ordering::Relaxed);
    CURRENT_PLAYING_INDEX.store(-1, Ordering::Relaxed);

    if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        current.stop();
        current.clear();
    }

    let idx = current_playing_index as usize;
    if let Some(mut s) = store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx) {
        s.is_playing = false;
        store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
    }

    let idx = index as usize;
    if current_playing_index != index
        && let Some(mut s) = store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx)
    {
        s.is_playing = false;
        store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
    }
}

fn video_editor_deep_filter_audio_player_sound_changed(ui: &AppWindow, index: i32, sound: f32) {
    let idx = index as usize;
    if let Some(mut s) = store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx) {
        s.sound = sound;
        store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
    }
}

fn video_editor_deep_filter_audio_player_sound_released(ui: &AppWindow, index: i32, sound: f32) {
    let idx = index as usize;
    if let Some(player) = CURRENT_PLAYER.get().unwrap().lock().unwrap().as_ref() {
        player.set_volume(sound.clamp(0.0, 1.0));
    }

    if let Some(mut s) = store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx) {
        s.sound = sound;
        store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
    }
}

fn video_editor_deep_filter_audio_player_progress_released(
    ui: &AppWindow,
    index: i32,
    value: f32,
) {
    let idx = index as usize;
    if let Some(mut s) = store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx) {
        s.progress = value;
        store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
    }

    video_editor_deep_filter_play_audio(ui, index);
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
                        store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx)
                    {
                        s.is_playing = false;
                        s.progress = 1.0;
                        store_video_editor_deep_filter_audio_player_status!(ui)
                            .set_row_data(idx, s);
                    }
                    return;
                }

                if let Some(mut s) =
                    store_video_editor_deep_filter_audio_player_status!(ui).row_data(idx)
                    && s.is_playing
                {
                    s.progress = (pos_ms as f64 / s.duration_ms as f64).min(1.0) as f32;
                    store_video_editor_deep_filter_audio_player_status!(ui).set_row_data(idx, s);
                }
            });

            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<PathBuf> {
    let config = global_store!(ui).get_video_editor_deep_filter_config();
    let model_dir = get_model_dir(&config);

    if model_dir.is_empty() {
        bail!(tr("Please set model directory").to_string());
    }

    let path = PathBuf::from(&model_dir);
    if !path.exists() {
        bail!(tr("Model directory not found").to_string());
    }

    if !path.join("config.ini").exists() {
        bail!(tr("Invalid model directory: config.ini not found").to_string());
    }

    Ok(path)
}

fn get_model_dir(config: &UIDeepFilterConfig) -> String {
    match config.selected_model_index {
        0 => config.dfn2_model_dir.to_string(),
        1 => config.dfn2_ll_model_dir.to_string(),
        2 => config.dfn2_h0_model_dir.to_string(),
        3 => config.dfn3_model_dir.to_string(),
        4 => config.dfn3_ll_model_dir.to_string(),
        5 => config.dfn3_h0_model_dir.to_string(),
        _ => config.dfn3_model_dir.to_string(),
    }
}

fn set_model_dir_by_index(config: &mut UIDeepFilterConfig, index: usize, dir: SharedString) {
    match index {
        0 => config.dfn2_model_dir = dir,
        1 => config.dfn2_ll_model_dir = dir,
        2 => config.dfn2_h0_model_dir = dir,
        3 => config.dfn3_model_dir = dir,
        4 => config.dfn3_ll_model_dir = dir,
        5 => config.dfn3_h0_model_dir = dir,
        _ => config.dfn3_model_dir = dir,
    }
}
