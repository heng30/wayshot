use hound::WavWriter;
use recorder::{AudioRecorder, bounded};
use std::{path::PathBuf, thread, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let (sender, receiver) = bounded(1024);
    let mut recorder = AudioRecorder::new()
        .with_enable_denoise(true)
        .with_frame_sender(Some(sender));

    let device_name = "default";
    recorder.start_recording(device_name)?;

    let spec = recorder.spec(device_name)?;
    let handle = thread::spawn(move || {
        let save_path = PathBuf::from("/tmp/audio_output.wav");

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

    log::debug!("Recording started. Speak into your microphone for 5 seconds...");

    for i in (1..=5).rev() {
        log::debug!("Recording... {} seconds remaining", i);
        std::thread::sleep(Duration::from_secs(1));
    }

    recorder.stop();
    handle.join().unwrap();

    Ok(())
}
