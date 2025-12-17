use recorder::{
    AudioRecorder, FPS, RecorderConfig, RecordingSession, bounded, platform_screen_capture,
};
use screen_capture::ScreenCapture;
use std::{sync::atomic::Ordering, thread, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("Recording for exactly 5 seconds...");

    let audio_recorder = AudioRecorder::new();
    let Some(default_input) = audio_recorder.get_default_input_device()? else {
        panic!("No default input device found");
    };

    log::info!(
        "default audio device name: {}. config: {:?}",
        default_input.name,
        default_input.default_config
    );

    let mut screen_capturer = platform_screen_capture();
    let screen_infos = screen_capturer.available_screens()?;
    assert!(!screen_infos.is_empty());

    log::info!("screen_infos: {screen_infos:?}");

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        #[cfg(not(target_os = "windows"))]
        RecorderConfig::make_filename("/tmp"),
        #[cfg(target_os = "windows")]
        RecorderConfig::make_filename("C:/Users/blue/Desktop"),
    )
    // .with_enable_audio_channel_user(true)
    // .with_enable_speaker_channel_user(true)
    .with_enable_denoise(true)
    .with_audio_device_name(Some(default_input.name))
    .with_enable_recording_speaker(true)
    .with_convert_to_mono(true)
    .with_resolution(recorder::Resolution::Original((
        screen_infos[0].logical_size.width as u32,
        screen_infos[0].logical_size.height as u32,
    )))
    .with_enable_cursor_tracking(true)
    .with_fps(FPS::Fps30);

    log::info!("Recording configuration: {:#?}", config);

    let rt_handle = tokio::runtime::Handle::current();
    let (frame_sender, frame_receiver) = bounded(32);
    let mut session = RecordingSession::new(config).with_frame_sender_user(Some(frame_sender));
    session.start(rt_handle, screen_capturer)?;

    let stop_sig = session.get_stop_sig().clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        log::info!("5 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    thread::spawn(move || {
        while let Ok(frame) = frame_receiver.recv() {
            log::debug!(
                "frame_receiver_user frame len: {} bytes",
                frame.buffer.len()
            );
        }
    });

    if let Some(rx) = session.get_audio_level_receiver() {
        thread::spawn(move || {
            while let Ok(db) = rx.recv() {
                log::debug!("audio_level_receiver_user db level: {db:.0}",);
            }
            log::info!("exit audio_level_receiver_user");
        });
    } else {
        log::info!("audio_level_receiver_user is none");
    }

    if let Some(rx) = session.get_speaker_level_receiver() {
        thread::spawn(move || {
            while let Ok(db) = rx.recv() {
                log::debug!("speaker_level_receiver_user db level: {db:.0}",);
            }
            log::info!("exit speaker_level_receiver_user");
        });
    } else {
        log::info!("speaker_level_receiver_user is none");
    }

    session.wait()?;

    log::info!("Recording completed successfully!");

    Ok(())
}
