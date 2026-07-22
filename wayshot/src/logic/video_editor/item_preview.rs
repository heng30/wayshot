use crate::{
    global_logic, global_store,
    logic::{toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, MediaType as UIMediaType, VideoEditorSubtitle as UIVideoEditorSubtitle,
    },
};
use audio_utils::{
    audio::apply_fade_in,
    loader::{AudioConfig, load_audio_file},
};
use rodio::{
    ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer,
};
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, VecModel, Weak};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};
use video_editor::{
    metadata::get_metadata,
    preview::{PreviewConfig, PreviewRenderer},
    tracks::segment::Segment,
    tracks::subtitle_track::extract_subtitles,
    tracks::{manager::Manager, track::Track, unified_mixer::UnifiedMixerConfig},
};
use video_utils::subtitle::{ms_to_srt_timestamp, parse_lrc_file};

static ITEM_PREVIEW_PLAYER: Mutex<Option<ItemPreviewPlayer>> = Mutex::new(None);
static ITEM_PREVIEW_AUDIO_STATE: Mutex<Option<ItemPreviewAudioState>> = Mutex::new(None);
static ITEM_PREVIEW_VIDEO_STATE: Mutex<Option<ItemPreviewVideoState>> = Mutex::new(None);
static ITEM_PREVIEW_VIDEO_RENDERER: Mutex<Option<PreviewRenderer>> = Mutex::new(None);
static ITEM_PREVIEW_VIDEO_THREAD_ID: AtomicU32 = AtomicU32::new(0);

struct ItemPreviewPlayer {
    audio_thread: Option<tokio::task::JoinHandle<()>>,
    stop_sig: Arc<AtomicBool>,
}

struct ItemPreviewAudioState {
    inc_index: u64,
    audio_sink: Option<Arc<Player>>,
    #[allow(unused)]
    audio_stream: Option<Arc<MixerDeviceSink>>,
    audio_config: Option<AudioConfig>,
}

struct ItemPreviewVideoState {
    inc_index: u64,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_item_preview_play, ui);
    logic_cb!(video_editor_item_preview_stop, ui);
    logic_cb!(video_editor_item_preview_seek, ui, position);
    logic_cb!(video_editor_item_preview_progress_changed, ui, value);
    logic_cb!(video_editor_item_preview_progress_released, ui, value);
    logic_cb!(video_editor_item_preview_progress_pressed, ui, value);
    logic_cb!(video_editor_item_preview_sound_changed, ui, sound);
    logic_cb!(video_editor_item_preview_sound_released, ui, sound);
}

fn inner_init(ui: &AppWindow) {
    match DeviceSinkBuilder::open_default_sink() {
        Ok(device_sink) => {
            let ui_state = global_store!(ui).get_video_editor_ui_state();
            let volume = ui_state.preview_volume.clamp(0.0, 100.0) / 100.0;

            let player = Player::connect_new(&device_sink.mixer());
            player.set_volume(volume);

            *ITEM_PREVIEW_AUDIO_STATE.lock().unwrap() = Some(ItemPreviewAudioState {
                inc_index: 0,
                audio_sink: Some(Arc::new(player)),
                audio_stream: Some(Arc::new(device_sink)),
                audio_config: None,
            });
        }
        Err(e) => crate::toast_warn!(
            ui,
            format!("{}: {e}", tr("Failed to create audio output stream"))
        ),
    }
}

pub fn show_preview_item(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let item = global_store!(ui).get_video_editor_preview_item();
    let path = PathBuf::from(item.file_path.to_string());

    if !path.exists() {
        return;
    }

    if path
        .extension()
        .is_some_and(|e| e.to_ascii_lowercase() == "lrc")
    {
        load_lrc_preview(ui_weak, path);
        return;
    }

    match item.media_type {
        UIMediaType::Subtitle => load_subtitle_preview(ui_weak, path),
        UIMediaType::Image => load_image_preview(ui_weak, path),
        UIMediaType::Video => load_video_preview(ui_weak, path),
        UIMediaType::Audio => load_audio_preview(ui_weak, path),
        _ => {}
    }
}

