use crate::{
    config, global_logic, global_store,
    logic::{
        toast::{self, async_toast_warn},
        tr::tr,
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, DenoiseStatus as UIDenoiseStatus, Fps as UIFps,
        MergeTrackStatus as UIMergeTrackStatus, RecordStatus as UIRecordStatus,
        Resolution as UIResolution, SettingControl as UISettingControl, Source as UISource,
        SourceType,
    },
    toast_success, toast_warn,
};
use anyhow::{Result, bail};
use once_cell::sync::Lazy;
use recorder::{
    AudioRecorder, FPS, RecorderConfig, RecordingSession, Resolution, SpeakerRecorder,
    StreamingAudioRecorder, bounded,
};
use slint::{
    ComponentHandle, Model, SharedPixelBuffer, SharedString, ToSharedString, VecModel, Weak,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    thread,
};

#[derive(Default)]
struct Cache {
    desktop_speaker_amplification: Option<Arc<AtomicI32>>,
    desktop_speaker_stop_sig: Option<Arc<AtomicBool>>,

    input_audio_amplification: Option<Arc<AtomicI32>>,
    input_streaming_audio_recorder: Option<StreamingAudioRecorder>,

    recorder_stop_sig: Option<Arc<AtomicBool>>,
    denoise_stop_sig: Option<Arc<AtomicBool>>,
    merge_stop_sig: Option<Arc<AtomicBool>>,

    speaker_device_info: Option<(u32, String)>,
}

static CACHE: Lazy<Mutex<Cache>> = Lazy::new(|| Mutex::new(Cache::default()));

#[macro_export]
macro_rules! store_audio_sources {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_audio_sources()
            .as_any()
            .downcast_ref::<VecModel<SharedString>>()
            .expect("We know we set a VecModel<SharedString> earlier")
    };
}

#[macro_export]
macro_rules! store_video_sources {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_sources()
            .as_any()
            .downcast_ref::<VecModel<SharedString>>()
            .expect("We know we set a VecModel<SharedString> earlier")
    };
}

