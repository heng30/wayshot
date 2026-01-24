use crate::{
    global_store,
    logic::toast,
    logic_cb,
    slint_generatedAppWindow::{AppWindow, Subtitle as UISubtitle},
    store_transcribe_subtitles, toast_warn,
};
use audio_utils::audio::AudioConfig;
use cutil::time::media_timestamp_to_ms;
use once_cell::sync::Lazy;
use rodio::{OutputStream, OutputStreamBuilder, Sink, buffer::SamplesBuffer};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::sync::{Arc, Mutex};
use video_utils::subtitle::{ms_to_srt_timestamp, srt_timestamp_to_ms};

const MAX_WAVE_FORM_SAMPLE_COUNTS: i32 = 200;
static CURRENT_AUDIO_PLAYER: Lazy<Mutex<CurrentAudioPlayer>> =
    Lazy::new(|| Mutex::new(CurrentAudioPlayer::default()));

#[macro_export]
macro_rules! store_transcribe_subtitle_audio_samples {
    ($subtitle: expr) => {
        $subtitle
            .audio_samples
            .as_any()
            .downcast_ref::<VecModel<f32>>()
            .expect("We know we set a VecModel<f32> earlier")
    };
}

#[derive(Default)]
struct CurrentAudioPlayer {
    audio_config: Option<AudioConfig>,
    audio_sink: Option<Arc<Sink>>,
    audio_stream: Option<Arc<OutputStream>>,
}

pub fn init(ui: &AppWindow) {
    logic_cb!(transcribe_audio_player_init, ui);
    logic_cb!(transcribe_play_audio, ui, start_timestamp, end_timestamp);
    logic_cb!(transcribe_stop_audio, ui);
    logic_cb!(transcribe_sound_wave_update, ui, index, max_samples);
    logic_cb!(transcribe_sound_wave_zoom_changed, ui, index, level);
    logic_cb!(transcribe_sound_wave_moved, ui, index, percent);
    logic_cb!(transcribe_sound_wave_start_position_changed, ui, index, pos);
    logic_cb!(transcribe_sound_wave_end_position_changed, ui, index, pos);
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

fn transcribe_audio_player_init(ui: &AppWindow) {
    match OutputStreamBuilder::open_default_stream() {
        Ok(stream) => {
            let sound = global_store!(ui)
                .get_transcribe_setting()
                .audio_sound
                .clamp(0.0, 1.0);

            let sink = Sink::connect_new(stream.mixer());
            sink.set_volume(sound);

            let mut audio_player = CURRENT_AUDIO_PLAYER.lock().unwrap();
            audio_player.audio_sink = Some(Arc::new(sink));
            audio_player.audio_stream = Some(Arc::new(stream));
        }
        Err(e) => toast_warn!(ui, format!("Failed to create audio output stream: {e}")),
    }
}

fn transcribe_play_audio(
    ui: &AppWindow,
    start_timestamp: SharedString,
    end_timestamp: SharedString,
) {
    let start_ms = match srt_timestamp_to_ms(&start_timestamp) {
        Ok(ms) => ms,
        Err(_) => {
            toast_warn!(ui, "Invalid start timestamp");
            return;
        }
    };

    let end_ms = match srt_timestamp_to_ms(&end_timestamp) {
        Ok(ms) => ms,
        Err(_) => {
            toast_warn!(ui, "Invalid end timestamp");
            return;
        }
    };

    if start_ms >= end_ms {
        toast_warn!(ui, "Invalid time range");
        return;
    }

    let (audio_config, sink) = {
        let player = CURRENT_AUDIO_PLAYER.lock().unwrap();
        let audio_config = match player.audio_config.clone() {
            Some(config) => config,
            None => {
                toast_warn!(ui, "No audio file loaded");
                return;
            }
        };
        let sink = match player.audio_sink.clone() {
            Some(sink) => sink,
            None => {
                toast_warn!(ui, "Audio player not initialized");
                return;
            }
        };
        (audio_config, sink)
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        if let Err(e) = play_audio_segment(&audio_config, start_ms, end_ms, sink) {
            toast::async_toast_warn(ui_weak, format!("Failed to play audio: {e}"));
        }
    });
}

fn play_audio_segment(
    audio_config: &AudioConfig,
    start_ms: u64,
    end_ms: u64,
    sink: Arc<Sink>,
) -> Result<(), Box<dyn std::error::Error>> {
    sink.stop();
    sink.clear();

    let source = SamplesBuffer::new(
        audio_config.channel as u16,
        audio_config.sample_rate,
        extract_audio_samples(audio_config, start_ms, end_ms),
    );

    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}

fn extract_audio_samples(audio_config: &AudioConfig, start_ms: u64, end_ms: u64) -> Vec<f32> {
    let sample_rate = audio_config.sample_rate as u64;
    let channels = audio_config.channel as u64;
    let start_sample = ((sample_rate * start_ms * channels) / 1000) as usize;
    let end_sample =
        ((sample_rate * end_ms * channels) / 1000).min(audio_config.samples.len() as u64) as usize;

    let mut samples = Vec::with_capacity(end_sample - start_sample);
    samples.extend(&audio_config.samples[start_sample..end_sample]);
    samples
}

fn transcribe_stop_audio(_ui: &AppWindow) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.stop();
    };
}

fn transcribe_sound_wave_update(ui: &AppWindow, index: i32, max_samples: i32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitle = store_transcribe_subtitles!(entry).row_data(index).unwrap();

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
        let amplitude = get_sound_wave_amplitude(&samples);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let entry = global_store!(ui).get_transcribe();
            let subtitles = store_transcribe_subtitles!(entry);

            if index >= subtitles.row_count() {
                return;
            }

            let mut subtitle = subtitles.row_data(index).unwrap();
            subtitle.audio_wave_amplitude = amplitude;
            subtitle.audio_samples = ModelRc::new(VecModel::from_slice(&samples));
            subtitles.set_row_data(index, subtitle);
        });
    });
}

fn downsample_audio(audio_data: &[f32], target_length: usize) -> Vec<f32> {
    if audio_data.len() <= target_length {
        return audio_data.to_vec();
    }

    let chunk_size = (audio_data.len() as f32 / target_length as f32).ceil() as usize;

    audio_data
        .chunks(chunk_size)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
        .collect()
}

fn transcribe_sound_wave_zoom_changed(ui: &AppWindow, index: i32, level: f32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
    transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}

fn transcribe_sound_wave_moved(ui: &AppWindow, index: i32, percent: f32) {
    if percent == 0.0 {
        return;
    }

    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
    transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}

fn transcribe_sound_wave_start_position_changed(ui: &AppWindow, index: i32, pos: f32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
    transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}

fn transcribe_sound_wave_end_position_changed(ui: &AppWindow, index: i32, pos: f32) {
    let index = index as usize;
    let entry = global_store!(ui).get_transcribe();
    let subtitles = store_transcribe_subtitles!(entry);
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
    transcribe_sound_wave_update(ui, index as i32, MAX_WAVE_FORM_SAMPLE_COUNTS);
}