fn load_subtitle_preview(ui_weak: Weak<AppWindow>, path: PathBuf) {
    tokio::task::spawn_blocking(move || {
        let metadata = match get_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to load subtitle"), e),
                );
                return;
            }
        };

        let subtitle_meta = match metadata.subtitles.first() {
            Some(m) => m,
            None => {
                toast::async_toast_warn(ui_weak.clone(), tr("No subtitle stream found"));
                return;
            }
        };

        let entries = match extract_subtitles(&path, subtitle_meta.index) {
            Ok(e) => e,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to parse subtitle"), e),
                );
                return;
            }
        };

        let ui_entries: Vec<UIVideoEditorSubtitle> = entries
            .iter()
            .map(|e| UIVideoEditorSubtitle {
                start_timestamp: ms_to_srt_timestamp(e.start.as_millis() as u64).into(),
                end_timestamp: ms_to_srt_timestamp(e.end.as_millis() as u64).into(),
                subtitle: e.text.clone().into(),
            })
            .collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_item_preview_subtitle_entries(ModelRc::new(
                VecModel::from_slice(&ui_entries),
            ));
        });
    });
}

fn load_lrc_preview(ui_weak: Weak<AppWindow>, path: PathBuf) {
    tokio::task::spawn_blocking(move || {
        let entries = match parse_lrc_file(&path) {
            Ok(e) => e,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to parse lyrics"), e),
                );
                return;
            }
        };

        let ui_entries: Vec<UIVideoEditorSubtitle> = entries
            .iter()
            .map(|e| UIVideoEditorSubtitle {
                start_timestamp: ms_to_srt_timestamp(e.start_timestamp).into(),
                end_timestamp: ms_to_srt_timestamp(e.end_timestamp).into(),
                subtitle: e.text.clone().into(),
            })
            .collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_item_preview_subtitle_entries(ModelRc::new(
                VecModel::from_slice(&ui_entries),
            ));
        });
    });
}

fn load_image_preview(ui_weak: Weak<AppWindow>, path: PathBuf) {
    tokio::task::spawn_blocking(move || match image::open(&path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            let raw_bytes = rgba.as_raw().clone();

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &raw_bytes, width, height,
                );
                let slint_image = slint::Image::from_rgba8(buffer);
                global_store!(ui).set_video_editor_item_preview_image(slint_image);
            });
        }
        Err(e) => {
            toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Failed to load image"), e))
        }
    });
}

fn load_video_preview(ui_weak: Weak<AppWindow>, path: PathBuf) {
    tokio::task::spawn_blocking(move || {
        let metadata = match get_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to load video metadata"), e),
                );
                return;
            }
        };

        let segment = Segment::new(Duration::ZERO, metadata.duration, Arc::new(metadata), 1.0);

        let first_frame = match segment.first_frame_image() {
            Ok(frame) => frame,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("Failed to extract first frame: {}", e),
                );
                return;
            }
        };

        let (width, height) = first_frame.dimensions();
        let raw_bytes = first_frame.as_raw().clone();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let buffer =
                SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw_bytes, width, height);
            let img = slint::Image::from_rgba8(buffer);
            global_store!(ui).set_video_editor_item_preview_image(img);
        });
    });
}