#[macro_export]
macro_rules! store_sources {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_sources()
            .as_any()
            .downcast_ref::<VecModel<UISource>>()
            .expect("We know we set a VecModel<UISource> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(&ui);

    logic_cb!(init_sources_dialog, ui);
    logic_cb!(update_sources, ui, setting);
    logic_cb!(choose_save_dir, ui);
    logic_cb!(input_audio_amplification_changed, ui, v);
    logic_cb!(desktop_speaker_amplification_changed, ui, v);
    logic_cb!(refresh_desktop_speaker, ui, show_toast);
    logic_cb!(input_audio_changed, ui, name, show_toast);
    logic_cb!(start_recording, ui);
    logic_cb!(stop_recording, ui);
    logic_cb!(stop_merge_tracks, ui);
    logic_cb!(stop_denoise, ui);
}

fn inner_init(ui: &AppWindow) {
    global_store!(ui).set_preview_image(Default::default());
    store_sources!(ui).set_vec(vec![]);
    store_audio_sources!(ui).set_vec(vec![]);
    store_video_sources!(ui).set_vec(vec![]);

    global_store!(ui).set_ffmpeg_is_installed(recorder::is_ffmpeg_installed());

    if let Err(e) = init_input_audio(&ui) {
        toast_warn!(ui, format!("{e}"));
    }

    init_desktop_speaker(&ui);

    if let Err(e) = init_video(&ui) {
        toast_warn!(ui, format!("{e}"));
    }
}

fn init_input_audio(ui: &AppWindow) -> Result<()> {
    let mut names = vec![];
    let recorder = AudioRecorder::new(None)?;

    log::info!("Available Audio Devices:");

    let devices = recorder.get_available_devices()?;
    for (i, device) in devices.into_iter().enumerate() {
        log::info!(
            "{}. {} ({}) {:?}",
            i + 1,
            device.name,
            "Input",
            device.default_config
        );
        names.push(device.name.to_shared_string());
    }

    store_audio_sources!(ui).set_vec(names.clone());

    if names.is_empty() {
        bail!("available input device no found");
    }

    let control_config = config::all().control;
    if control_config.input_audio.is_empty()
        || names
            .iter()
            .find(|item| item.as_str() == control_config.input_audio.as_str())
            .is_none()
    {
        if let Some(default_input) = recorder.get_default_input_device()? {
            log::info!(
                "Default Input Device: {} {:?}",
                default_input.name,
                default_input.default_config
            );

            let mut control_setting = global_store!(ui).get_setting_control();
            control_setting.input_audio = default_input.name.into();
            control_setting.input_audio_gain = 0.0;
            control_setting.enable_input_audio = true;
            global_store!(ui).set_setting_control(control_setting.clone());
            global_logic!(ui).invoke_set_setting_control(control_setting);
        } else {
            bail!("Default input device no found");
        }
    }

    let name: SharedString = config::all().control.input_audio.into();
    store_sources!(ui).push(UISource {
        ty: SourceType::Audio,
        name: name.clone(),
    });

    input_audio_changed(&ui, name, false);

    Ok(())
}

fn init_desktop_speaker(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    if let Err(e) = create_desktop_speaker(ui) {
        log::warn!("{e}");
    }

    thread::spawn(move || {
        let Ok(recorder) =
            SpeakerRecorder::new(PathBuf::new(), Arc::new(AtomicBool::new(false)), None, true)
        else {
            log::warn!("init desktop speaker recorder failed");
            return;
        };

        loop {
            thread::sleep(std::time::Duration::from_secs(5));

            if !config::all().control.enable_desktop_speaker {
                continue;
            }

            let device_info = CACHE.lock().unwrap().speaker_device_info.clone();

            if speaker_device_changed(&recorder, &device_info) {
                log::info!("speaker device changed: current speaker deivce info: {device_info:?}");

                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    refresh_desktop_speaker(&ui, false);
                });
            }
        }
    });
}

fn create_desktop_speaker(ui: &AppWindow) -> Result<()> {
    let ui_weak = ui.as_weak();

    thread::spawn(move || {
        let (sender, receiver) = bounded(3);

        let ui_weak_clone = ui_weak.clone();
        thread::spawn(move || {
            while let Ok(db) = receiver.recv() {
                // log::debug!("speaker_level_receiver db level: {db:.0}",);

                _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_speaker_audio_db(db as i32);
                });
            }
            log::info!("exit desktop speaker receiver thread");
        });

        let stop_sig = Arc::new(AtomicBool::new(false));

        match SpeakerRecorder::new(
            PathBuf::new(),
            stop_sig.clone(),
            Some(Arc::new(sender)),
            true,
        ) {
            Err(e) => {
                async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("create desktop speaker recorder failed")),
                );

                return;
            }

            Ok(recorder) => {
                let amplification = Arc::new(AtomicI32::new(
                    config::all().control.desktop_speaker_gain as i32,
                ));
                let mut recorder = recorder.with_amplification(amplification.clone());

                {
                    let mut cache = CACHE.lock().unwrap();
                    cache.desktop_speaker_stop_sig = Some(stop_sig);
                    cache.desktop_speaker_amplification = Some(amplification);
                    cache.speaker_device_info = recorder.get_device_info();
                }

                if let Err(e) = recorder.start_recording() {
                    async_toast_warn(
                        ui_weak.clone(),
                        format!("{}: {e}", tr("start desktop speaker recorder failed")),
                    );
                    return;
                }
            }
        }

        log::info!("exit desktop speaker thread");
    });

    Ok(())
}

