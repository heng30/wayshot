use super::model::mark_overlapped_timestamp;
use crate::{
    db::TranscribeConfigData,
    global_logic, global_store,
    logic::{toast, tr::tr, video_editor::transcribe::save_transcribe_config},
    logic_cb,
    slint_generatedAppWindow::{AppWindow, Subtitle as UISubtitle},
    store_video_editor_transcribe_subtitles, toast_warn,
};
use audio_utils::{
    audio::{apply_fade_in, downsample_audio, max_sound_wave_amplitude},
    loader::AudioConfig,
};
use once_cell::sync::Lazy;
use rodio::{
    ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::sync::{Arc, Mutex};
use video_utils::subtitle::{ms_to_srt_timestamp, srt_timestamp_to_ms};

pub const MAX_WAVE_FORM_SAMPLE_COUNTS: i32 = 200;
static CURRENT_AUDIO_PLAYER: Lazy<Mutex<CurrentAudioPlayer>> =
    Lazy::new(|| Mutex::new(CurrentAudioPlayer::default()));

#[derive(Default)]
struct CurrentAudioPlayer {
    audio_config: Option<AudioConfig>,
    audio_sink: Option<Arc<Player>>,
    audio_stream: Option<Arc<MixerDeviceSink>>,
    inc_index: u64,
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(
        video_editor_transcribe_audio_player_sound_changed,
        ui,
        sound
    );
    logic_cb!(
        video_editor_transcribe_audio_player_sound_released,
        ui,
        sound
    );
    logic_cb!(
        video_editor_transcribe_audio_player_speed_changed,
        ui,
        speed
    );
    logic_cb!(
        video_editor_transcribe_audio_player_progress_changed,
        ui,
        value
    );
    logic_cb!(
        video_editor_transcribe_audio_player_progress_released,
        ui,
        value
    );
    logic_cb!(
        video_editor_transcribe_audio_player_progress_pressed,
        ui,
        value
    );

    logic_cb!(
        video_editor_transcribe_play_audio,
        ui,
        start_timestamp,
        end_timestamp
    );
    logic_cb!(video_editor_transcribe_stop_audio, ui);

    logic_cb!(
        video_editor_transcribe_sound_wave_update,
        ui,
        index,
        max_samples
    );
    logic_cb!(
        video_editor_transcribe_sound_wave_zoom_changed,
        ui,
        index,
        level
    );
    logic_cb!(video_editor_transcribe_sound_wave_moved, ui, index, percent);
    logic_cb!(
        video_editor_transcribe_sound_wave_start_position_changed,
        ui,
        index,
        pos
    );
    logic_cb!(
        video_editor_transcribe_sound_wave_end_position_changed,
        ui,
        index,
        pos
    );
}

fn inner_init(ui: &AppWindow) {
    match DeviceSinkBuilder::open_default_sink() {
        Ok(device_sink) => {
            let setting = global_store!(ui).get_video_editor_transcribe_setting();
            let sound = setting.audio_sound.clamp(0.0, 1.0);
            let speed = setting.audio_speed.max(0.5);

            let player = Player::connect_new(&device_sink.mixer());
            player.set_volume(sound);
            player.set_speed(speed);

            let mut audio_player = CURRENT_AUDIO_PLAYER.lock().unwrap();
            audio_player.audio_sink = Some(Arc::new(player));
            audio_player.audio_stream = Some(Arc::new(device_sink));
        }
        Err(e) => toast_warn!(
            ui,
            format!("{}: {e}", tr("Failed to create audio output stream"))
        ),
    }
}

pub fn get_current_audio_config() -> Option<AudioConfig> {
    CURRENT_AUDIO_PLAYER.lock().unwrap().audio_config.clone()
}

pub fn set_current_audio_config(config: Option<AudioConfig>) {
    CURRENT_AUDIO_PLAYER.lock().unwrap().audio_config = config;
}

fn video_editor_transcribe_audio_player_sound_changed(_ui: &AppWindow, sound: f32) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.set_volume(sound.clamp(0.0, 1.0));
    }
}

fn video_editor_transcribe_audio_player_sound_released(ui: &AppWindow, sound: f32) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.set_volume(sound.clamp(0.0, 1.0));
    }

    let mut setting = global_store!(ui).get_video_editor_transcribe_setting();
    setting.audio_sound = sound;
    global_store!(ui).set_video_editor_transcribe_setting(setting.clone());
    save_transcribe_config(TranscribeConfigData::from(setting));
}

fn video_editor_transcribe_audio_player_speed_changed(ui: &AppWindow, speed: f32) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.set_speed(speed);
    }

    let mut setting = global_store!(ui).get_video_editor_transcribe_setting();
    setting.audio_speed = speed;
    global_store!(ui).set_video_editor_transcribe_setting(setting.clone());
    save_transcribe_config(TranscribeConfigData::from(setting));
}

fn video_editor_transcribe_audio_player_progress_changed(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_transcribe_audio_player_progress(value);
}

