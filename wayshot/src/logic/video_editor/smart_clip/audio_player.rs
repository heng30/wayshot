use crate::{global_store, slint_generatedAppWindow::AppWindow};
use audio_utils::{audio::apply_fade_in, loader::AudioConfig};
use once_cell::sync::Lazy;
use rodio::{
    ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer,
};
use slint::ComponentHandle;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

static CURRENT_AUDIO_PLAYER: Lazy<Mutex<CurrentAudioPlayer>> =
    Lazy::new(|| Mutex::new(CurrentAudioPlayer::default()));

/// Entry info for playing-index tracking.
/// Each item is (index_in_all_entries, start_ms, end_ms, is_kept).
pub type EntryInfo = (usize, u64, u64, bool);

#[derive(Default)]
struct CurrentAudioPlayer {
    audio_sink: Option<Arc<Player>>,
    audio_stream: Option<Arc<MixerDeviceSink>>,
    inc_index: u64,
    /// The total concatenated duration (ms) of ALL checked segments (for progress bar)
    total_concat_duration_ms: u64,
    /// The offset (ms) within the total concat timeline where current playback starts
    playback_start_offset_ms: u64,
    /// The duration (ms) of the audio currently being played (subset from seek point)
    playback_duration_ms: u64,
    /// All entries info for playing-index tracking (index, start_ms, end_ms, is_kept)
    all_entries: Vec<EntryInfo>,
    /// Whether to update playing-index during playback (true for full play, false for single entry)
    update_playing_index: bool,
}

pub fn init(_ui: &AppWindow) {
    match DeviceSinkBuilder::open_default_sink() {
        Ok(device_sink) => {
            let player = Player::connect_new(&device_sink.mixer());
            player.set_volume(0.8);

            let mut audio_player = CURRENT_AUDIO_PLAYER.lock().unwrap();
            audio_player.audio_sink = Some(Arc::new(player));
            audio_player.audio_stream = Some(Arc::new(device_sink));
        }
        Err(e) => {
            log::warn!("Failed to create audio output stream for smart clip: {e}");
        }
    }
}

pub fn stop_audio() {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.stop();
    };
}

pub fn set_volume(volume: f32) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.set_volume(volume);
    };
}

pub fn set_speed(speed: f32) {
    if let Some(ref sink) = CURRENT_AUDIO_PLAYER.lock().unwrap().audio_sink {
        sink.set_speed(speed);
    };
}