fn init_video(ui: &AppWindow) -> Result<()> {
    let mut names = vec![];
    let screen_infos = capture::available_screens()?;

    if screen_infos.is_empty() {
        bail!("available screen no found");
    }

    log::info!("Available Screen Devices:");

    for sinfo in screen_infos.into_iter() {
        log::info!("screen_infos: {sinfo:?}");
        names.push(sinfo.name.to_shared_string());
    }

    store_video_sources!(ui).set_vec(names.clone());

    if names.is_empty() {
        bail!("available screens no found");
    }

    let control_config = config::all().control;
    if control_config.screen.is_empty()
        || names
            .iter()
            .find(|item| item.as_str() == control_config.screen.as_str())
            .is_none()
    {
        log::info!("Default screen Device: {}", names[0]);

        let mut control_setting = global_store!(ui).get_setting_control();
        control_setting.screen = names[0].clone();
        global_store!(ui).set_setting_control(control_setting.clone());
        global_logic!(ui).invoke_set_setting_control(control_setting);
    }

    let name: SharedString = config::all().control.screen.into();
    store_sources!(ui).push(UISource {
        ty: SourceType::Video,
        name: name.clone(),
    });

    tokio::spawn(async move {
        if let Err(e) = RecordingSession::init(name.as_str()) {
            log::warn!("RecordingSession::init failed in `init_video`: {e}");
        }
    });

    Ok(())
}

fn init_sources_dialog(ui: &AppWindow) {
    if let Err(e) = inner_init_sources_dialog(ui) {
        log::warn!("{e}");
    }
}

fn inner_init_sources_dialog(ui: &AppWindow) -> Result<()> {
    let mut names = vec![];
    let recorder = AudioRecorder::new(None)?;

    log::info!("Available Audio Devices:");

    let devices = recorder.get_available_devices()?;
    for (i, device) in devices.into_iter().enumerate() {
        log::info!(
            "{}. {} ({}) {:?}",
            i + 1,
            device.name,
            "Input",
            device.default_config
        );
        names.push(device.name.to_shared_string());
    }
    store_audio_sources!(ui).set_vec(vec![]);
    store_audio_sources!(ui).set_vec(names);

    let mut names = vec![];
    let screen_infos = capture::available_screens()?;
    if screen_infos.is_empty() {
        bail!("available screen no found");
    }

    log::info!("Available Screen Devices:");

    for sinfo in screen_infos.into_iter() {
        log::info!("screen_infos: {sinfo:?}");
        names.push(sinfo.name.to_shared_string());
    }

    store_video_sources!(ui).set_vec(vec![]);
    store_video_sources!(ui).set_vec(names);

    Ok(())
}

fn update_sources(ui: &AppWindow, setting: UISettingControl) {
    store_sources!(ui).set_vec(vec![]);

    store_sources!(ui).push(UISource {
        ty: SourceType::Audio,
        name: setting.input_audio,
    });

    store_sources!(ui).push(UISource {
        ty: SourceType::Video,
        name: setting.screen,
    });
}

fn choose_save_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory"), "") else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_save_dir(dir.to_string_lossy().to_string().into());
        });
    });
}

fn input_audio_amplification_changed(_ui: &AppWindow, v: f32) {
    let amplification = CACHE.lock().unwrap().input_audio_amplification.clone();
    if let Some(amplification) = amplification {
        amplification.store(v as i32, Ordering::Relaxed);
    } else {
        log::warn!("input audio amplification is None");
    }
}

fn desktop_speaker_amplification_changed(_ui: &AppWindow, v: f32) {
    let amplification = CACHE.lock().unwrap().desktop_speaker_amplification.clone();
    if let Some(amplification) = amplification {
        amplification.store(v as i32, Ordering::Relaxed);
    } else {
        log::warn!("desktop speaker amplification is None");
    }
}

fn refresh_desktop_speaker(ui: &AppWindow, show_toast: bool) {
    {
        if let Some(stop_sig) = CACHE.lock().unwrap().desktop_speaker_stop_sig.take() {
            stop_sig.store(true, Ordering::Relaxed);
        }
    }

    if let Err(e) = create_desktop_speaker(ui) {
        if show_toast {
            toast_warn!(ui, format!("{e}"));
        } else {
            log::warn!("{e}");
        }
    } else {
        if show_toast {
            toast_success!(ui, "refresh successfully");
        }
    }
}

