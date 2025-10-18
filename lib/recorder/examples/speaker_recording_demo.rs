use recorder::SpeakerRecorder;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let save_path = PathBuf::from("target/speaker_output.wav");
    let stop_sig = Arc::new(AtomicBool::new(false));

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

    let mut recorder = SpeakerRecorder::new(save_path.clone(), stop_sig, None, false)?;
    recorder.start_recording()?;
    log::info!("Successfully save: {}", save_path.display());
    Ok(())
}