/// Play checked segments starting from a given offset within the total concat timeline.
///
/// - `total_concat_duration_ms`: total duration of ALL checked segments (for progress bar full range)
/// - `playback_start_offset_ms`: offset within the total concat timeline where playback begins
/// - `segments`: the audio segments to actually play (may be a subset from the seek point)
/// - `all_entries`: all entries info (index, start_ms, end_ms, is_kept) for playing-index tracking
/// - `update_playing_index`: whether to update playing-index during playback (true for full play only)
pub fn play_segments_from_offset(
    ui: &AppWindow,
    audio_config: &AudioConfig,
    segments: &[(u64, u64)],
    total_concat_duration_ms: u64,
    playback_start_offset_ms: u64,
    all_entries: Vec<EntryInfo>,
    update_playing_index: bool,
) {
    let sink = {
        let player = CURRENT_AUDIO_PLAYER.lock().unwrap();
        match player.audio_sink.clone() {
            Some(sink) => sink,
            None => return,
        }
    };

    let sound = global_store!(ui).get_video_editor_smart_clip_audio_sound();
    sink.set_volume(sound.clamp(0.0, 1.0));

    let ui_weak = ui.as_weak();
    let runtime_handle = tokio::runtime::Handle::current();

    // Concatenate audio from segments
    let mut combined_samples: Vec<f32> = Vec::new();
    let mut playback_duration_ms: u64 = 0;

    for (start_ms, end_ms) in segments.iter() {
        let dur = end_ms.saturating_sub(*start_ms);
        playback_duration_ms += dur;
        let seg_samples = extract_audio_samples(audio_config, *start_ms, *end_ms);
        combined_samples.extend(seg_samples);
    }

    if combined_samples.is_empty() || playback_duration_ms == 0 {
        return;
    }

    // Store playback info for progress tracking
    {
        let mut player = CURRENT_AUDIO_PLAYER.lock().unwrap();
        player.total_concat_duration_ms = total_concat_duration_ms;
        player.playback_start_offset_ms = playback_start_offset_ms;
        player.playback_duration_ms = playback_duration_ms;
        player.all_entries = all_entries;
        player.update_playing_index = update_playing_index;
    }

    // Apply fade in to avoid clicks
    apply_fade_in(
        &mut combined_samples,
        audio_config.channel,
        audio_config.sample_rate,
        200,
    );

    let source = SamplesBuffer::new(
        ChannelCount::new(audio_config.channel as u16).unwrap(),
        SampleRate::new(audio_config.sample_rate).unwrap(),
        combined_samples,
    );

    sink.clear();
    sink.stop();
    sink.append(source);
    sink.play();

    let player_clone = sink.clone();
    let ui_weak_clone = ui_weak.clone();
    let start_time = std::time::Instant::now();

    let initial_progress = if total_concat_duration_ms > 0 {
        (playback_start_offset_ms as f64 / total_concat_duration_ms as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    global_store!(ui).set_video_editor_smart_clip_audio_player_progress(initial_progress as f32);
    global_store!(ui).set_video_editor_smart_clip_audio_player_is_playing(true);

    let speed = global_store!(ui).get_video_editor_smart_clip_audio_speed();
    sink.set_speed(speed);
    let inc_index = {
        let mut player = CURRENT_AUDIO_PLAYER.lock().unwrap();
        player.inc_index += 1;
        player.inc_index
    };

    runtime_handle.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;

            if CURRENT_AUDIO_PLAYER.lock().unwrap().inc_index != inc_index {
                break;
            }

            let (total_dur, start_offset, all_entries, update_idx) = {
                let player = CURRENT_AUDIO_PLAYER.lock().unwrap();
                (
                    player.total_concat_duration_ms,
                    player.playback_start_offset_ms,
                    player.all_entries.clone(),
                    player.update_playing_index,
                )
            };

            let elapsed_ms = (start_time.elapsed().as_millis() as f64 * speed as f64) as u64;
            let current_ms = start_offset + elapsed_ms;
            let progress = if total_dur > 0 {
                (current_ms as f64 / total_dur as f64).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Compute playing-index from current position in the concat timeline
            let playing_index = if update_idx && !all_entries.is_empty() {
                compute_playing_index(&all_entries, current_ms)
            } else {
                -1
            };

            _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                if CURRENT_AUDIO_PLAYER.lock().unwrap().inc_index == inc_index {
                    global_store!(ui)
                        .set_video_editor_smart_clip_audio_player_progress(progress as f32);
                    if update_idx {
                        global_store!(ui).set_video_editor_smart_clip_playing_index(playing_index);
                    }
                }
            });

            if player_clone.is_paused() {
                break;
            }
        }
    });

    // Wait for playback to end in a separate thread
    std::thread::spawn(move || {
        sink.sleep_until_end();
        sink.pause();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if CURRENT_AUDIO_PLAYER.lock().unwrap().inc_index == inc_index {
                global_store!(ui).set_video_editor_smart_clip_audio_player_is_playing(false);
                if update_playing_index {
                    global_store!(ui).set_video_editor_smart_clip_playing_index(-1);
                }
            }
        });
    });
}

/// Given the current position (ms) in the concatenated checked-segments timeline,
/// find the index of the entry being played in the full entries list.
///
/// `all_entries`: (index_in_all, start_ms, end_ms, is_kept) sorted by index
/// `current_concat_ms`: position in the concat timeline of checked segments
fn compute_playing_index(all_entries: &[EntryInfo], current_concat_ms: u64) -> i32 {
    let mut cumulative_ms: u64 = 0;
    for &(idx, start_ms, end_ms, is_kept) in all_entries {
        if !is_kept {
            continue;
        }
        let dur = end_ms.saturating_sub(start_ms);
        let seg_end = cumulative_ms + dur;
        if current_concat_ms < seg_end {
            return idx as i32;
        }
        cumulative_ms = seg_end;
    }
    // If we've gone past the last segment, return the last checked entry's index
    all_entries
        .iter()
        .rev()
        .find(|&&(_, _, _, is_kept)| is_kept)
        .map(|&(idx, _, _, _)| idx as i32)
        .unwrap_or(-1)
}

fn extract_audio_samples(audio_config: &AudioConfig, start_ms: u64, end_ms: u64) -> Vec<f32> {
    let sample_rate = audio_config.sample_rate as u64;
    let channels = audio_config.channel as u64;
    let start_sample = ((sample_rate * start_ms * channels) / 1000) as usize;
    let end_sample =
        ((sample_rate * end_ms * channels) / 1000).min(audio_config.samples.len() as u64) as usize;

    if start_sample >= end_sample {
        return vec![];
    }

    let mut samples = Vec::with_capacity(end_sample - start_sample);
    samples.extend(&audio_config.samples[start_sample..end_sample]);
    samples
}