fn speaker_device_changed(recorder: &SpeakerRecorder, device_info: &Option<(u32, String)>) -> bool {
    let Ok(Some((node_id, node_name))) = recorder.find_default_output() else {
        log::warn!("find default speaker device failed");
        return false;
    };

    if let Some((old_node_id, old_node_name)) = device_info {
        return node_id != *old_node_id || node_name.as_str() != old_node_name.as_str();
    }

    true
}

fn input_audio_changed(ui: &AppWindow, name: SharedString, show_toast: bool) {
    if let Err(e) = inner_input_audio_changed(ui, name) {
        if show_toast {
            toast_warn!(ui, format!("{e}"));
        }
    } else {
        if show_toast {
            toast_success!(ui, "change input audio device successfully");
        }
    }
}

fn inner_input_audio_changed(ui: &AppWindow, name: SharedString) -> Result<()> {
    {
        let mut cache = CACHE.lock().unwrap();
        cache.input_audio_amplification.take();

        if let Some(recorder) = cache.input_streaming_audio_recorder.take() {
            _ = recorder.stop();
        }
    }

    let amplification = Arc::new(AtomicI32::new(
        config::all().control.input_audio_gain as i32,
    ));

    let recorder = AudioRecorder::new(Some(1024))?.with_amplification(amplification.clone());
    let streaming_recorder = StreamingAudioRecorder::start(recorder, &name, PathBuf::new(), true)?;

    let receiver = streaming_recorder.get_audio_level_receiver();

    {
        let mut cache = CACHE.lock().unwrap();
        cache.input_audio_amplification = Some(amplification);
        cache.input_streaming_audio_recorder = Some(streaming_recorder);
    }

    if let Some(receiver) = receiver {
        let ui_weak = ui.as_weak();
        thread::spawn(move || {
            while let Ok(db) = receiver.recv() {
                // log::debug!("input_audio_level_receiver db level: {db:.0}",);
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_input_audio_db(db as i32);
                });
            }
        });
    }

    Ok(())
}

fn start_recording(ui: &AppWindow) {
    let all_config = config::all();

    if all_config.recorder.save_dir.is_empty()
        || !PathBuf::from(all_config.recorder.save_dir.clone()).exists()
        || !PathBuf::from(all_config.recorder.save_dir.clone()).is_dir()
    {
        let ui_weak = ui.as_weak();
        tokio::spawn(async move {
            let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory"), "")
            else {
                return;
            };

            let mut all = config::all();
            all.recorder.save_dir = dir.to_string_lossy().to_string();
            _ = config::save(all);
        });
        return;
    }

    let ui_weak = ui.as_weak();
    thread::spawn(move || {
        if let Err(e) = inner_start_recording(ui_weak.clone()) {
            toast::async_toast_warn(ui_weak, e.to_string());
        }
    });
}

