use crate::{
    config::{self, AiModel},
    db::{MusicGenConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory,
        toast,
        tr::tr,
        video_editor::{
            playlist::import_file_to_playlist,
            project::{MUSICGEN_CONFIG_ID, PROJECT_STATE},
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        VideoEditorMusicGenConfig as UIMusicGenConfig,
    },
};
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use musicgen_rs::{DecoderMode, Model, MusicGen};
use once_cell::sync::Lazy;
use rodio::{
    ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer,
};
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel, Weak};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};
use video_editor::{filters::traits::SubtitleEntry, tracks::Track};

const MUSICGEN_CHANNEL: u16 = 1;
const MUSICGEN_SAMPLE_RATE: u32 = 32000;

static PROCESS_CANCEL: AtomicBool = AtomicBool::new(false);
static PROCESS_ID: AtomicU32 = AtomicU32::new(0);
static GENERATED_SAMPLES: Lazy<Mutex<Option<Vec<f32>>>> = Lazy::new(|| Mutex::new(None));
static AUDIO_DEVICE: OnceLock<Arc<MixerDeviceSink>> = OnceLock::new();
static CURRENT_PLAYER: OnceLock<Mutex<Option<Arc<Player>>>> = OnceLock::new();
static PLAYER_INC_INDEX: AtomicU32 = AtomicU32::new(0);

#[macro_export]
macro_rules! store_video_editor_musicgen_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_musicgen_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect(
                "We know we set a VecModel<UIDownloader> earlier for video editor musicgen models",
            )
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_musicgen_generate, ui);
    logic_cb!(video_editor_musicgen_cancel, ui);
    logic_cb!(video_editor_musicgen_choose_model_dir, ui);
    logic_cb!(video_editor_musicgen_choose_export_dir, ui);
    logic_cb!(video_editor_musicgen_update_config, ui, config);
    logic_cb!(video_editor_musicgen_model_start_download, ui, index, url);
    logic_cb!(video_editor_musicgen_model_cancel_download, ui, index, url);
    logic_cb_pure!(video_editor_musicgen_setting_is_valid, ui);
    logic_cb!(video_editor_musicgen_audio_player_sound_changed, ui, sound);
    logic_cb!(
        video_editor_musicgen_audio_player_progress_changed,
        ui,
        value
    );
    logic_cb!(
        video_editor_musicgen_audio_player_progress_released,
        ui,
        value
    );
    logic_cb!(
        video_editor_musicgen_audio_player_progress_pressed,
        ui,
        value
    );
    logic_cb!(video_editor_musicgen_play_audio, ui);
    logic_cb!(video_editor_musicgen_stop_audio, ui);
    logic_cb!(video_editor_musicgen_export_audio, ui);
    logic_cb!(video_editor_musicgen_generate_prompt, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = Model::all_models()
        .into_iter()
        .map(|m| UIDownloader {
            url: m.browse_url().into(),
            filename: format!("{} model dir", m).into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect::<Vec<_>>();
    store_video_editor_musicgen_models_downloader!(ui).set_vec(downloaders);

    match DeviceSinkBuilder::open_default_sink() {
        Ok(device_sink) => {
            let sound = global_store!(ui).get_video_editor_musicgen_audio_sound();
            let player = Player::connect_new(&device_sink.mixer());
            player.set_volume(sound.clamp(0.0, 1.0));

            AUDIO_DEVICE
                .set(Arc::new(device_sink))
                .ok()
                .expect("AUDIO_DEVICE already initialized");
            CURRENT_PLAYER
                .set(Mutex::new(Some(Arc::new(player))))
                .ok()
                .expect("CURRENT_PLAYER already initialized");
        }
        Err(e) => log::warn!("Failed to create audio output stream: {e}"),
    }

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_musicgen_config()
            .await
            .unwrap_or_else(|| MusicGenConfigData {
                id: MUSICGEN_CONFIG_ID.to_string(),
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_musicgen_config(config.into());
        });
    });
}

