use crate::{
    config,
    db::{PLAYER_SETTING_TABLE as DB_TABLE, SettingPlayer},
    global_store,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, HistoryEntry as UIHistoryEntry, PlaylistItem as UIPlaylistItem,
    },
    store_history_entries, toast_warn,
};
use mp4_player::{Config as PlayerConfig, DecodedVideoFrame, Mp4Player};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, SharedPixelBuffer, VecModel};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

const PLAYER_SETTING_ID: &str = "player_setting_id";
static CURRENT_PLAYER: Lazy<Mutex<CurrentPlayer>> =
    Lazy::new(|| Mutex::new(CurrentPlayer::default()));

#[macro_export]
macro_rules! store_playlist_entries {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_playlist_entries()
            .as_any()
            .downcast_ref::<VecModel<UIPlaylistItem>>()
            .expect("We know we set a VecModel<UIPlaylistItem> earlier")
    };
}

crate::db_remove_all!(DB_TABLE);
crate::db_add!(DB_TABLE, SettingPlayer);
crate::db_update!(DB_TABLE, SettingPlayer);

#[derive(Default)]
struct CurrentPlayer {
    player: Option<Mp4Player>,
    stop_sig: Option<Arc<AtomicBool>>,
    sound: Option<Arc<AtomicU32>>,
    file: String,
    current_time: Duration,

    inc_index: u64,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(init_playlist, ui);
    logic_cb!(get_current_playlist_index, ui);
    logic_cb!(player_play_prev, ui, index);
    logic_cb!(player_play_next, ui, index);
    logic_cb!(player_play, ui, index);
    logic_cb!(player_stop, ui);
    logic_cb!(player_forward, ui, index);
    logic_cb!(player_backward, ui, index);
    logic_cb!(player_sound_changed, ui, sound);
    logic_cb!(player_progress_changed, ui, index, progress);
}

fn inner_init(ui: &AppWindow) {
    store_playlist_entries!(ui).set_vec(vec![]);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let entry = if let Ok(setting) = sqldb::entry::select(DB_TABLE, PLAYER_SETTING_ID).await
            && let Ok(entry) = serde_json::from_str::<SettingPlayer>(&setting.data)
        {
            entry
        } else {
            let mut entry = SettingPlayer::default();
            entry.id = PLAYER_SETTING_ID.to_string();
            entry.current_time = "00:00".to_string();
            entry.end_time = "00:00".to_string();
            entry.sound = 50;

            db_remove_all(ui_weak.clone());
            db_add(ui_weak.clone(), entry.clone());

            entry
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_setting_player(entry.into());
        });
    });
}

fn init_playlist(ui: &AppWindow) {
    let playlist = store_history_entries!(ui)
        .iter()
        .map(|item| UIPlaylistItem {
            title: item.file,
            duration: item.duration,
        })
        .collect::<Vec<_>>();

    store_playlist_entries!(ui).set_vec(playlist);
}

fn get_current_playlist_index(ui: &AppWindow) -> i32 {
    let file = &CURRENT_PLAYER.lock().unwrap().file;
    if let Some(index) = store_history_entries!(ui)
        .iter()
        .position(|item| item.file.as_str() == file.as_str())
    {
        index as i32
    } else {
        -1
    }
}

fn player_play_prev(ui: &AppWindow, index: i32) {
    player_play(ui, index);
}

fn player_play_next(ui: &AppWindow, index: i32) {
    player_play(ui, index);
}

