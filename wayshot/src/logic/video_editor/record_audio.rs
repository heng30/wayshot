use super::{
    common_type::VideoEditorRecordAudioConfig, playlist::import_file_to_playlist,
    project::RECORD_AUDIO_CONFIG_ID,
};
use crate::{
    db::VIDEO_EDITOR_TABLE,
    global_store,
    logic::{recorder::picker_directory, toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, VideoEditorRecordAudioConfig as UIVideoEditorRecordAudioConfig,
        VideoEditorVolume as UIVideoEditorVolume,
    },
};
use crossbeam::channel::{self, Receiver, Sender};
use record_audio::{AudioDeviceInfo, AudioRecorder};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{
    path::PathBuf,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

static RECORD_AUDIO_STATE: Mutex<Option<RecordAudioState>> = Mutex::new(None);

pub struct RecordAudioState {
    recorder: AudioRecorder,
    level_receiver: Option<Receiver<(f32, f32)>>,
    level_sender: Option<Sender<(f32, f32)>>,
    recording_start_time: Option<Instant>,
}

pub fn init(ui: &AppWindow) {
    init_inner(ui);

    logic_cb!(video_editor_start_recording_audio, ui);
    logic_cb!(video_editor_stop_recording_audio, ui);
    logic_cb!(video_editor_record_audio_select_dir, ui);
    logic_cb!(video_editor_record_audio_update, ui, config);
}

pub fn init_inner(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let ui_weak_for_devices = ui.as_weak();

    {
        let mut state = RECORD_AUDIO_STATE.lock().unwrap();
        *state = Some(RecordAudioState {
            recorder: AudioRecorder::new(),
            level_receiver: None,
            level_sender: None,
            recording_start_time: None,
        });
    }

    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, RECORD_AUDIO_CONFIG_ID).await {
            Ok(setting) => serde_json::from_str::<VideoEditorRecordAudioConfig>(&setting.data)
                .unwrap_or_default(),
            _ => {
                let config = VideoEditorRecordAudioConfig::default();
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, RECORD_AUDIO_CONFIG_ID, "{}").await;
                config
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_record_audio_config(config.into());
        });
    });

    // Get available audio devices
    tokio::spawn(async move {
        let devices: Vec<SharedString> = {
            let state = RECORD_AUDIO_STATE.lock().unwrap();
            match *state {
                Some(ref state) => match state.recorder.get_input_devices() {
                    Ok(device_list) => device_list
                        .into_iter()
                        .map(|d: AudioDeviceInfo| d.name.into())
                        .collect(),
                    Err(e) => {
                        log::warn!("Failed to get audio input devices: {}", e);
                        vec![]
                    }
                },
                _ => vec![],
            }
        };

        _ = ui_weak_for_devices.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_audio_devices(ModelRc::new(VecModel::from(devices)));
        });
    });
}

fn video_editor_start_recording_audio(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let config = global_store!(ui).get_video_editor_record_audio_config();
    let save_dir = config.save_dir.to_string();
    let device = config.device.to_string();
    let gain = config.gain;
    let mono = config.mono;

    if save_dir.is_empty() {
        crate::toast_warn!(ui, tr("Please select a save directory first"));
        return;
    }

    let save_path = PathBuf::from(&save_dir);
    if !save_path.exists() {
        crate::toast_warn!(ui, tr("Save directory does not exist"));
        return;
    }

    thread::spawn(move || {
        let device_name = if device.is_empty() {
            None
        } else {
            Some(device.as_str())
        };

        let (sender, receiver) = channel::bounded::<(f32, f32)>(32);
        let recorder = AudioRecorder::new()
            .with_level_sender(sender.clone())
            .with_gain(gain)
            .with_mono(mono);

        {
            let mut state = RECORD_AUDIO_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                s.recorder = recorder;
                s.level_sender = Some(sender);
                s.level_receiver = Some(receiver);
                s.recording_start_time = Some(Instant::now());

                match s.recorder.start_recording(device_name) {
                    Ok(()) => log::info!("Audio recording started"),
                    Err(e) => {
                        toast::async_toast_warn(
                            ui_weak.clone(),
                            format!("{}: {}", tr("Failed to start recording"), e),
                        );
                        return;
                    }
                }
            } else {
                log::error!("Record audio state not initialized");
                return;
            }
        }

        _ = ui_weak.upgrade_in_event_loop(|ui| {
            global_store!(ui).set_video_editor_is_recording_audio(true);
            global_store!(ui).set_video_editor_record_audio_volume(UIVideoEditorVolume {
                left: -200.0,
                right: -200.0,
            });
        });

        // Spawn thread to update volume UI
        let ui_weak_volume = ui_weak.clone();
        thread::spawn(move || {
            loop {
                let receiver_opt = {
                    let state = RECORD_AUDIO_STATE.lock().unwrap();
                    if let Some(ref s) = *state {
                        s.level_receiver.clone()
                    } else {
                        None
                    }
                };

                let Some(receiver) = receiver_opt else {
                    break;
                };

                let is_recording = {
                    let state = RECORD_AUDIO_STATE.lock().unwrap();
                    if let Some(ref s) = *state {
                        s.recorder.is_recording()
                    } else {
                        false
                    }
                };

                if !is_recording {
                    break;
                }

                match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok((left, right)) => {
                        _ = ui_weak_volume.upgrade_in_event_loop(move |ui| {
                            global_store!(ui).set_video_editor_record_audio_volume(
                                UIVideoEditorVolume { left, right },
                            );
                        });
                    }
                    Err(channel::RecvTimeoutError::Timeout) => {}
                    Err(channel::RecvTimeoutError::Disconnected) => break,
                }

                let duration_secs = {
                    let state = RECORD_AUDIO_STATE.lock().unwrap();
                    if let Some(ref s) = *state {
                        s.recorder.recorded_duration_secs() as i32
                    } else {
                        0
                    }
                };

                _ = ui_weak_volume.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_record_audio_duration(duration_secs);
                });
            }
        });
    });
}

