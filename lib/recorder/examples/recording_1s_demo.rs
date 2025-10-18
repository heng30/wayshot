use recorder::{FPS, RecorderConfig, RecordingSession};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::debug!("Recording for exactly 5 seconds...");

    let screen_infos = capture::available_screens()?;
    assert!(!screen_infos.is_empty());

    log::debug!("screen_infos: {screen_infos:?}");

    RecordingSession::init(&screen_infos[0].name)?;

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        PathBuf::from("target/info.mp4"),
    )
    .with_resolution(recorder::Resolution::Original((
        screen_infos[0].logical_size.width as u32,
        screen_infos[0].logical_size.height as u32,
    )))
    .with_fps(FPS::Fps30);

    log::debug!("Recording configuration: {:#?}", config);

    let mut session = RecordingSession::new(config);

    let stop_sig = session.stop_sig().clone();

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        log::debug!("5 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    session.start()?;
    session.wait(None::<Box<dyn FnMut(f32)>>, move |v| {
        let v = (v * 100.0) as u32;
        log::debug!("combine tracks progress: {v}%");
    })?;

    log::debug!("Recording completed successfully!");

    Ok(())
}
