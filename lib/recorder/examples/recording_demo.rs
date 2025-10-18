use recorder::{FPS, RecorderConfig, RecordingSession};
use std::sync::atomic::Ordering;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::debug!("Press Ctrl-C to stop recording.");

    let screen_infos = capture::available_screens()?;
    assert!(!screen_infos.is_empty());

    RecordingSession::init(&screen_infos[0].name)?;

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        RecorderConfig::make_filename("target"),
    )
    .with_fps(FPS::Fps30);

    log::debug!("Recording configuration: {:#?}", config);

    let mut session = RecordingSession::new(config);

    let stop_sig = session.stop_sig().clone();
    ctrlc::set_handler(move || {
        log::debug!("Ctrl-C received, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    })?;

    session.start()?;
    session.wait(None::<Box<dyn FnMut(f32)>>, move |v| {
        let v = (v * 100.0) as u32;
        log::debug!("combine tracks progress: {v}%");
    })?;

    Ok(())
}
