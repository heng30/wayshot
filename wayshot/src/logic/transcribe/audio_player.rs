use crate::{
    config,
    db::{PLAYER_SETTING_TABLE as DB_TABLE, SettingPlayer},
    global_store,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::AppWindow,
    toast_warn,
};
use audio_utils::audio::AudioConfig;
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, VecModel};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

const AUDIO_PLAYER_SETTING_ID: &str = "transcribe_audio_player_setting_id";
static CURRENT_AUDIO_PLAYER: Lazy<Mutex<CurrentAudioPlayer>> =
    Lazy::new(|| Mutex::new(CurrentAudioPlayer::default()));

crate::db_remove_all!(DB_TABLE);
crate::db_add!(DB_TABLE, SettingPlayer);
crate::db_update!(DB_TABLE, SettingPlayer);

#[derive(Default)]
struct CurrentAudioPlayer {
    audio_config: Option<AudioConfig>,
    stop_sig: Option<Arc<AtomicBool>>,
    sound: Option<Arc<AtomicU32>>,
    file: String,
    current_time: Duration,

    inc_index: u64,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    // callback sound-waves-init(max-sample: int);
    // callback sound-wave-update(index: int, max-samples: int);
    // callback sound-wave-zoom-changed(index: int, level: float);
    // callback sound-wave-moved(index: int, percent: float);
    // callback sound-wave-start-position-changed(index: int, pos: float);
    // callback sound-wave-end-position-changed(index: int, pos: float);
    // logic_cb!(init_playlist, ui);
    // logic_cb!(get_current_playlist_index, ui);
    // logic_cb!(player_play_prev, ui, index);
    // logic_cb!(player_play_next, ui, index);
    // logic_cb!(player_play, ui, index);
    // logic_cb!(player_stop, ui);
    // logic_cb!(player_forward, ui, index);
    // logic_cb!(player_backward, ui, index);
    // logic_cb!(player_sound_changed, ui, sound);
    // logic_cb!(player_progress_changed, ui, index, progress);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let entry = if let Ok(setting) =
            sqldb::entry::select(DB_TABLE, AUDIO_PLAYER_SETTING_ID).await
            && let Ok(entry) = serde_json::from_str::<SettingPlayer>(&setting.data)
        {
            entry
        } else {
            let mut entry = SettingPlayer::default();
            entry.id = AUDIO_PLAYER_SETTING_ID.to_string();
            entry.current_time = "00:00".to_string();
            entry.end_time = "00:00".to_string();
            entry.sound = 50;

            db_remove_all(ui_weak.clone());
            db_add(ui_weak.clone(), entry.clone());

            entry
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_setting_trancribe_audio_player(entry.into());
        });
    });
}

pub fn get_current_audio_config() -> Option<AudioConfig> {
    CURRENT_AUDIO_PLAYER.lock().unwrap().audio_config.clone()
}

pub fn set_current_audio_config(config: Option<AudioConfig>) {
    CURRENT_AUDIO_PLAYER.lock().unwrap().audio_config = config;
}

pub fn get_sound_wave_amplitude(samples: &[f32]) -> f32 {
    let max_value = samples
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(&0.0);

    if *max_value == 0.0 {
        1.0
    } else {
        1.0 / max_value.abs().min(1.0)
    }
}