fn save_musicgen_config(config: MusicGenConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize musicgen config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, MUSICGEN_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, MUSICGEN_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save musicgen config: {:?}", e);
            }
        }
    });
}

async fn load_musicgen_config() -> Option<MusicGenConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, MUSICGEN_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_musicgen_generate(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_musicgen_config();
    let prompt = config.prompt.to_string();
    if prompt.is_empty() {
        crate::toast_warn!(ui, tr("No prompt entered"));
        return;
    }

    save_musicgen_config(config.clone().into());

    let model_dir = get_model_dir(&config);
    if model_dir.is_empty() {
        global_store!(ui).set_video_editor_is_show_musicgen_setting_dialog(true);
        crate::toast_warn!(ui, tr("Please select a model directory"));
        return;
    }

    let model_dir_path = PathBuf::from(&model_dir);
    if !model_dir_path.exists() {
        global_store!(ui).set_video_editor_is_show_musicgen_setting_dialog(true);
        crate::toast_warn!(ui, tr("Model directory not found"));
        return;
    }

    let model = match config.selected_model_index {
        0 => Model::Small,
        1 => Model::SmallFp16,
        2 => Model::SmallQuant,
        3 => Model::Medium,
        4 => Model::MediumFp16,
        5 => Model::MediumQuant,
        6 => Model::Large,
        _ => Model::Small,
    };

    let duration = config.duration.clamp(1, 30) as usize;

    // Cancel any previous generation
    PROCESS_CANCEL.store(true, Ordering::Relaxed);
    let process_id = PROCESS_ID.fetch_add(1, Ordering::Relaxed) + 1;
    PROCESS_CANCEL.store(false, Ordering::Relaxed);

    global_store!(ui).set_video_editor_musicgen_is_generating(true);
    global_store!(ui).set_video_editor_musicgen_progress(0.0);

    // Check if prompt needs translation (contains non-ASCII characters)
    let needs_translation = !prompt.is_ascii();
    let ui_weak = ui.as_weak();
    let ai_setting = config::all().ai_model.clone();

    tokio::spawn(async move {
        let english_prompt = if needs_translation {
            translate_prompt(ui_weak.clone(), &prompt, &ai_setting).await
        } else {
            Some(prompt.clone())
        };

        let english_prompt = match english_prompt {
            Some(p) => p,
            None => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }
                    global_store!(ui).set_video_editor_musicgen_is_generating(false);
                });
                return;
            }
        };

        if PROCESS_CANCEL.load(Ordering::Relaxed) {
            return;
        }

        if PROCESS_ID.load(Ordering::Relaxed) != process_id {
            return;
        }

        let mut musicgen = match MusicGen::load(&model_dir_path, model, DecoderMode::Merged) {
            Ok(m) => m,
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_musicgen_is_generating(false);
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to load model")));
                });
                return;
            }
        };

        if PROCESS_CANCEL.load(Ordering::Relaxed) {
            return;
        }

        let ui_weak_progress = ui_weak.clone();
        let cancel_sig = &PROCESS_CANCEL;
        let result = musicgen.generate(
            &english_prompt,
            duration,
            Box::new(move |current, total| {
                if cancel_sig.load(Ordering::Relaxed)
                    || PROCESS_ID.load(Ordering::Relaxed) != process_id
                {
                    return true;
                }

                let progress = current / total;
                _ = ui_weak_progress.upgrade_in_event_loop(move |ui| {
                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }
                    global_store!(ui).set_video_editor_musicgen_progress(progress);
                });

                false
            }),
        );

        match result {
            Ok(output) => {
                let duration_ms = (output.samples.len() as u64 * 1000
                    / MUSICGEN_SAMPLE_RATE as u64
                    / MUSICGEN_CHANNEL as u64) as i32;

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }
                    *GENERATED_SAMPLES.lock().unwrap() = Some(output.samples);
                    global_store!(ui).set_video_editor_musicgen_is_generating(false);
                    global_store!(ui).set_video_editor_musicgen_progress(1.0);
                    global_store!(ui).set_video_editor_musicgen_audio_duration_ms(duration_ms);
                    global_store!(ui).set_video_editor_musicgen_audio_player_progress(0.0);
                    global_store!(ui).set_video_editor_musicgen_audio_player_is_playing(false);
                    crate::toast_success!(ui, tr("Audio generated successfully"));
                });
            }
            Err(musicgen_rs::Error::Aborted) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }
                    global_store!(ui).set_video_editor_musicgen_is_generating(false);
                });
            }
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }
                    global_store!(ui).set_video_editor_musicgen_is_generating(false);
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to generate audio")));
                });
            }
        }
    });
}

