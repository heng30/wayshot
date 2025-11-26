use hound::WavWriter;
use recorder::{SpeakerRecorder, SpeakerRecorderConfig, bounded, platform_speaker_recoder};
use std::{
    error::Error,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let stop_sig = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = bounded(1024);

    let stop_sig_clone = stop_sig.clone();
    ctrlc::set_handler(move || {
        log::debug!("Ctrl-C received, stopping recording...");
        stop_sig_clone.store(true, Ordering::Relaxed);
    })?;

    let stop_sig_clone = stop_sig.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(10));
        log::debug!("10 seconds elapsed, stopping recording...");
        stop_sig_clone.store(true, Ordering::Relaxed);
    });

    // Start audio playback thread to generate something for loopback recording to capture
    let playback_stop = Arc::new(AtomicBool::new(false));
    let playback_stop_clone = playback_stop.clone();
    thread::spawn(move || {
        // Generate a simple 440Hz tone using Windows API
        play_tone_for_test(&playback_stop_clone);
    });

    let handle = thread::spawn(move || {
        #[cfg(target_os = "windows")]
        let save_path = PathBuf::from("C:/Users/blue/Desktop/speaker_with_audio_test.wav");

        #[cfg(not(target_os = "windows"))]
        let save_path = PathBuf::from("/tmp/speaker_with_audio_test.wav");

        let spec = platform_speaker_recoder(SpeakerRecorderConfig::default())
            .unwrap()
            .spec();

        let mut writer = match WavWriter::create(&save_path, spec) {
            Ok(writer) => {
                log::info!("Created WAV file: {}", save_path.display());
                writer
            }
            Err(e) => {
                log::error!("Failed to create WAV file: {}", e);
                return;
            }
        };

        let mut samples_written = 0u64;
        while let Ok(frame) = receiver.recv() {
            for &sample in &frame {
                if let Err(e) = writer.write_sample(sample) {
                    log::error!("Failed to write audio sample: {}", e);
                    break;
                }
                samples_written += 1;
            }
        }

        match writer.finalize() {
            Ok(_) => {
                log::info!("Successfully saved WAV file: {}", save_path.display());
                log::info!("Total samples written: {}", samples_written);
                let estimated_duration = samples_written as f64 / (44100.0 * 2.0); // stereo samples
                log::info!("Estimated duration: {:.2} seconds", estimated_duration);
            }
            Err(e) => log::error!("Failed to finalize WAV file: {}", e),
        }
    });

    let config = SpeakerRecorderConfig::new(stop_sig).with_frame_sender(Some(sender));
    platform_speaker_recoder(config)?.start_recording()?;

    // Stop playback
    playback_stop.store(true, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(500)); // Give playback time to stop

    handle.join().unwrap();

    Ok(())
}

// Simple tone generator for testing
#[cfg(target_os = "windows")]
fn play_tone_for_test(stop_signal: &Arc<AtomicBool>) {
    use winapi::um::winuser::MB_OK;
    use winapi::um::winuser::MessageBeep;

    log::info!("Starting test tone generation...");

    while !stop_signal.load(Ordering::Relaxed) {
        // Generate system beep sounds periodically
        // Note: This may not be captured by loopback recording as it goes through different path
        unsafe {
            MessageBeep(MB_OK);
        }
        thread::sleep(Duration::from_secs(1));

        log::debug!("Played test beep");
    }

    log::info!("Stopped test tone generation");
}

#[cfg(not(target_os = "windows"))]
fn play_tone_for_test(_stop_signal: &Arc<AtomicBool>) {
    log::warn!("Audio playback test not implemented for this platform");
}