fn inner_start_recording(ui_weak: Weak<AppWindow>) -> Result<()> {
    log::info!("start recording...");
    let all_config = config::all();

    if all_config.control.screen.is_empty() {
        bail!("available screen no found");
    }

    let screen_info = capture::available_screens()?
        .into_iter()
        .find(|item| item.name == all_config.control.screen);

    if screen_info.is_none() {
        bail!("no found screen: {}", all_config.control.screen);
    }

    let screen_info = screen_info.unwrap();
    log::debug!("screen_info: {screen_info:?}");

    let resolution = if matches!(all_config.recorder.resolution, UIResolution::Original) {
        Resolution::Original((
            screen_info.logical_size.width as u32,
            screen_info.logical_size.height as u32,
        ))
    } else {
        all_config.recorder.resolution.into()
    };

    let input_audio_name =
        if all_config.control.input_audio.is_empty() || !all_config.control.enable_input_audio {
            None
        } else {
            Some(all_config.control.input_audio)
        };

    if !RecordingSession::init_finished() {
        RecordingSession::init(&all_config.control.screen)?;
    }

    let config = RecorderConfig::new(
        all_config.control.screen.clone(),
        screen_info.logical_size.clone(),
        RecorderConfig::make_filename(&all_config.recorder.save_dir),
    )
    .with_enable_frame_channel_user(true)
    .with_enable_preview_mode(all_config.recorder.enable_preview)
    .with_enable_denoise(all_config.recorder.enable_denoise)
    .with_real_time_denoise(all_config.recorder.real_time_denoise)
    .with_convert_input_wav_to_mono(all_config.recorder.convert_input_wav_to_mono)
    .with_enable_recording_speaker(all_config.control.enable_desktop_speaker)
    .with_include_cursor(all_config.recorder.include_cursor)
    .with_remove_cache_files(all_config.recorder.remove_temporary_files)
    .with_audio_device_name(input_audio_name)
    .with_audio_amplification(Arc::new(AtomicI32::new(
        all_config.control.input_audio_gain as i32,
    )))
    .with_speaker_amplification(Arc::new(AtomicI32::new(
        all_config.control.desktop_speaker_gain as i32,
    )))
    .with_fps(all_config.recorder.fps.clone().into())
    .with_resolution(resolution);

    log::info!("Recording configuration: {:#?}", config);

    let mut session = RecordingSession::new(config);
    session.start()?;

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_denoise_progress(0.0);
        global_store!(ui).set_merge_tracks_progress(0.0);
        global_store!(ui).set_denoise_status(UIDenoiseStatus::None);
        global_store!(ui).set_merge_tracks_status(UIMergeTrackStatus::None);
        global_store!(ui).set_record_status(UIRecordStatus::Recording);
    });

    let stop_sig = session.stop_sig().clone();
    let stop_sig_merge = session.get_stop_combine_tracks();
    let stop_sig_denoise = session.get_stop_denoise();
    {
        let mut cache = CACHE.lock().unwrap();
        cache.recorder_stop_sig = Some(stop_sig);
        cache.merge_stop_sig = Some(stop_sig_merge);
        cache.denoise_stop_sig = Some(stop_sig_denoise);
    }

    let frame_receiver_user = session.get_frame_receiver_user();
    let ui_weak_clone = ui_weak.clone();
    thread::spawn(move || {
        if let Some(rx) = frame_receiver_user {
            while let Ok(frame) = rx.recv() {
                if let Some(ref frame) = frame.frame {
                    log::debug!(
                        "frame_receiver_user frame len: {} bytes",
                        frame.cb_data.data.pixel_data.len()
                    );
                }

                _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                    if let Some(ref frame) = frame.frame {
                        let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                            &frame.cb_data.data.pixel_data,
                            frame.cb_data.data.width,
                            frame.cb_data.data.height,
                        );
                        let img = slint::Image::from_rgba8(buffer);
                        global_store!(ui).set_preview_image(img);
                    }

                    let mut sinfo = global_store!(ui).get_stats_info();
                    sinfo.fps = frame.stats.fps;
                    sinfo.total = frame.stats.total_frames as i32;
                    sinfo.loss =
                        frame.stats.loss_frames as f32 / frame.stats.total_frames.max(1) as f32;
                    global_store!(ui).set_stats_info(sinfo);
                });
            }
            log::info!("exit frame_receiver_user");
        } else {
            log::info!("frame_receiver_user is none");
        }
    });

    let ui_weak_denoise_clone = ui_weak.clone();
    let ui_weak_merge_clone = ui_weak.clone();
    let final_video_path = session.output_path();

    session.wait(
        Some(move |v| {
            // log::debug!("denoise progress: {}%", (v * 100.0) as u32);
            _ = ui_weak_denoise_clone.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_record_status(UIRecordStatus::Denoising);
                global_store!(ui).set_denoise_progress(v);
            });
        }),
        move |v| {
            // log::debug!("combine tracks progress: {}%", (v * 100.0) as u32);
            _ = ui_weak_merge_clone.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_record_status(UIRecordStatus::Mergeing);
                global_store!(ui).set_merge_tracks_progress(v);
            });
        },
    )?;

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_record_status(UIRecordStatus::Stopped);
        let all_config = config::all();

        if all_config.recorder.enable_denoise
            && !all_config.recorder.real_time_denoise
            && global_store!(ui).get_denoise_status() != UIDenoiseStatus::Cancelled
        {
            global_store!(ui).set_denoise_status(UIDenoiseStatus::Finished);
        }

        if global_store!(ui).get_denoise_status() != UIDenoiseStatus::Cancelled
            && global_store!(ui).get_merge_tracks_status() != UIMergeTrackStatus::Cancelled
        {
            global_store!(ui).set_merge_tracks_status(UIMergeTrackStatus::Finished);
            global_store!(ui).set_final_video_path(final_video_path.display().to_shared_string());
        }
    });

    log::info!("Recording completed successfully!");

    Ok(())
}