async fn translate_prompt(
    ui_weak: Weak<AppWindow>,
    prompt: &str,
    ai_setting: &AiModel,
) -> Option<String> {
    if ai_setting.api_base_url.is_empty()
        || ai_setting.model_name.is_empty()
        || ai_setting.api_key.is_empty()
    {
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            crate::toast_info!(ui, tr("Please setup AI model for translation"));
        });
        return None;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);

    let system_prompt = "You are a translation assistant. Translate the following text to English. Only output the translated text, no additional explanation or formatting.";

    let request_config = APIConfig {
        api_base_url: ai_setting.api_base_url.clone(),
        api_model: ai_setting.model_name.clone(),
        api_key: ai_setting.api_key.clone(),
        temperature: Some(0.3),
    };

    let ui_weak_chat = ui_weak.clone();
    let prompt_owned = prompt.to_string();
    tokio::spawn(async move {
        let chat_config = ChatConfig { tx };
        let chat = Chat::new(
            system_prompt,
            &prompt_owned,
            chat_config,
            request_config,
            vec![],
        );
        if let Err(e) = chat.start().await {
            toast::async_toast_warn(
                ui_weak_chat,
                format!("{}: {e}", tr("Failed to translate prompt")),
            );
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if let Some(ref text) = item.text {
            resp.push_str(text);
        }
    }

    let translated = resp.trim().to_string();
    if translated.is_empty() {
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            crate::toast_warn!(ui, tr("Failed to translate prompt"));
        });
        return None;
    }

    Some(translated)
}

fn video_editor_musicgen_generate_prompt(ui: &AppWindow) {
    let ai_setting = config::all().ai_model.clone();
    if ai_setting.api_base_url.is_empty()
        || ai_setting.model_name.is_empty()
        || ai_setting.api_key.is_empty()
    {
        crate::toast_info!(ui, tr("Please setup AI model and try again"));
        return;
    }

    let subtitle_entries: Vec<SubtitleEntry> = {
        let state = PROJECT_STATE.lock().unwrap();
        let Some(ref s) = *state else {
            crate::toast_warn!(ui, tr("No project opened"));
            return;
        };

        let mut found_entries: Vec<SubtitleEntry> = vec![];
        for track in &s.tracks_manager.tracks {
            if let Track::Subtitle(st) = track {
                found_entries = st.get_subtitle_entries();
                break;
            }
        }
        found_entries
    };

    if subtitle_entries.is_empty() {
        crate::toast_warn!(ui, tr("No subtitle track found or subtitle track is empty"));
        return;
    }

    let language = global_store!(ui)
        .get_setting_preference()
        .language
        .to_string();
    let language_name = match language.as_str() {
        "cn" => "Chinese",
        "en" => "English",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        _ => "English",
    };

    let subtitle_text = subtitle_entries
        .iter()
        .map(|e| e.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    crate::toast_info!(ui, tr("Please wait while AI generates the prompt"));

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let system_prompt = format!(
            "You are a music prompt generator. Given the following video subtitles, generate a concise and descriptive music prompt that matches the mood, theme, and atmosphere of the content. The prompt should be suitable for AI music generation (like MusicGen). The prompt should be in {language_name}. Only output the music prompt, no additional explanation or formatting.",
        );

        let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);

        let request_config = APIConfig {
            api_base_url: ai_setting.api_base_url.clone(),
            api_model: ai_setting.model_name.clone(),
            api_key: ai_setting.api_key.clone(),
            temperature: Some(0.7),
        };

        let ui_weak_chat = ui_weak.clone();
        tokio::spawn(async move {
            let chat_config = ChatConfig { tx };
            let chat = Chat::new(
                &system_prompt,
                &subtitle_text,
                chat_config,
                request_config,
                vec![],
            );
            if let Err(e) = chat.start().await {
                toast::async_toast_warn(
                    ui_weak_chat,
                    format!("{}: {e}", tr("Failed to generate prompt")),
                );
            }
        });

        let mut resp = String::new();
        while let Some(item) = rx.recv().await {
            if let Some(ref text) = item.text {
                resp.push_str(text);
            }
        }

        let generated_prompt = resp.trim().to_string();
        if generated_prompt.is_empty() {
            toast::async_toast_warn(ui_weak, tr("Failed to generate prompt"));
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_musicgen_config();
            config.prompt = generated_prompt.into();
            global_store!(ui).set_video_editor_musicgen_config(config.clone());
            save_musicgen_config(config.into());
        });
    });
}