fn video_editor_transcribe_audio_player_progress_released(ui: &AppWindow, value: f32) {
    let entry = global_store!(ui).get_video_editor_transcribe();
    let start_timestamp =
        ms_to_srt_timestamp((entry.media_duration_ms * value.clamp(0.0, 1.0)) as u64).into();
    let end_timestamp = ms_to_srt_timestamp(entry.media_duration_ms as u64).into();

    global_store!(ui).set_video_editor_transcribe_audio_player_progress(value);
    global_logic!(ui).invoke_video_editor_transcribe_play_audio(start_timestamp, end_timestamp);
}

fn video_editor_transcribe_audio_player_progress_pressed(ui: &AppWindow, value: f32) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.clear();
    };

    global_store!(ui).set_video_editor_transcribe_audio_player_progress(value);
}

fn video_editor_transcribe_play_audio(
    ui: &AppWindow,
    start_timestamp: SharedString,
    end_timestamp: SharedString,
) {
    let Ok(start_ms) = srt_timestamp_to_ms(&start_timestamp) else {
        return;
    };

    let Ok(end_ms) = srt_timestamp_to_ms(&end_timestamp) else {
        return;
    };

    let start_ms = if start_ms == end_ms { 0 } else { start_ms };

    let (audio_config, sink) = {
        let player = CURRENT_AUDIO_PLAYER.lock().unwrap();
        let audio_config = match player.audio_config.clone() {
            Some(config) => config,
            None => {
                toast_warn!(ui, tr("No audio file loaded"));
                return;
            }
        };
        let sink = match player.audio_sink.clone() {
            Some(sink) => sink,
            None => {
                toast_warn!(ui, tr("Audio player not initialized"));
                return;
            }
        };
        (audio_config, sink)
    };

    let config = global_store!(ui).get_video_editor_transcribe_setting();
    sink.set_volume(config.audio_sound);
    sink.set_speed(config.audio_speed.max(0.5));

    let ui_weak = ui.as_weak();
    let runtime_handle = tokio::runtime::Handle::current();

    std::thread::spawn(move || {
        if let Err(e) = play_audio_segment(
            ui_weak.clone(),
            runtime_handle,
            &audio_config,
            start_ms,
            end_ms,
            sink,
        ) {
            toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("Failed to play audio")));
        }
    });
}

fn play_audio_segment(
    ui_weak: Weak<AppWindow>,
    runtime_handle: tokio::runtime::Handle,
    audio_config: &AudioConfig,
    start_ms: u64,
    end_ms: u64,
    player: Arc<Player>,
) -> Result<(), Box<dyn std::error::Error>> {
    player.clear();
    player.stop();

    let mut samples = extract_audio_samples(audio_config, start_ms, end_ms);
    let total_duration_ms = audio_config.duration.as_millis() as u64;

    apply_fade_in(
        &mut samples,
        audio_config.channel,
        audio_config.sample_rate,
        200,
    );

    let source = SamplesBuffer::new(
        ChannelCount::new(audio_config.channel as u16).unwrap(),
        SampleRate::new(audio_config.sample_rate).unwrap(),
        samples,
    );

    player.append(source);

    let player_clone = player.clone();
    let ui_weak_clone = ui_weak.clone();
    let start_time = std::time::Instant::now();

    let progress = (start_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0);
    _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_video_editor_transcribe_audio_player_progress(progress as f32);
        global_logic!(ui)
            .invoke_video_editor_transcribe_subtitles_update_playing_index(progress as f32);
        global_store!(ui).set_video_editor_transcribe_audio_player_is_playing(true);
    });

    let inc_index = {
        let mut player = CURRENT_AUDIO_PLAYER.lock().unwrap();
        player.inc_index += 1;
        player.inc_index
    };

    runtime_handle.spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(20));
        loop {
            interval.tick().await;

            if CURRENT_AUDIO_PLAYER.lock().unwrap().inc_index != inc_index {
                break;
            }

            let speed = CURRENT_AUDIO_PLAYER
                .lock()
                .unwrap()
                .audio_sink
                .as_ref()
                .map(|s| s.speed())
                .unwrap_or(1.0);
            let elapsed_ms =
                start_ms + (start_time.elapsed().as_millis() as f64 * speed as f64) as u64;
            let progress = (elapsed_ms as f64 / total_duration_ms as f64).clamp(0.0, 1.0);

            _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                if CURRENT_AUDIO_PLAYER.lock().unwrap().inc_index == inc_index {
                    global_store!(ui)
                        .set_video_editor_transcribe_audio_player_progress(progress as f32);
                }
            });

            if player_clone.is_paused() {
                break;
            }
        }
    });

    player.play();
    player.sleep_until_end();
    player.pause();

    _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_video_editor_transcribe_audio_player_is_playing(false);
    });

    Ok(())
}

pub fn extract_audio_samples(audio_config: &AudioConfig, start_ms: u64, end_ms: u64) -> Vec<f32> {
    let sample_rate = audio_config.sample_rate as u64;
    let channels = audio_config.channel as u64;
    let start_sample = ((sample_rate * start_ms * channels) / 1000) as usize;
    let end_sample =
        ((sample_rate * end_ms * channels) / 1000).min(audio_config.samples.len() as u64) as usize;

    if start_sample > end_sample {
        return vec![];
    }

    let mut samples = Vec::with_capacity(end_sample - start_sample);
    samples.extend(&audio_config.samples[start_sample..end_sample]);
    samples
}