fn stop_recording(ui: &AppWindow) {
    let stop_sig = CACHE.lock().unwrap().recorder_stop_sig.take();
    if let Some(sig) = stop_sig {
        sig.store(true, Ordering::Relaxed);
    } else {
        log::warn!("recorder_stop_sig is None");
    }

    let all_config = config::all();
    if all_config.recorder.enable_denoise && !all_config.recorder.real_time_denoise {
        global_store!(ui).set_record_status(UIRecordStatus::Denoising);
    } else {
        global_store!(ui).set_record_status(UIRecordStatus::Mergeing);
    }
}

fn stop_denoise(ui: &AppWindow) {
    let stop_sig = CACHE.lock().unwrap().denoise_stop_sig.take();
    if let Some(sig) = stop_sig {
        sig.store(true, Ordering::Relaxed);
    } else {
        log::warn!("denoise_stop_sig is None");
    }

    global_store!(ui).set_record_status(UIRecordStatus::Stopped);
    global_store!(ui).set_denoise_status(UIDenoiseStatus::Cancelled);
}

fn stop_merge_tracks(ui: &AppWindow) {
    let stop_sig = CACHE.lock().unwrap().merge_stop_sig.take();
    if let Some(sig) = stop_sig {
        sig.store(true, Ordering::Relaxed);
    } else {
        log::warn!("merge_stop_sig is None");
    }

    global_store!(ui).set_record_status(UIRecordStatus::Stopped);
    global_store!(ui).set_merge_tracks_status(UIMergeTrackStatus::Cancelled);
}

pub fn picker_directory(ui: Weak<AppWindow>, title: &str, filename: &str) -> Option<PathBuf> {
    let result = native_dialog::DialogBuilder::file()
        .set_title(title)
        .set_filename(filename)
        .open_single_dir()
        .show();

    match result {
        Ok(Some(path)) => Some(path),
        Err(e) => {
            toast::async_toast_warn(
                ui,
                format!("{}. {}: {}", tr("Choose directory failed"), tr("Reason"), e),
            );
            None
        }
        _ => None,
    }
}

impl From<UIResolution> for Resolution {
    fn from(entry: UIResolution) -> Self {
        match entry {
            UIResolution::P720 => Resolution::P720,
            UIResolution::P1080 => Resolution::P1080,
            UIResolution::P2K => Resolution::P2K,
            UIResolution::P4K => Resolution::P4K,
            _ => unreachable!(),
        }
    }
}

impl From<UIFps> for FPS {
    fn from(entry: UIFps) -> Self {
        match entry {
            UIFps::Fps24 => FPS::Fps24,
            UIFps::Fps25 => FPS::Fps25,
            UIFps::Fps30 => FPS::Fps30,
            UIFps::Fps60 => FPS::Fps60,
        }
    }
}