fn player_play(ui: &AppWindow, index: i32) {
    if index < 0 || index as usize >= store_history_entries!(ui).row_count() {
        player_stop(ui);
        return;
    }

    let history_entry = store_history_entries!(ui).row_data(index as usize).unwrap();
    let Some(end_time) = cutil::time::media_timestamp_to_second(&history_entry.duration) else {
        player_stop(ui);
        toast_warn!(ui, tr("video file duration is 0"));
        return;
    };

    let file_path = PathBuf::from(config::all().recorder.save_dir).join(&history_entry.file);
    if !file_path.exists() {
        player_stop(ui);
        toast_warn!(ui, format!("{} `{}`", tr("No found"), file_path.display()));
        return;
    }

    let (current_time, inc_index) = {
        let mut current_player = CURRENT_PLAYER.lock().unwrap();
        current_player.inc_index += 1;

        if current_player.player.is_none()
            || current_player.file.as_str() != history_entry.file.as_str()
            || current_player.current_time.as_secs() + 1 > end_time
        {
            (Duration::ZERO, current_player.inc_index)
        } else {
            (
                current_player.current_time.clone(),
                current_player.inc_index,
            )
        }
    };

    let mut player_setting = global_store!(ui).get_setting_player();
    let stop_sig = Arc::new(AtomicBool::new(false));
    let sound = Arc::new(AtomicU32::new(player_setting.sound.clamp(0, 100) as u32));

    player_setting.current_time =
        cutil::time::seconds_to_media_timestamp(current_time.as_secs_f64()).into();
    player_setting.end_time = history_entry.duration.clone();
    global_store!(ui).set_setting_player(player_setting);

    if current_time.is_zero() {
        global_store!(ui).set_player_progress(0.0);
    }

    global_store!(ui).set_player_is_playing(true);

    let config = PlayerConfig::new(file_path)
        .with_stop_sig(stop_sig.clone())
        .with_sound(sound.clone());

    match Mp4Player::new(config) {
        Ok(mut player) => {
            player.play(current_time);

            let ui_weak = ui.as_weak();
            let video_receiver = player.video_frame_receiver();

            {
                let mut current_player = CURRENT_PLAYER.lock().unwrap();
                current_player.file = history_entry.file.clone().into();
                current_player.player = Some(player);
                current_player.sound = Some(sound);
                current_player.stop_sig = Some(stop_sig.clone());
                current_player.current_time = current_time;
            }

            std::thread::spawn(move || {
                let mut frame_count = 0;

                loop {
                    if stop_sig.load(Ordering::Relaxed) {
                        break;
                    }

                    if let Ok(decoded_frame) =
                        video_receiver.recv_timeout(Duration::from_millis(10))
                    {
                        frame_count += 1;

                        match decoded_frame {
                            DecodedVideoFrame::Data(frame) => {
                                {
                                    let mut current_player = CURRENT_PLAYER.lock().unwrap();
                                    if current_player.inc_index != inc_index {
                                        break;
                                    }

                                    current_player.current_time = frame.timestamp;
                                }

                                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                                    let mut setting = global_store!(ui).get_setting_player();
                                    setting.current_time = cutil::time::seconds_to_media_timestamp(
                                        frame.timestamp.as_secs_f64(),
                                    )
                                    .into();
                                    global_store!(ui).set_setting_player(setting);

                                    let progress = (frame.timestamp.as_secs_f32() * 100.0
                                        / end_time as f32)
                                        .clamp(0.0, 100.0);
                                    global_store!(ui).set_player_progress(progress);

                                    let buffer =
                                        SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                                            &frame.image_buffer.as_raw(),
                                            frame.image_buffer.width(),
                                            frame.image_buffer.height(),
                                        );
                                    let img = slint::Image::from_rgb8(buffer);
                                    global_store!(ui).set_player_image(img);
                                });
                            }
                            DecodedVideoFrame::Empty => {
                                log::debug!("📹 Video frame #{}: Empty frame", frame_count);
                            }
                            DecodedVideoFrame::EOF => {
                                log::debug!("receiver `DecodedVideoFrame::EOF`");

                                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                                    player_stop(&ui);
                                });
                            }
                            DecodedVideoFrame::None => (),
                        }
                    }
                }

                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    if CURRENT_PLAYER.lock().unwrap().inc_index == inc_index {
                        player_stop(&ui);
                    }
                });
            });
        }
        Err(e) => {
            toast_warn!(ui, format!("{}: {e}", tr("new mp4 player failed")));
            return;
        }
    }
}

fn player_stop(ui: &AppWindow) {
    let current_player = CURRENT_PLAYER.lock().unwrap();
    if let Some(ref sig) = current_player.stop_sig {
        sig.store(true, Ordering::Relaxed);
    }

    global_store!(ui).set_player_is_playing(false);
}

fn player_forward(ui: &AppWindow, index: i32) {
    player_current_offset(ui, index, 5);
}

fn player_backward(ui: &AppWindow, index: i32) {
    player_current_offset(ui, index, -5);
}

fn player_current_offset(ui: &AppWindow, index: i32, offset: i64) {
    let mut setting = global_store!(ui).get_setting_player();
    let current_time = CURRENT_PLAYER.lock().unwrap().current_time.as_secs();

    match cutil::time::media_timestamp_to_second(&setting.end_time) {
        Some(end_time) => {
            let current_time = if offset < 0 && (current_time as i64) < offset.abs() {
                0u64
            } else {
                (current_time as i64 + offset) as u64
            };

            if current_time >= end_time {
                setting.current_time = setting.end_time.clone();
                global_store!(ui).set_setting_player(setting);
                global_store!(ui).set_player_progress(100.0);

                CURRENT_PLAYER.lock().as_mut().unwrap().current_time =
                    Duration::from_secs(end_time);

                player_stop(ui);
            } else {
                let current_time = current_time.clamp(0, end_time);
                setting.current_time =
                    cutil::time::seconds_to_media_timestamp(current_time as f64).into();
                global_store!(ui).set_setting_player(setting);

                CURRENT_PLAYER.lock().as_mut().unwrap().current_time =
                    Duration::from_secs(current_time);

                player_play(ui, index);
            }
        }
        _ => log::warn!("invalid end time: {}", setting.end_time),
    }
}

fn player_sound_changed(ui: &AppWindow, sound: i32) {
    let setting = global_store!(ui).get_setting_player();
    db_update(ui.as_weak(), setting.into());

    if let Some(ref sig) = CURRENT_PLAYER.lock().as_ref().unwrap().sound {
        sig.store(sound.clamp(0, 100) as u32, Ordering::Relaxed);
    }
}

fn player_progress_changed(ui: &AppWindow, index: i32, progress: f32) {
    if progress < 0.0 || progress > 100.0 {
        return;
    }

    let mut setting = global_store!(ui).get_setting_player();
    if let Some(end_time) = cutil::time::media_timestamp_to_second(&setting.end_time) {
        let current_time = end_time as f64 * progress as f64 / 100.0;
        setting.current_time = cutil::time::seconds_to_media_timestamp(current_time).into();
        global_store!(ui).set_setting_player(setting);

        CURRENT_PLAYER.lock().as_mut().unwrap().current_time =
            Duration::from_secs_f64(current_time);

        player_play(ui, index);
    }
}