fn load_audio_preview(ui_weak: Weak<AppWindow>, path: PathBuf) {
    // Increment inc_index to invalidate any in-progress load
    let my_index = {
        let mut audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref mut state) = *audio_state {
            state.inc_index += 1;
            state.inc_index
        } else {
            return;
        }
    };

    // Stop any existing playback first
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        stop_item_preview(&ui);
    });

    tokio::task::spawn_blocking(move || {
        let audio_config = match load_audio_file(&path) {
            Ok(c) => c,
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("Failed to load audio: {e}"));
                return;
            }
        };

        // Check if a newer preview request has been made
        {
            let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
            if let Some(ref state) = *audio_state {
                if state.inc_index != my_index {
                    return; // Stale request, discard
                }
            } else {
                return;
            }
        }

        let total_duration_ms = audio_config.duration.as_millis() as u64;
        let end_time_str =
            cutil::time::seconds_to_media_timestamp(total_duration_ms as f64 / 1000.0).into();

        {
            let mut audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
            if let Some(ref mut state) = *audio_state {
                state.audio_config = Some(audio_config.clone());
            }
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            {
                let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
                if let Some(ref state) = *audio_state {
                    if state.inc_index != my_index {
                        return; // Stale request, discard
                    }
                } else {
                    return;
                }
            }

            global_store!(ui).set_video_editor_item_preview_current_time("00:00".into());
            global_store!(ui).set_video_editor_item_preview_end_time(end_time_str);
            global_store!(ui).set_video_editor_item_preview_progress(0.0);
            global_store!(ui).set_video_editor_is_show_item_preview_dialog(true);
            global_store!(ui).set_video_editor_item_preview_is_playing(true);
            global_logic!(ui).invoke_video_editor_item_preview_play();
        });
    });
}

fn video_editor_item_preview_play(ui: &AppWindow) {
    let current_progress = global_store!(ui).get_video_editor_item_preview_progress();
    let progress = if current_progress >= 0.99 {
        0.0
    } else {
        current_progress
    };

    let item = global_store!(ui).get_video_editor_preview_item();
    let path = PathBuf::from(item.file_path.to_string());
    if !path.exists() {
        return;
    }

    stop_item_preview(ui);

    global_store!(ui).set_video_editor_item_preview_is_playing(true);
    let volume = global_store!(ui)
        .get_video_editor_ui_state()
        .preview_volume
        .clamp(0.0, 100.0);
    video_editor_item_preview_sound_changed(ui, volume as i32);

    match item.media_type {
        UIMediaType::Audio => play_audio_file(ui.as_weak(), path, progress),
        UIMediaType::Video => play_video_file(ui.as_weak(), path, progress),
        _ => global_store!(ui).set_video_editor_item_preview_is_playing(false),
    }
}

fn video_editor_item_preview_stop(ui: &AppWindow) {
    global_store!(ui).set_video_editor_item_preview_is_playing(false);

    {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state
            && let Some(ref sink) = state.audio_sink
        {
            sink.clear();
            sink.pause();
        }
    }

    {
        ITEM_PREVIEW_VIDEO_THREAD_ID.fetch_add(1, Ordering::SeqCst);
        let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
        if let Some(ref mut renderer) = *guard {
            renderer.pause();
        }
    }
}

fn video_editor_item_preview_progress_changed(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_item_preview_progress(value);
}

fn video_editor_item_preview_progress_pressed(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_item_preview_is_playing(false);
    global_store!(ui).set_video_editor_item_preview_progress(value);

    {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state
            && let Some(ref sink) = state.audio_sink
        {
            sink.clear();
        }
    }

    {
        ITEM_PREVIEW_VIDEO_THREAD_ID.fetch_add(1, Ordering::SeqCst);
        let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
        if let Some(ref mut renderer) = *guard {
            renderer.pause();
        }
    }
}

fn video_editor_item_preview_progress_released(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_item_preview_progress(value);

    let item = global_store!(ui).get_video_editor_preview_item();
    match item.media_type {
        UIMediaType::Audio => seek_audio(ui, value),
        UIMediaType::Video => seek_video(ui, value),
        _ => {}
    }
}

fn video_editor_item_preview_sound_changed(_ui: &AppWindow, sound: i32) {
    let volume = (sound.clamp(0, 100) as f32) / 100.0;

    {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state
            && let Some(ref sink) = state.audio_sink
        {
            sink.set_volume(volume);
        }
    }

    {
        let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
        if let Some(ref mut renderer) = *guard {
            renderer.set_volume(volume);
        }
    }
}