fn video_editor_stop_recording_audio(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let config = global_store!(ui).get_video_editor_record_audio_config();
    let save_dir = config.save_dir.to_string();

    tokio::spawn(async move {
        let recorded_audio = {
            let mut state = RECORD_AUDIO_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                match s.recorder.stop_recording() {
                    Ok(audio) => {
                        s.level_receiver = None;
                        s.level_sender = None;
                        s.recording_start_time = None;
                        Some(audio)
                    }
                    Err(e) => {
                        toast::async_toast_warn(
                            ui_weak.clone(),
                            format!("{}: {}", tr("Failed to stop recording"), e),
                        );
                        return;
                    }
                }
            } else {
                log::error!("Record audio state not initialized");
                return;
            }
        };

        let Some(audio) = recorded_audio else {
            return;
        };

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("recording_{}.wav", timestamp);
        let save_path = PathBuf::from(&save_dir).join(&filename);

        toast::async_toast_info(ui_weak.clone(), format!("{} {}", tr("Saving"), save_path.display()));

        match audio.save_to_file(&save_path) {
            Ok(()) => {
                let duration = audio.duration_secs();
                log::info!(
                    "Audio saved to {} (duration: {:.1}s)",
                    save_path.display(),
                    duration
                );
                toast::async_toast_success(
                    ui_weak.clone(),
                    format!("{}: {} ({:.1}s)", tr("Audio saved"), filename, duration),
                );

                import_file_to_playlist(ui_weak.clone(), save_path, None).await;
            }
            Err(e) => {
                log::error!("Failed to save audio: {}", e);
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to save audio"), e),
                );
            }
        }

        _ = ui_weak.upgrade_in_event_loop(|ui| {
            global_store!(ui).set_video_editor_is_recording_audio(false);
            global_store!(ui).set_video_editor_record_audio_duration(0);
            global_store!(ui).set_video_editor_record_audio_volume(UIVideoEditorVolume {
                left: -200.0,
                right: -200.0,
            });
        });
    });
}

fn video_editor_record_audio_select_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let current_config = global_store!(ui).get_video_editor_record_audio_config();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory")) else {
            return;
        };

        let dir_str = dir.to_string_lossy().to_string();
        let new_config = UIVideoEditorRecordAudioConfig {
            save_dir: dir_str.clone().into(),
            device: current_config.device.clone(),
            gain: current_config.gain,
            mono: current_config.mono,
        };

        let config: VideoEditorRecordAudioConfig = new_config.clone().into();
        let data = serde_json::to_string(&config).expect("Failed to serialize record audio config");
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, RECORD_AUDIO_CONFIG_ID, &data).await
        {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}. {e}", tr("update entry failed")),
            );
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_record_audio_config(new_config);
            crate::toast_success!(ui, tr("Save directory updated"));
        });
    });
}

fn video_editor_record_audio_update(ui: &AppWindow, config: UIVideoEditorRecordAudioConfig) {
    let ui_weak = ui.as_weak();
    let config_inner: VideoEditorRecordAudioConfig = config.clone().into();

    tokio::spawn(async move {
        let data =
            serde_json::to_string(&config_inner).expect("Failed to serialize record audio config");
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, RECORD_AUDIO_CONFIG_ID, &data).await
        {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}. {e}", tr("update entry failed")),
            );
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_record_audio_config(config);
            crate::toast_success!(ui, tr("Config updated"));
        });
    });
}
