use recorder::{
    AudioRecorder, FPS, RecorderConfig, RecordingSession, ShareScreenConfig,
    platform_screen_capture,
};
use screen_capture::ScreenCapture;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let mut screen_capturer = platform_screen_capture();
    let screen_infos = screen_capturer.available_screens()?;
    assert!(!screen_infos.is_empty());

    log::debug!("screen_infos: {screen_infos:?}");

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        RecorderConfig::make_filename("/tmp"),
    )
    .with_process_mode(recorder::ProcessMode::ShareScreen)
    .with_share_screen_config(ShareScreenConfig::default().with_save_mp4(true))
    .with_enable_recording_speaker(true)
    .with_audio_device_name(Some(default_input.name))
    .with_resolution(recorder::Resolution::Original((
        screen_infos[0].logical_size.width as u32,
        screen_infos[0].logical_size.height as u32,
    )))
    .with_fps(FPS::Fps30);

    log::debug!("Recording configuration: {:#?}", config);

    let mut session = RecordingSession::new(config);

    let stop_sig = session.get_stop_sig().clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(30));
        log::debug!("stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    session.start(screen_capturer)?;
    session.wait()?;

    log::debug!("Recording completed successfully!");

    Ok(())
}