fn video_editor_item_preview_sound_released(ui: &AppWindow, sound: i32) {
    let volume = (sound.clamp(0, 100) as f32) / 100.0;

    {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state
            && let Some(ref sink) = state.audio_sink
        {
            sink.set_volume(volume);
        }
    }

    {
        let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
        if let Some(ref mut renderer) = *guard {
            renderer.set_volume(volume);
        }
    }

    let mut ui_state = global_store!(ui).get_video_editor_ui_state();
    ui_state.preview_volume = sound.clamp(0, 100) as f32;
    global_store!(ui).set_video_editor_ui_state(ui_state);
    global_logic!(ui).invoke_video_editor_update_ui_state();
}

fn video_editor_item_preview_seek(ui: &AppWindow, position: f32) {
    stop_item_preview(ui);
    video_editor_item_preview_progress_released(ui, position);
}

fn stop_item_preview(ui: &AppWindow) {
    global_store!(ui).set_video_editor_item_preview_is_playing(false);

    {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state
            && let Some(ref sink) = state.audio_sink
        {
            sink.clear();
            sink.pause();
        }
    }

    {
        let mut guard = ITEM_PREVIEW_PLAYER.lock().unwrap();
        if let Some(ref mut player) = *guard {
            player.stop_sig.store(true, Ordering::Relaxed);
            _ = player.audio_thread.take();
        }
        *guard = None;
    }

    {
        ITEM_PREVIEW_VIDEO_THREAD_ID.fetch_add(1, Ordering::SeqCst);
        let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
        if let Some(ref mut renderer) = *guard {
            renderer.stop();
        }
    }
}

fn play_audio_file(ui_weak: Weak<AppWindow>, path: PathBuf, progress: f32) {
    let stop_sig = Arc::new(AtomicBool::new(false));
    let player: Arc<Player> = {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state {
            match state.audio_sink.clone() {
                Some(p) => p,
                None => return,
            }
        } else {
            return;
        }
    };

    // Check if audio_config is already loaded (e.g. by load_audio_preview)
    let cached_config = {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        audio_state.as_ref().and_then(|s| s.audio_config.clone())
    };

    let runtime_handle = tokio::runtime::Handle::current();
    let handle = tokio::task::spawn_blocking(move || {
        let audio_config = if let Some(config) = cached_config {
            config
        } else {
            match load_audio_file(&path) {
                Ok(config) => config,
                Err(e) => {
                    toast::async_toast_warn(
                        ui_weak,
                        format!("{}: {e}", tr("Failed to load audio file")),
                    );
                    return;
                }
            }
        };

        let mut my_inc_index = 0u64;
        {
            let mut audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
            if let Some(ref mut state) = *audio_state {
                state.audio_config = Some(audio_config.clone());
                state.inc_index += 1;
                my_inc_index = state.inc_index;
            }
        }

        let total_duration_ms = audio_config.duration.as_millis() as u64;
        let end_time_str =
            cutil::time::seconds_to_media_timestamp(total_duration_ms as f64 / 1000.0).into();

        let start_ms = if progress > 0.0 {
            (total_duration_ms as f64 * progress.clamp(0.0, 1.0) as f64) as u64
        } else {
            0
        };

        if let Err(e) = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui)
                .set_video_editor_item_preview_current_time("00:00".to_string().into());
            global_store!(ui).set_video_editor_item_preview_end_time(end_time_str);
        }) {
            log::warn!("Failed to update UI duration state: {:?}", e);
        }

        if let Err(e) = play_audio_segment(
            ui_weak.clone(),
            runtime_handle,
            &audio_config.samples,
            audio_config.channel,
            audio_config.sample_rate,
            start_ms,
            total_duration_ms,
            player.clone(),
        ) {
            log::warn!("Failed to play audio: {}", e);
        }

        on_playback_finished(ui_weak, my_inc_index);
    });

    {
        let mut guard = ITEM_PREVIEW_PLAYER.lock().unwrap();
        *guard = Some(ItemPreviewPlayer {
            audio_thread: Some(handle),
            stop_sig,
        });
    }
}