fn video_editor_transcribe_stop_audio(ui: &AppWindow) {
    global_store!(ui).set_video_editor_transcribe_audio_player_is_playing(false);
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.stop();
    };
}

fn video_editor_transcribe_sound_wave_update(ui: &AppWindow, index: i32, max_samples: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitle = store_video_editor_transcribe_subtitles!(entry)
        .row_data(index)
        .unwrap();

    let Ok(start_ms) = srt_timestamp_to_ms(&subtitle.start_timestamp) else {
        return;
    };

    let Ok(end_ms) = srt_timestamp_to_ms(&subtitle.end_timestamp) else {
        return;
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(audio_config) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_config.clone() else {
            return;
        };

        let samples = extract_audio_samples(&audio_config, start_ms, end_ms);
        if samples.is_empty() {
            return;
        }

        let samples = downsample_audio(&samples, max_samples as usize);
        let amplitude = max_sound_wave_amplitude(&samples);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let entry = global_store!(ui).get_video_editor_transcribe();
            let subtitles = store_video_editor_transcribe_subtitles!(entry);

            if index >= subtitles.row_count() {
                return;
            }

            let mut subtitle = subtitles.row_data(index).unwrap();
            subtitle.audio_wave_amplitude = amplitude;
            subtitle.audio_samples = ModelRc::new(VecModel::from_slice(&samples));
            subtitles.set_row_data(index, subtitle);

            mark_overlapped_timestamp(&ui, index as usize);
        });
    });
}

fn video_editor_transcribe_sound_wave_zoom_changed(ui: &AppWindow, index: i32, level: f32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let mut subtitle = subtitles.row_data(index).unwrap();

    let Ok(start_ms) = srt_timestamp_to_ms(&subtitle.start_timestamp) else {
        return;
    };

    let Ok(end_ms) = srt_timestamp_to_ms(&subtitle.end_timestamp) else {
        return;
    };

    let new_end_ms = if level <= 0.0 {
        start_ms
    } else {
        let duration = ((end_ms - start_ms) as f32 / level) as u64;
        if duration == 0 {
            start_ms + 1000
        } else {
            start_ms + duration
        }
    };

    subtitle.end_timestamp = ms_to_srt_timestamp(new_end_ms).into();
    subtitles.set_row_data(index, subtitle);
    video_editor_transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}

fn video_editor_transcribe_sound_wave_moved(ui: &AppWindow, index: i32, percent: f32) {
    if percent == 0.0 {
        return;
    }

    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let mut subtitle = subtitles.row_data(index).unwrap();

    let Ok(start_ms) = srt_timestamp_to_ms(&subtitle.start_timestamp) else {
        return;
    };

    let Ok(end_ms) = srt_timestamp_to_ms(&subtitle.end_timestamp) else {
        return;
    };

    let duration = ((end_ms - start_ms) as f32 * percent) as i64;
    let new_start_ms = (start_ms as i64 + duration).max(0) as u64;
    let new_end_ms = (end_ms as i64 + duration).max(0) as u64;

    subtitle.start_timestamp = ms_to_srt_timestamp(new_start_ms).into();
    subtitle.end_timestamp = ms_to_srt_timestamp(new_end_ms).into();
    subtitles.set_row_data(index, subtitle);
    video_editor_transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}

fn video_editor_transcribe_sound_wave_start_position_changed(ui: &AppWindow, index: i32, pos: f32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let mut subtitle = subtitles.row_data(index).unwrap();

    let Ok(start_ms) = srt_timestamp_to_ms(&subtitle.start_timestamp) else {
        return;
    };

    let Ok(end_ms) = srt_timestamp_to_ms(&subtitle.end_timestamp) else {
        return;
    };

    let duration = (end_ms - start_ms) as f32 * pos;
    let new_start_ms = start_ms + duration as u64;

    subtitle.start_timestamp = ms_to_srt_timestamp(new_start_ms).into();
    subtitles.set_row_data(index, subtitle);
    video_editor_transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}

fn video_editor_transcribe_sound_wave_end_position_changed(ui: &AppWindow, index: i32, pos: f32) {
    let index = index as usize;
    let entry = global_store!(ui).get_video_editor_transcribe();
    let subtitles = store_video_editor_transcribe_subtitles!(entry);
    let mut subtitle = subtitles.row_data(index).unwrap();

    let Ok(start_ms) = srt_timestamp_to_ms(&subtitle.start_timestamp) else {
        return;
    };

    let Ok(end_ms) = srt_timestamp_to_ms(&subtitle.end_timestamp) else {
        return;
    };

    let duration = (end_ms - start_ms) as f32 * pos;
    let new_end_ms = start_ms + duration as u64;

    subtitle.end_timestamp = ms_to_srt_timestamp(new_end_ms).into();
    subtitles.set_row_data(index, subtitle);
    video_editor_transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}
