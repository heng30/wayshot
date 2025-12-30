use crate::{
    config, global_logic, global_store,
    logic::{
        toast::{self, async_toast_warn},
        tr::tr,
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, FeatureType, Fps as UIFps, ProcessMode as UIProcessMode,
        RecordStatus as UIRecordStatus, Resolution as UIResolution,
        SettingControl as UISettingControl, Source as UISource, SourceType,
    },
    toast_success, toast_warn,
};
use anyhow::{Result, bail};
use once_cell::sync::Lazy;
use recorder::{
    AsyncErrorChannel, AsyncErrorReceiver, AsyncErrorSender, AudioRecorder, FPS, ProcessMode,
    RecorderConfig, RecordingSession, Resolution, SpeakerRecorder, SpeakerRecorderConfig, bounded,
    platform_screen_capture, platform_speaker_recoder,
};
use screen_capture::{ScreenCapture, ScreenInfo};
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
    recorder_stop_sig: Option<Arc<AtomicBool>>,

    audio_gain: Option<Arc<AtomicI32>>,
    audio_recorder: Option<AudioRecorder>,

    speaker_gain: Option<Arc<AtomicI32>>,
    speaker_stop_sig: Option<Arc<AtomicBool>>,
    speaker_device_info: Option<(u32, String)>,

    async_error_sender: Option<AsyncErrorSender>,
}

static CACHE: Lazy<Mutex<Cache>> = Lazy::new(|| Mutex::new(Cache::default()));

crate::impl_c_like_enum_convert!(
    UIProcessMode,
    ProcessMode,
    RecordScreen,
    ShareScreen,
    PushStream
);

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

    logic_cb!(toggle_control_enable_stats, ui);
    logic_cb!(toggle_control_enable_preview, ui);

    logic_cb!(init_sources_dialog, ui);
    logic_cb!(choose_save_dir, ui);
    logic_cb!(update_sources, ui, setting);

    logic_cb!(audio_changed, ui, name, show_toast);
    logic_cb!(audio_gain_changed, ui, v);

    logic_cb!(refresh_speaker, ui, show_toast);
    logic_cb!(speaker_gain_changed, ui, v);

    logic_cb!(start_recording, ui);
    logic_cb!(stop_recording, ui);

    logic_cb!(cal_region_width, ui, height);
    logic_cb!(cal_region_height, ui, width);

    logic_cb!(open_file, ui, file);
}

fn inner_init(ui: &AppWindow) {
    #[cfg(feature = "desktop-wayland-wlr")]
    global_store!(ui).set_feature_type(FeatureType::WaylandWlr);

    #[cfg(feature = "desktop-wayland-portal")]
    global_store!(ui).set_feature_type(FeatureType::WaylandPortal);

    #[cfg(feature = "desktop-windows")]
    global_store!(ui).set_feature_type(FeatureType::Windows);

    global_store!(ui).set_preview_image(Default::default());
    store_sources!(ui).set_vec(vec![]);
    store_audio_sources!(ui).set_vec(vec![]);
    store_video_sources!(ui).set_vec(vec![]);

    let (async_error_sender, async_error_receiver) = AsyncErrorChannel(16);
    show_async_error_task(ui.as_weak(), async_error_receiver);
    CACHE.lock().unwrap().async_error_sender = Some(async_error_sender);

    if let Err(e) = init_audio(&ui) {
        toast_warn!(ui, format!("{e}"));
    }

    init_speaker(&ui);

    if let Err(e) = init_video(&ui) {
        toast_warn!(ui, format!("{e}"));
    }
}