fn play_audio_segment(
    ui_weak: Weak<AppWindow>,
    runtime_handle: tokio::runtime::Handle,
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    start_ms: u64,
    total_duration_ms: u64,
    player: Arc<Player>,
) -> anyhow::Result<()> {
    player.clear();
    player.stop();

    let mut segment_samples =
        extract_audio_samples(samples, channels, sample_rate, start_ms, total_duration_ms);

    if segment_samples.is_empty() {
        return Ok(());
    }

    apply_fade_in(&mut segment_samples, channels, sample_rate, 200);

    let source = SamplesBuffer::new(
        ChannelCount::new(channels).unwrap(),
        SampleRate::new(sample_rate).unwrap(),
        segment_samples,
    );

    player.append(source);

    let progress = (start_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0);
    let current_time_str = cutil::time::seconds_to_media_timestamp(start_ms as f64 / 1000.0).into();

    if let Err(e) = ui_weak.clone().upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_video_editor_item_preview_progress(progress as f32);
        global_store!(ui).set_video_editor_item_preview_current_time(current_time_str);
    }) {
        log::warn!("Failed to set initial progress state: {:?}", e);
    }

    let inc_index = {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state {
            state.inc_index
        } else {
            0
        }
    };

    start_progress_updates(
        ui_weak.clone(),
        runtime_handle.clone(),
        start_ms,
        total_duration_ms,
        inc_index,
        true,
        Some(player.clone()),
    );

    let current_volume = player.volume();
    player.set_volume(current_volume);
    player.play();
    player.sleep_until_end();
    player.pause();

    Ok(())
}

fn extract_audio_samples(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    start_ms: u64,
    end_ms: u64,
) -> Vec<f32> {
    let start_sample = ((sample_rate as u64 * start_ms * channels as u64) / 1000) as usize;
    let end_sample =
        ((sample_rate as u64 * end_ms * channels as u64) / 1000).min(samples.len() as u64) as usize;

    if start_sample >= end_sample {
        return vec![];
    }

    samples[start_sample..end_sample].to_vec()
}

fn seek_audio(ui: &AppWindow, value: f32) {
    let (sink, audio_config) = {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        if let Some(ref state) = *audio_state {
            (state.audio_sink.clone(), state.audio_config.clone())
        } else {
            return;
        }
    };

    if let (Some(sink), Some(config)) = (sink, audio_config) {
        let my_inc_index = {
            let mut audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
            if let Some(ref mut state) = *audio_state {
                state.inc_index += 1;
                state.inc_index
            } else {
                return;
            }
        };

        let ui_weak = ui.as_weak();
        let total_duration_ms = config.duration.as_millis() as u64;
        let start_ms = (total_duration_ms as f64 * value.clamp(0.0, 1.0) as f64) as u64;
        let runtime_handle = tokio::runtime::Handle::current();

        global_store!(ui).set_video_editor_item_preview_is_playing(true);

        tokio::task::spawn_blocking(move || {
            if let Err(e) = play_audio_segment(
                ui_weak.clone(),
                runtime_handle,
                &config.samples,
                config.channel,
                config.sample_rate,
                start_ms,
                total_duration_ms,
                sink.clone(),
            ) {
                log::warn!("Failed to play audio segment: {}", e);
            }

            on_playback_finished(ui_weak, my_inc_index);
        });
    }
}

