use portable_atomic::AtomicF32;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, buffer::SamplesBuffer};
use std::{
    num::NonZero,
    path::PathBuf,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

type AudioSink = Player;
use video_editor::{
    filters::audio::GainFilter,
    metadata::get_metadata,
    tracks::{
        audio_track::{AudioTrack, UnifiedAudioTracksMixerIterator},
        manager::Manager,
        segment::Segment,
        track::{InnerTrack, Track},
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let files = vec![
        ("data/test.mp4", -6.0), // MP4 with -6dB gain
        ("data/test.wav", 0.0),  // WAV with normal volume
    ];

    let mut manager = Manager::new();

    for (file_path, gain_db) in &files {
        let file_path = PathBuf::from(file_path);
        log::info!("Loading audio from: {}", file_path.display());

        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                log::warn!("Failed to load {}: {:?}", file_path.display(), e);
                continue;
            }
        };

        if metadata.audios.is_empty() {
            log::warn!("No audio tracks found in {}", file_path.display());
            continue;
        }

        let audio_meta = &metadata.audios[0];
        log::info!("  Sample Rate: {} Hz", audio_meta.sample_rate);
        log::info!("  Channels: {}", audio_meta.channels);
        log::info!("  Duration: {:.2}s", metadata.duration.as_secs_f64());

        // Create segment with gain filter
        let mut segment = Segment::new(Duration::from_secs(3), metadata.duration, metadata.clone(), 1.0);

        let gain_filter = GainFilter::from_db(*gain_db);
        segment.add_audio_filter(Box::new(gain_filter));
        log::info!("  Applied gain filter: {} dB", gain_db);

        let inner_track =
            InnerTrack::new(metadata.clone(), metadata.duration, vec![Arc::new(segment)]);

        let audio_track = AudioTrack {
            name: format!("Audio Track {}", file_path.display()),
            hiding: false,
            locked: false,
            track: inner_track,
        };

        manager.add_track(Track::Audio(Arc::new(audio_track)));
    }

    if manager.is_empty() {
        log::warn!("No audio tracks to mix!");
        return Ok(());
    }

    log::info!("=== Mixing {} audio track(s) ===", manager.tracks.len());
    log::info!("Total Duration: {:.2}s", manager.duration.as_secs_f64());

    let mixer_iter = manager.unified_audio_tracks_mixer_iter(
        Duration::from_secs(1),
        Duration::from_secs(3),
        Duration::from_secs(10),
        Duration::from_secs(1),
        None, // output_channels: auto-detect
        None, // output_sample_rate: auto-detect
    )?;

    let volume = Arc::new(AtomicF32::new(0.3));
    let speed = Arc::new(AtomicF32::new(1.0));
    let stop_signal = Arc::new(AtomicBool::new(false));

    log::info!("Playing mixed audio...");
    log::info!("Press Ctrl+C to stop");

    let _stream = play_audio(mixer_iter, volume, speed, stop_signal)?;

    log::info!("Playback finished!");
    std::thread::sleep(Duration::from_millis(500));

    Ok(())
}

fn play_audio(
    mixer_iter: UnifiedAudioTracksMixerIterator,
    volume: Arc<AtomicF32>,
    _speed: Arc<AtomicF32>,
    stop_sig: Arc<AtomicBool>,
) -> Result<MixerDeviceSink, Box<dyn std::error::Error>> {
    let device_sink = DeviceSinkBuilder::open_default_sink()?;
    let sink = AudioSink::connect_new(&device_sink.mixer());

    let mut current_iter = mixer_iter;
    let stop_sig_clone = stop_sig.clone();

    loop {
        if stop_sig_clone.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }

        match current_iter.next() {
            Some(audio_data) if !audio_data.samples.is_empty() => {
                sink.set_speed(1.0);
                sink.set_volume(volume.load(Ordering::Relaxed).max(0.0));

                // 使用 SamplesBuffer 而不是手动实现 Source trait
                let channels =
                    NonZero::new(audio_data.channels).ok_or("Audio channels must be non-zero")?;
                let sample_rate = NonZero::new(audio_data.sample_rate)
                    .ok_or("Audio sample rate must be non-zero")?;
                let source = SamplesBuffer::new(channels, sample_rate, audio_data.samples);
                sink.append(source);

                if sink.len() > 3 {
                    while !sink.empty() {
                        sink.set_speed(1.0);
                        sink.set_volume(volume.load(Ordering::Relaxed).max(0.0));

                        if stop_sig_clone.load(Ordering::Relaxed) {
                            sink.stop();
                            return Ok(device_sink);
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            Some(_) => continue,
            None => break,
        }
    }

    while !sink.empty() {
        sink.set_speed(1.0);
        sink.set_volume(volume.load(Ordering::Relaxed).max(0.0));

        if stop_sig_clone.load(Ordering::Relaxed) {
            sink.stop();
            return Ok(device_sink);
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    sink.sleep_until_end();
    Ok(device_sink)
}