fn init_audio(ui: &AppWindow) -> Result<()> {
    let mut names = vec![];
    let recorder = AudioRecorder::new();

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
    if control_config.audio.is_empty()
        || names
            .iter()
            .find(|item| item.as_str() == control_config.audio.as_str())
            .is_none()
    {
        if let Some(default_input) = recorder.get_default_input_device()? {
            log::info!(
                "Default Input Device: {} {:?}",
                default_input.name,
                default_input.default_config
            );

            let mut control_setting = global_store!(ui).get_setting_control();
            control_setting.audio = default_input.name.into();
            control_setting.audio_gain = 0.0;
            control_setting.enable_audio = true;
            global_store!(ui).set_setting_control(control_setting.clone());
            global_logic!(ui).invoke_set_setting_control(control_setting);
        } else {
            bail!("Default input device no found");
        }
    }

    let name: SharedString = config::all().control.audio.into();
    store_sources!(ui).push(UISource {
        ty: SourceType::Audio,
        name: name.clone(),
    });

    audio_changed(&ui, name, false);

    Ok(())
}

fn init_speaker(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    if let Err(e) = create_speaker(ui) {
        log::warn!("{e}");
    }

    thread::spawn(move || {
        let config = SpeakerRecorderConfig::new(Arc::new(AtomicBool::new(false)));
        let Ok(recorder) = platform_speaker_recoder(config) else {
            log::warn!("init desktop speaker recorder failed");
            return;
        };

        loop {
            thread::sleep(std::time::Duration::from_secs(5));

            if !config::all().control.enable_speaker {
                continue;
            }

            let device_info = CACHE.lock().unwrap().speaker_device_info.clone();

            if speaker_device_changed(&recorder, &device_info) {
                log::info!("speaker device changed: current speaker deivce info: {device_info:?}");

                _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                    refresh_speaker(&ui, false);
                });
            }
        }
    });
}

fn create_speaker(ui: &AppWindow) -> Result<()> {
    let ui_weak = ui.as_weak();

    thread::spawn(move || {
        let stop_sig = Arc::new(AtomicBool::new(false));
        let (level_sender, level_receiver) = bounded(16);

        let ui_weak_clone = ui_weak.clone();
        thread::spawn(move || {
            while let Ok(db) = level_receiver.recv() {
                // log::debug!("speaker_level_receiver db level: {db:.0}",);

                _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_speaker_audio_db(db as i32);
                });
            }
            log::info!("exit desktop speaker receiver thread");
        });

        let gain = Arc::new(AtomicI32::new(config::all().control.speaker_gain as i32));
        let config = SpeakerRecorderConfig::new(stop_sig.clone())
            .with_level_sender(Some(level_sender))
            .with_gain(Some(gain.clone()));

        match platform_speaker_recoder(config) {
            Err(e) => {
                async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("create desktop speaker recorder failed")),
                );

                return;
            }

            Ok(recorder) => {
                {
                    let mut cache = CACHE.lock().unwrap();
                    cache.speaker_gain = Some(gain);
                    cache.speaker_stop_sig = Some(stop_sig);
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
    let screen_infos = platform_screen_capture().available_screens()?;

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
        if let Err(e) = warmup_video_encoder() {
            log::warn!("Warmup video encoder failed: {e}");
        }
    });

    Ok(())
}

fn toggle_control_enable_stats(ui: &AppWindow) {
    let mut setting = global_store!(ui).get_setting_control();
    setting.enable_stats = !setting.enable_stats;
    global_store!(ui).set_setting_control(setting.clone());
    global_logic!(ui).invoke_set_setting_control(setting);
}

fn toggle_control_enable_preview(ui: &AppWindow) {
    let mut setting = global_store!(ui).get_setting_control();
    setting.enable_preview = !setting.enable_preview;

    if !setting.enable_preview {
        global_store!(ui).set_preview_image(Default::default());
    }

    global_store!(ui).set_setting_control(setting.clone());
    global_logic!(ui).invoke_set_setting_control(setting);
}

fn init_sources_dialog(ui: &AppWindow) {
    if let Err(e) = inner_init_sources_dialog(ui) {
        log::warn!("{e}");
    }
}

