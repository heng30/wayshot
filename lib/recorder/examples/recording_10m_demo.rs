use recorder::{AudioRecorder, FPS, RecorderConfig, RecordingSession};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::debug!("Recording for exactly 5 seconds...");

    let audio_recorder = AudioRecorder::new();
    let Some(default_input) = audio_recorder.get_default_input_device()? else {
        panic!("No default input device found");
    };

    log::debug!(
        "default audio device name: {}. config: {:?}",
        default_input.name,
        default_input.default_config
    );

    let screen_infos = capture::available_screens()?;
    assert!(!screen_infos.is_empty());

    log::debug!("screen_infos: {screen_infos:?}");

    RecordingSession::init(&screen_infos[0].name)?;

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        RecorderConfig::make_filename("target"),
    )
    .with_enable_recording_speaker(true)
    // .with_audio_device_name(Some(default_input.name))
    .with_fps(FPS::Fps30);
    // .with_resolution(recorder::Resolution::Original((
    //     screen_infos[0].logical_size.width as u32,
    //     screen_infos[0].logical_size.height as u32,
    // )));

    log::debug!("Recording configuration: {:#?}", config);

    let mut session = RecordingSession::new(config);

    let stop_sig = session.get_stop_sig().clone();

    // Start a timer thread that stops recording after 5 seconds
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(3600));
        log::debug!("5 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    session.start()?;
    session.wait()?;

    log::debug!("Recording completed successfully!");

    Ok(())
}