fn video_editor_musicgen_cancel(ui: &AppWindow) {
    PROCESS_CANCEL.store(true, Ordering::Relaxed);
    PROCESS_ID.fetch_add(1, Ordering::Relaxed);
    *GENERATED_SAMPLES.lock().unwrap() = None;
    global_store!(ui).set_video_editor_musicgen_is_generating(false);
    global_store!(ui).set_video_editor_musicgen_progress(0.0);
    global_store!(ui).set_video_editor_musicgen_audio_duration_ms(0);
    global_store!(ui).set_video_editor_musicgen_audio_player_is_playing(false);
    global_store!(ui).set_video_editor_musicgen_audio_player_progress(0.0);
}

fn video_editor_musicgen_setting_is_valid(ui: &AppWindow) -> bool {
    let config = global_store!(ui).get_video_editor_musicgen_config();
    let model_dir = get_model_dir(&config);

    !config.export_dir.is_empty()
        && PathBuf::from(config.export_dir.as_str()).exists()
        && !model_dir.is_empty()
        && PathBuf::from(&model_dir).exists()
}

fn video_editor_musicgen_choose_model_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let config = global_store!(ui).get_video_editor_musicgen_config();
    let index = config.selected_model_index as usize;

    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose model directory")) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_musicgen_config();
            let dir: SharedString = dirpath.to_string_lossy().to_string().into();
            set_model_dir_by_index(&mut config, index, dir);
            global_store!(ui).set_video_editor_musicgen_config(config.clone());
            save_musicgen_config(config.into());
        });
    });
}

fn video_editor_musicgen_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose export directory"))
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_musicgen_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_musicgen_config(config.clone());
            save_musicgen_config(config.into());
        });
    });
}

fn video_editor_musicgen_update_config(ui: &AppWindow, config: UIMusicGenConfig) {
    global_store!(ui).set_video_editor_musicgen_config(config.clone());
    save_musicgen_config(config.into());
}