fn inner_init_sources_dialog(ui: &AppWindow) -> Result<()> {
    let mut names = vec![];
    let recorder = AudioRecorder::new();

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
    let screen_infos = platform_screen_capture().available_screens()?;
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
        name: setting.audio,
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

fn audio_gain_changed(_ui: &AppWindow, v: f32) {
    let gain = CACHE.lock().unwrap().audio_gain.clone();
    if let Some(gain) = gain {
        gain.store(v as i32, Ordering::Relaxed);
    } else {
        log::warn!("input audio gian is None");
    }
}

fn speaker_gain_changed(_ui: &AppWindow, v: f32) {
    let gain = CACHE.lock().unwrap().speaker_gain.clone();
    if let Some(gain) = gain {
        gain.store(v as i32, Ordering::Relaxed);
    } else {
        log::warn!("speaker gain is None");
    }
}

fn refresh_speaker(ui: &AppWindow, show_toast: bool) {
    {
        if let Some(stop_sig) = CACHE.lock().unwrap().speaker_stop_sig.take() {
            stop_sig.store(true, Ordering::Relaxed);
        }
    }

    if let Err(e) = create_speaker(ui) {
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

fn speaker_device_changed(
    recorder: &impl SpeakerRecorder,
    device_info: &Option<(u32, String)>,
) -> bool {
    let Ok(Some((node_id, node_name))) = recorder.find_default_output() else {
        log::warn!("find default speaker device failed");
        return false;
    };

    if let Some((old_node_id, old_node_name)) = device_info {
        return node_id != *old_node_id || node_name.as_str() != old_node_name.as_str();
    }

    true
}

fn audio_changed(ui: &AppWindow, name: SharedString, show_toast: bool) {
    if let Err(e) = inner_audio_changed(ui, name) {
        if show_toast {
            toast_warn!(ui, format!("{e}"));
        }
    } else {
        if show_toast {
            toast_success!(ui, "change input audio device successfully");
        }
    }
}

fn inner_audio_changed(ui: &AppWindow, name: SharedString) -> Result<()> {
    {
        let mut cache = CACHE.lock().unwrap();
        cache.audio_gain.take();

        if let Some(recorder) = cache.audio_recorder.take() {
            recorder.stop();
        }
    }

    let gain = Arc::new(AtomicI32::new(config::all().control.audio_gain as i32));
    let (level_sender, level_receiver) = bounded(16);

    let mut recorder = AudioRecorder::new()
        .with_gain(Some(gain.clone()))
        .with_level_sender(Some(level_sender));

    recorder.start_recording(name.as_str())?;

    {
        let mut cache = CACHE.lock().unwrap();
        cache.audio_gain = Some(gain);
        cache.audio_recorder = Some(recorder);
    }

    let ui_weak = ui.as_weak();
    thread::spawn(move || {
        while let Ok(db) = level_receiver.recv() {
            // log::debug!("audio_level_receiver db level: {db:.0}",);
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_audio_db(db as i32);
            });
        }
    });

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
    let rt_handle = tokio::runtime::Handle::current();
    let process_mode = global_store!(ui).get_process_mode();

    thread::spawn(move || {
        if let Err(e) = inner_start_recording(rt_handle, ui_weak.clone(), process_mode.into()) {
            toast::async_toast_warn(ui_weak, e.to_string());
        }
    });
}

fn warmup_video_encoder() -> Result<()> {
    let screen_info = current_screen_info()?;
    log::debug!("screen_info: {screen_info:?}");

    let all_config = config::all();
    let resolution = if matches!(all_config.recorder.resolution, UIResolution::Original) {
        Resolution::Original((
            screen_info.logical_size.width as u32,
            screen_info.logical_size.height as u32,
        ))
    } else {
        all_config.recorder.resolution.into()
    };

    RecordingSession::warmup_video_encoder(
        screen_info.logical_size,
        resolution,
        all_config.recorder.fps.into(),
    );

    Ok(())
}

fn inner_start_recording(
    rt_handle: tokio::runtime::Handle,
    ui_weak: Weak<AppWindow>,
    process_mode: ProcessMode,
) -> Result<()> {
    log::info!("start recording...");

    let screen_info = current_screen_info()?;
    log::debug!("screen_info: {screen_info:?}");

    let all_config = config::all();
    let save_mp4 = all_config.share_screen.save_mp4 || all_config.push_stream.save_mp4;
    let async_error_sender = CACHE.lock().unwrap().async_error_sender.clone();

    let resolution = if matches!(all_config.recorder.resolution, UIResolution::Original) {
        Resolution::Original((
            screen_info.logical_size.width as u32,
            screen_info.logical_size.height as u32,
        ))
    } else {
        all_config.recorder.resolution.into()
    };

    let audio_name = if all_config.control.audio.is_empty() || !all_config.control.enable_audio {
        None
    } else {
        Some(all_config.control.audio)
    };

    let config = RecorderConfig::new(
        all_config.control.screen.clone(),
        screen_info.logical_size.clone(),
        RecorderConfig::make_filename(&all_config.recorder.save_dir),
    )
    .with_process_mode(process_mode)
    .with_async_error_sender(async_error_sender)
    .with_include_cursor(all_config.recorder.include_cursor)
    .with_enable_denoise(all_config.recorder.enable_denoise)
    .with_convert_to_mono(all_config.recorder.convert_to_mono)
    .with_enable_recording_speaker(all_config.control.enable_speaker)
    .with_audio_device_name(audio_name)
    .with_audio_gain(Arc::new(AtomicI32::new(
        all_config.control.audio_gain as i32,
    )))
    .with_speaker_gain(Arc::new(AtomicI32::new(
        all_config.control.speaker_gain as i32,
    )))
    .with_fps(all_config.recorder.fps.clone().into())
    .with_resolution(resolution)
    .with_enable_cursor_tracking(all_config.cursor_tracker.enable_tracking)
    .with_region_width(all_config.cursor_tracker.region_width)
    .with_region_height(all_config.cursor_tracker.region_height)
    .with_debounce_radius(all_config.cursor_tracker.debounce_radius as u32)
    .with_stable_radius(all_config.cursor_tracker.stable_radius as u32)
    .with_fast_moving_duration(all_config.cursor_tracker.fast_moving_duration as u64)
    .with_zoom_transition_duration(all_config.cursor_tracker.zoom_transition_duration as u64)
    .with_reposition_edge_threshold(all_config.cursor_tracker.reposition_edge_threshold)
    .with_reposition_transition_duration(
        all_config.cursor_tracker.reposition_transition_duration as u64,
    )
    .with_zoom_in_transition_type(all_config.cursor_tracker.zoom_in_transition_type.into())
    .with_zoom_out_transition_type(all_config.cursor_tracker.zoom_out_transition_type.into())
    .with_max_stable_region_duration(all_config.cursor_tracker.max_stable_region_duration as u64)
    .with_share_screen_config(all_config.share_screen.into())
    .with_push_stream_config(all_config.push_stream.into());

    log::info!("Recording configuration: {:#?}", config);

    let (frame_sender_user, frame_receiver_user) = bounded(16);
    let mut session = RecordingSession::new(config).with_frame_sender_user(Some(frame_sender_user));
    session.start(rt_handle, platform_screen_capture())?;

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_start_recording_timer(false);
        global_store!(ui).set_final_video_path(SharedString::default());
        global_store!(ui).set_record_status(UIRecordStatus::Recording);
    });

    let stop_sig = session.get_stop_sig().clone();
    {
        let mut cache = CACHE.lock().unwrap();
        cache.recorder_stop_sig = Some(stop_sig);
    }

    let ui_weak_clone = ui_weak.clone();
    thread::spawn(move || {
        while let Ok(frame) = frame_receiver_user.recv() {
            log::debug!(
                "frame_receiver_user buffer len: {} bytes",
                frame.buffer.len()
            );

            _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                if global_store!(ui).get_setting_control().enable_preview {
                    let buffer = SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                        &frame.buffer.as_raw(),
                        frame.buffer.width(),
                        frame.buffer.height(),
                    );
                    let img = slint::Image::from_rgb8(buffer);
                    global_store!(ui).set_preview_image(img);
                } else {
                    global_store!(ui).set_preview_image(Default::default());
                }

                global_store!(ui).set_start_recording_timer(true);

                let mut sinfo = global_store!(ui).get_stats_info();
                sinfo.fps = frame.stats.fps;
                sinfo.total = frame.stats.total_frames as i32;
                sinfo.loss =
                    frame.stats.loss_frames as f32 / frame.stats.total_frames.max(1) as f32;
                sinfo.share_screen_connections = frame.stats.share_screen_connections as i32;
                global_store!(ui).set_stats_info(sinfo);
            });
        }
        log::info!("exit frame_receiver_user");
    });

    let final_video_path = session.save_path();

    session.wait()?;

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_start_recording_timer(false);
        global_store!(ui).set_record_status(UIRecordStatus::Stopped);

        if matches!(process_mode, ProcessMode::RecordScreen)
            || (matches!(process_mode, ProcessMode::ShareScreen)
                || matches!(process_mode, ProcessMode::PushStream) && save_mp4)
        {
            global_store!(ui).set_final_video_path(final_video_path.display().to_shared_string());
            global_logic!(ui).invoke_add_history(final_video_path.display().to_shared_string());
        }
    });

    log::info!("Recording completed successfully!");

    Ok(())
}