fn play_video_file(ui_weak: Weak<AppWindow>, path: PathBuf, value: f32) {
    let thread_id = ITEM_PREVIEW_VIDEO_THREAD_ID.fetch_add(1, Ordering::SeqCst) + 1;

    let volume = {
        let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
        audio_state
            .as_ref()
            .and_then(|s| s.audio_sink.as_ref().map(|sink| sink.volume()))
            .unwrap_or(0.5)
    };

    tokio::task::spawn_blocking(move || {
        let tracks = match Track::new(&path, 1.0) {
            Ok(t) => t,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("Failed to load video file")),
                );
                return;
            }
        };

        let mut manager = Manager::new();
        for track in tracks {
            manager.add_track(track);
        }

        let mixer_config = UnifiedMixerConfig::default()
            .with_output_height(Some(720))
            .with_output_width(Some(1280))
            .with_output_fps(Some(25.0));

        let mut renderer =
            PreviewRenderer::new(Arc::new(manager), PreviewConfig::from(mixer_config));

        renderer.set_volume(volume);

        let total_duration_ms = renderer.duration().as_millis() as i32;
        let seek_position =
            Duration::from_millis((total_duration_ms as f64 * value.clamp(0.0, 1.0) as f64) as u64);

        if let Err(e) = renderer.seek(seek_position) {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {:?}", tr("Failed to seek to position"), e),
            );
            return;
        }

        if let Err(e) = renderer.play() {
            toast::async_toast_warn(
                ui_weak,
                format!("{}: {:?}", tr("Failed to start video playback"), e),
            );
            return;
        }
        let position_ms =
            Duration::from_millis((total_duration_ms as f64 * value.clamp(0.0, 1.0) as f64) as u64)
                .as_millis();
        let progress = if total_duration_ms > 0 {
            (position_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let current_time_str =
            cutil::time::seconds_to_media_timestamp(position_ms as f64 / 1000.0).into();
        let end_time_str =
            cutil::time::seconds_to_media_timestamp(total_duration_ms as f64 / 1000.0).into();

        if let Err(e) = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_item_preview_progress(progress);
            global_store!(ui).set_video_editor_item_preview_current_time(current_time_str);
            global_store!(ui).set_video_editor_item_preview_end_time(end_time_str);
        }) {
            log::warn!("Failed to set initial progress state: {:?}", e);
        }

        {
            let mut video_state = ITEM_PREVIEW_VIDEO_STATE.lock().unwrap();
            if let Some(ref mut state) = *video_state {
                state.inc_index += 1;
            } else {
                *video_state = Some(ItemPreviewVideoState { inc_index: 1 });
            }
        }

        *ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap() = Some(renderer);
        start_video_playback_thread(ui_weak, thread_id);
    });
}

fn seek_video(ui: &AppWindow, value: f32) {
    let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
    if let Some(ref mut renderer) = *guard {
        let total_duration_ms = renderer.duration().as_millis() as u64;
        let seek_position =
            Duration::from_millis((total_duration_ms as f64 * value.clamp(0.0, 1.0) as f64) as u64);

        if let Err(e) = renderer.seek(seek_position) {
            log::warn!("Failed to seek video: {:?}", e);
            return;
        }

        if let Err(e) = renderer.play() {
            log::warn!("Failed to resume video playback: {:?}", e);
        }

        {
            let mut state = ITEM_PREVIEW_VIDEO_STATE.lock().unwrap();
            if let Some(ref mut state) = *state {
                state.inc_index += 1;
            }
        }

        let thread_id = ITEM_PREVIEW_VIDEO_THREAD_ID.fetch_add(1, Ordering::SeqCst) + 1;
        start_video_playback_thread(ui.as_weak(), thread_id);
        global_store!(ui).set_video_editor_item_preview_is_playing(true);
    } else {
        let item = global_store!(ui).get_video_editor_preview_item();
        let path = PathBuf::from(item.file_path.to_string());
        if !path.exists() {
            return;
        }

        global_store!(ui).set_video_editor_item_preview_is_playing(true);
        play_video_file(ui.as_weak(), path, value);
    }
}