fn video_editor_musicgen_model_start_download(ui: &AppWindow, index: i32, _url: SharedString) {
    let ui_weak = ui.as_weak();
    let index = index as usize;
    let model = Model::all_models()[index];
    let specs = model.file_spec();

    tokio::spawn(async move {
        let Some(base_dir) = picker_directory(ui_weak.clone(), &tr("Choose model save directory"))
        else {
            return;
        };

        let save_dir = base_dir.join(model.dir_name());
        if let Err(e) = std::fs::create_dir_all(&save_dir) {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Failed to create directory")),
            );
            return;
        }

        let total_files = specs.len();
        for (file_index, (remote, local)) in specs.iter().enumerate() {
            let url = format!(
                "{}/{}",
                "https://huggingface.co/gabotechs/music_gen/resolve/main", remote
            );
            let _filename: SharedString = (*local).into();
            let save_path = save_dir.join(local);

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if let Some(mut item) =
                    store_video_editor_musicgen_models_downloader!(ui).row_data(index)
                {
                    item.state = UIDownloaderState::Downloading;
                    item.progress = file_index as f32 / total_files as f32;
                    store_video_editor_musicgen_models_downloader!(ui).set_row_data(index, item);
                }
            });

            let result = download_single_file(&url, &save_path).await;

            match result {
                Ok(true) => {
                    let progress = (file_index + 1) as f32 / total_files as f32;
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_musicgen_models_downloader!(ui).row_data(index)
                        {
                            item.progress = progress;
                            store_video_editor_musicgen_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                    });
                }
                Ok(false) => {
                    // Cancelled
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_musicgen_models_downloader!(ui).row_data(index)
                        {
                            item.state = UIDownloaderState::Cancelled;
                            store_video_editor_musicgen_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                    });
                    return;
                }
                Err(e) => {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut item) =
                            store_video_editor_musicgen_models_downloader!(ui).row_data(index)
                        {
                            item.state = UIDownloaderState::Failed;
                            store_video_editor_musicgen_models_downloader!(ui)
                                .set_row_data(index, item);
                        }
                        crate::toast_warn!(ui, format!("{}: {e}", tr("Download failed")));
                    });
                    return;
                }
            }
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Some(mut item) =
                store_video_editor_musicgen_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Finished;
                item.progress = 1.0;
                store_video_editor_musicgen_models_downloader!(ui).set_row_data(index, item);
            }

            let dir_str: SharedString = save_dir.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_musicgen_config();
            set_model_dir_by_index(&mut config, index, dir_str);
            global_store!(ui).set_video_editor_musicgen_config(config.clone());
            save_musicgen_config(config.into());

            crate::toast_success!(ui, tr("Model downloaded successfully"));
        });
    });
}

async fn download_single_file(url: &str, save_path: &PathBuf) -> Result<bool, String> {
    let dl = downloader::Downloader::new(url.to_string(), save_path.clone());

    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let result = dl
        .start(|_downloaded: u64, _total: u64, _progress: f32| {})
        .await
        .map_err(|e| e.to_string())?;

    match result {
        downloader::DownloadState::Finsished => Ok(true),
        downloader::DownloadState::Cancelled => Ok(false),
        downloader::DownloadState::Incompleted => Err("Download incomplete".to_string()),
    }
}

fn video_editor_musicgen_model_cancel_download(ui: &AppWindow, index: i32, _url: SharedString) {
    if let Some(mut item) =
        store_video_editor_musicgen_models_downloader!(ui).row_data(index as usize)
    {
        item.state = UIDownloaderState::Cancelled;
        store_video_editor_musicgen_models_downloader!(ui).set_row_data(index as usize, item);
    }
}

fn video_editor_musicgen_audio_player_sound_changed(_ui: &AppWindow, sound: f32) {
    if let Some(ref player) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        player.set_volume(sound.clamp(0.0, 1.0));
    }
}

fn video_editor_musicgen_audio_player_progress_changed(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_musicgen_audio_player_progress(value);
}

fn video_editor_musicgen_audio_player_progress_released(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_musicgen_audio_player_progress(value);
    if value >= 1.0 {
        return;
    }

    let samples_lock = GENERATED_SAMPLES.lock().unwrap();
    let Some(samples) = samples_lock.clone() else {
        return;
    };
    drop(samples_lock);

    let total_duration_ms = (samples.len() as u64 * 1000
        / MUSICGEN_SAMPLE_RATE as u64
        / MUSICGEN_CHANNEL as u64) as i32;
    if total_duration_ms <= 0 {
        return;
    }

    let start_ms = (value * total_duration_ms as f32) as u64;
    play_audio_from(ui, samples, start_ms, total_duration_ms as u64);
}