pub fn get_async_error_sender() -> Option<AsyncErrorSender> {
    CACHE.lock().unwrap().async_error_sender.clone()
}

fn show_async_error_task(ui_weak: Weak<AppWindow>, mut receiver: AsyncErrorReceiver) {
    tokio::spawn(async move {
        while let Some(err) = receiver.recv().await {
            // NOTE: maybe need to redesigned `err` type
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_record_status(UIRecordStatus::Stopped);
                toast_warn!(ui, err);
            });
        }

        log::info!("async_error_task exit...");
    });
}

fn stop_recording(ui: &AppWindow) {
    let stop_sig = CACHE.lock().unwrap().recorder_stop_sig.take();
    if let Some(sig) = stop_sig {
        sig.store(true, Ordering::Relaxed);
    } else {
        log::warn!("recorder_stop_sig is None");
    }

    global_store!(ui).set_record_status(UIRecordStatus::Stopped);
}

fn current_screen_info() -> Result<ScreenInfo> {
    let all_config = config::all();

    if all_config.control.screen.is_empty() {
        bail!("available screen no found");
    }

    let mut capture = platform_screen_capture();
    let screen_info = capture
        .available_screens()?
        .into_iter()
        .find(|item| item.name == all_config.control.screen);

    if screen_info.is_none() {
        bail!("no found screen: {}", all_config.control.screen);
    }

    Ok(screen_info.unwrap())
}

fn cal_region_width(_ui: &AppWindow, height: f32) -> i32 {
    match current_screen_info() {
        Ok(screen_info) => {
            if screen_info.logical_size.height <= 0 {
                log::warn!("{} height is zero", screen_info.name);
                0
            } else {
                (height as f32 * screen_info.logical_size.width as f32
                    / screen_info.logical_size.height as f32) as i32
            }
        }
        Err(e) => {
            log::warn!("{e}");
            0
        }
    }
}

fn cal_region_height(_ui: &AppWindow, width: f32) -> i32 {
    match current_screen_info() {
        Ok(screen_info) => {
            if screen_info.logical_size.width <= 0 {
                log::warn!("{} width is zero", screen_info.name);
                0
            } else {
                (width as f32 * screen_info.logical_size.height as f32
                    / screen_info.logical_size.width as f32) as i32
            }
        }
        Err(e) => {
            log::warn!("{e}");
            0
        }
    }
}

fn open_file(ui: &AppWindow, file: SharedString) {
    if !file.is_empty()
        && let Err(e) = open::that_detached(file.as_str())
    {
        toast_warn!(ui, format!("{}: `{}`. {e}", tr("Open file failed"), file));
    }
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