fn start_video_playback_thread(ui_weak: Weak<AppWindow>, thread_id: u32) {
    tokio::task::spawn_blocking(move || {
        let frame_rate = {
            let guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
            guard
                .as_ref()
                .map(|r| r.frame_rate() as f32)
                .unwrap_or(25.0)
        };

        let mut frame_index: u64 = 0;
        let start_time = Instant::now();
        let sleeper = spin_sleep::SpinSleeper::default();
        let frame_duration = Duration::from_secs_f64(1.0 / frame_rate as f64);

        loop {
            if ITEM_PREVIEW_VIDEO_THREAD_ID.load(Ordering::SeqCst) != thread_id {
                break;
            }

            let (is_playing, position, frame_data) = {
                let mut guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
                if let Some(ref mut renderer) = *guard {
                    if let Err(e) = renderer.update() {
                        log::warn!("[item_preview] renderer.update() error: {e:?}");
                    }

                    (
                        renderer.is_playing(),
                        renderer.position(),
                        renderer.current_frame().map(|f| f.clone()),
                    )
                } else {
                    break;
                }
            };

            if !is_playing {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_item_preview_is_playing(false);
                });

                break;
            }

            if let Some(frame) = frame_data {
                let (width, height) = frame.dimensions();
                let raw_bytes = frame.as_raw().clone();
                let total_duration_ms = {
                    let guard = ITEM_PREVIEW_VIDEO_RENDERER.lock().unwrap();
                    guard
                        .as_ref()
                        .map(|r| r.duration().as_millis() as u64)
                        .unwrap_or(0)
                };
                let position_ms = position.as_millis() as u64;
                let progress = if total_duration_ms > 0 {
                    (position_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                let current_time_str =
                    cutil::time::seconds_to_media_timestamp(position_ms as f64 / 1000.0).into();

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        &raw_bytes, width, height,
                    );
                    let img = slint::Image::from_rgba8(buffer);
                    global_store!(ui).set_video_editor_item_preview_image(img);
                    global_store!(ui).set_video_editor_item_preview_progress(progress);
                    global_store!(ui).set_video_editor_item_preview_current_time(current_time_str);
                });
            }

            frame_index += 1;
            let target_time = start_time + frame_duration * frame_index as u32;
            sleeper.sleep_until(target_time);
        }
    });
}

fn on_playback_finished(ui_weak: Weak<AppWindow>, inc_index: u64) {
    if let Err(e) = ui_weak.upgrade_in_event_loop(move |ui| {
        let is_current = {
            let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
            audio_state
                .as_ref()
                .map_or(false, |s| s.inc_index == inc_index)
        };

        if is_current {
            global_store!(ui).set_video_editor_item_preview_is_playing(false);
        }
    }) {
        log::warn!("Failed to update UI state after playback: {:?}", e);
    }
}

fn start_progress_updates(
    ui_weak: Weak<AppWindow>,
    runtime_handle: tokio::runtime::Handle,
    start_ms: u64,
    total_duration_ms: u64,
    inc_index: u64,
    is_audio: bool,
    player: Option<Arc<Player>>,
) -> tokio::task::JoinHandle<()> {
    runtime_handle.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(20));
        let start_time = std::time::Instant::now();

        loop {
            interval.tick().await;

            let current_inc_index = if is_audio {
                let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
                if let Some(ref state) = *audio_state {
                    state.inc_index
                } else {
                    break;
                }
            } else {
                let video_state = ITEM_PREVIEW_VIDEO_STATE.lock().unwrap();
                if let Some(ref state) = *video_state {
                    state.inc_index
                } else {
                    break;
                }
            };

            if current_inc_index != inc_index {
                break;
            }

            let elapsed_ms = start_ms + start_time.elapsed().as_millis() as u64;
            let progress = (elapsed_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0);
            let current_time_str =
                cutil::time::seconds_to_media_timestamp(elapsed_ms as f64 / 1000.0).into();

            if let Err(e) = ui_weak.upgrade_in_event_loop(move |ui| {
                if is_audio {
                    let audio_state = ITEM_PREVIEW_AUDIO_STATE.lock().unwrap();
                    if let Some(ref state) = *audio_state
                        && state.inc_index == inc_index
                    {
                        global_store!(ui).set_video_editor_item_preview_progress(progress as f32);
                        global_store!(ui)
                            .set_video_editor_item_preview_current_time(current_time_str);
                    }
                } else {
                    let video_state = ITEM_PREVIEW_VIDEO_STATE.lock().unwrap();
                    if let Some(ref state) = *video_state
                        && state.inc_index == inc_index
                    {
                        global_store!(ui).set_video_editor_item_preview_progress(progress as f32);
                        global_store!(ui)
                            .set_video_editor_item_preview_current_time(current_time_str);
                    }
                }
            }) {
                log::warn!("Failed to update progress: {:?}", e);
            }

            if is_audio
                && let Some(ref p) = player
                && (p.is_paused() || p.empty())
            {
                break;
            }
        }
    })
}