fn video_editor_musicgen_audio_player_progress_pressed(ui: &AppWindow, value: f32) {
    PLAYER_INC_INDEX.fetch_add(1, Ordering::Relaxed);
    global_store!(ui).set_video_editor_musicgen_audio_player_progress(value);
    video_editor_musicgen_stop_audio(ui);
}

fn video_editor_musicgen_play_audio(ui: &AppWindow) {
    let samples_lock = GENERATED_SAMPLES.lock().unwrap();
    let Some(samples) = samples_lock.clone() else {
        crate::toast_warn!(ui, tr("No audio generated yet"));
        return;
    };
    drop(samples_lock);

    let total_duration_ms = (samples.len() as u64 * 1000
        / MUSICGEN_SAMPLE_RATE as u64
        / MUSICGEN_CHANNEL as u64) as u64;

    let progress = global_store!(ui).get_video_editor_musicgen_audio_player_progress();
    let start_ms = if progress >= 1.0 {
        0
    } else {
        (progress * total_duration_ms as f32) as u64
    };

    play_audio_from(ui, samples, start_ms, total_duration_ms);
}

fn play_audio_from(ui: &AppWindow, samples: Vec<f32>, start_ms: u64, total_duration_ms: u64) {
    let sample_rate = MUSICGEN_SAMPLE_RATE as u64;
    let channels = MUSICGEN_CHANNEL as u64;
    let start_sample = ((sample_rate * start_ms * channels) / 1000) as usize;
    let end_sample = samples.len();

    if start_sample >= end_sample {
        return;
    }

    let play_samples = samples[start_sample..end_sample].to_vec();

    let player = {
        let player_lock = CURRENT_PLAYER.get().unwrap().lock().unwrap();
        player_lock.clone()
    };

    let Some(player) = player else {
        return;
    };

    player.stop();
    player.clear();

    let sound = global_store!(ui).get_video_editor_musicgen_audio_sound();
    player.set_volume(sound.clamp(0.0, 1.0));

    let source = SamplesBuffer::new(
        ChannelCount::new(MUSICGEN_CHANNEL).unwrap(),
        SampleRate::new(MUSICGEN_SAMPLE_RATE).unwrap(),
        play_samples,
    );
    player.append(source);

    global_store!(ui).set_video_editor_musicgen_audio_player_is_playing(true);

    let progress = (start_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0);
    global_store!(ui).set_video_editor_musicgen_audio_player_progress(progress as f32);

    let ui_weak = ui.as_weak();
    let inc_index = PLAYER_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;
    let player_clone = player.clone();

    // Spawn a thread to wait for playback completion
    std::thread::spawn(move || {
        player_clone.play();
        player_clone.sleep_until_end();
        player_clone.pause();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if PLAYER_INC_INDEX.load(Ordering::Relaxed) == inc_index {
                global_store!(ui).set_video_editor_musicgen_audio_player_is_playing(false);
                global_store!(ui).set_video_editor_musicgen_audio_player_progress(1.0);
            }
        });
    });

    // Progress update loop
    let ui_weak_progress = ui.as_weak();
    let start_time = std::time::Instant::now();
    let runtime_handle = tokio::runtime::Handle::current();

    runtime_handle.spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(20));
        loop {
            interval.tick().await;

            if PLAYER_INC_INDEX.load(Ordering::Relaxed) != inc_index {
                break;
            }

            let elapsed_ms = start_ms + start_time.elapsed().as_millis() as u64;
            let progress = (elapsed_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0);

            _ = ui_weak_progress.upgrade_in_event_loop(move |ui| {
                if PLAYER_INC_INDEX.load(Ordering::Relaxed) == inc_index {
                    global_store!(ui)
                        .set_video_editor_musicgen_audio_player_progress(progress as f32);
                }
            });

            if progress >= 1.0 {
                break;
            }
        }
    });
}

