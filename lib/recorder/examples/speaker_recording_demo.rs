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
        thread::sleep(Duration::from_secs(5));
        log::debug!("5 seconds elapsed, stopping recording...");
        stop_sig_clone.store(true, Ordering::Relaxed);
    });

    let handle = thread::spawn(move || {
        #[cfg(target_os = "windows")]
        let save_path = PathBuf::from("C:/Users/blue/Desktop/speaker_output.wav");

        #[cfg(not(target_os = "windows"))]
        let save_path = PathBuf::from("/tmp/speaker_output.wav");

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

        while let Ok(frame) = receiver.recv() {
            for &sample in &frame {
                if let Err(e) = writer.write_sample(sample) {
                    log::error!("Failed to write audio sample: {}", e);
                    break;
                }
            }
        }

        match writer.finalize() {
            Ok(_) => log::info!("Successfully saved WAV file: {}", save_path.display()),
            Err(e) => log::error!("Failed to finalize WAV file: {}", e),
        }
    });

    let config = SpeakerRecorderConfig::new(stop_sig).with_frame_sender(Some(sender));
    platform_speaker_recoder(config)?.start_recording()?;

    handle.join().unwrap();

    Ok(())
}