fn video_editor_musicgen_stop_audio(ui: &AppWindow) {
    global_store!(ui).set_video_editor_musicgen_audio_player_is_playing(false);
    PLAYER_INC_INDEX.fetch_add(1, Ordering::Relaxed);

    if let Some(ref player) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        player.stop();
        player.clear();
    }
}

fn video_editor_musicgen_export_audio(ui: &AppWindow) {
    let samples_lock = GENERATED_SAMPLES.lock().unwrap();
    let Some(samples) = samples_lock.clone() else {
        crate::toast_warn!(ui, tr("No audio to export"));
        return;
    };
    drop(samples_lock);

    let config = global_store!(ui).get_video_editor_musicgen_config();
    let export_dir = config.export_dir.to_string();

    if !export_dir.is_empty() && PathBuf::from(&export_dir).exists() {
        let output_path = PathBuf::from(&export_dir).join("musicgen_output.wav");
        save_wav_file(ui.as_weak(), &samples, &output_path);
    } else {
        let ui_weak = ui.as_weak();
        tokio::spawn(async move {
            let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose export directory"))
            else {
                return;
            };

            let export_dir = dirpath.to_string_lossy().to_string().into();
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let mut config = global_store!(ui).get_video_editor_musicgen_config();
                config.export_dir = export_dir;
                global_store!(ui).set_video_editor_musicgen_config(config.clone());
                save_musicgen_config(config.into());
            });

            let output_path = dirpath.join("musicgen_output.wav");
            save_wav_file(ui_weak, &samples, &output_path);
        });
    }
}

fn save_wav_file(ui_weak: Weak<AppWindow>, samples: &[f32], output_path: &PathBuf) {
    let spec = hound::WavSpec {
        channels: MUSICGEN_CHANNEL,
        sample_rate: MUSICGEN_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    match hound::WavWriter::create(output_path, spec) {
        Ok(mut writer) => {
            for sample in samples {
                let clamped = sample.clamp(-1.0, 1.0);
                let i16_sample = (clamped * 32767.0) as i16;
                if writer.write_sample(i16_sample).is_err() {
                    break;
                }
            }
            if let Err(e) = writer.finalize() {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("Failed to save audio file")),
                );
                return;
            }

            toast::async_toast_success(
                ui_weak.clone(),
                format!("{} {}", tr("Audio exported to"), output_path.display()),
            );

            // Add the exported audio to the playlist
            let output_path = output_path.clone();
            tokio::spawn(async move {
                import_file_to_playlist(ui_weak, output_path, None).await;
            });
        }
        Err(e) => {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Failed to create audio file")),
            );
        }
    }
}

fn get_model_dir(config: &UIMusicGenConfig) -> String {
    match config.selected_model_index {
        0 => config.small_model_dir.to_string(),
        1 => config.small_fp16_model_dir.to_string(),
        2 => config.small_quant_model_dir.to_string(),
        3 => config.medium_model_dir.to_string(),
        4 => config.medium_fp16_model_dir.to_string(),
        5 => config.medium_quant_model_dir.to_string(),
        6 => config.large_model_dir.to_string(),
        _ => config.small_model_dir.to_string(),
    }
}

fn set_model_dir_by_index(config: &mut UIMusicGenConfig, index: usize, dir: SharedString) {
    match index {
        0 => config.small_model_dir = dir,
        1 => config.small_fp16_model_dir = dir,
        2 => config.small_quant_model_dir = dir,
        3 => config.medium_model_dir = dir,
        4 => config.medium_fp16_model_dir = dir,
        5 => config.medium_quant_model_dir = dir,
        6 => config.large_model_dir = dir,
        _ => config.small_model_dir = dir,
    }
}
