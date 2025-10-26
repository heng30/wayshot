use recorder::{AudioRecorder, FPS, RecorderConfig, RecordingSession, bounded};
use std::{sync::atomic::Ordering, thread, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let screen_infos = capture::available_screens()?;
    assert!(!screen_infos.is_empty());

    RecordingSession::init(&screen_infos[0].name)?;

    log::info!("screen_infos: {screen_infos:?}");

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        RecorderConfig::make_filename("/tmp"),
    )
    // .with_enable_audio_channel_user(true)
    // .with_enable_speaker_channel_user(true)
    // .with_enable_denoise(true)
    .with_audio_device_name(Some(default_input.name))
    // .with_enable_recording_speaker(true)
    // .with_convert_mono(true)
    .with_resolution(recorder::Resolution::Original((
        screen_infos[0].logical_size.width as u32,
        screen_infos[0].logical_size.height as u32,
    )))
    .with_fps(FPS::Fps30);

    log::info!("Recording configuration: {:#?}", config);

    let (frame_sender, frame_receiver) = bounded(32);
    let mut session = RecordingSession::new(config).with_frame_sender_user(Some(frame_sender));
    session.start()?;

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
                frame.frame.cb_data.data.pixel_data.len()
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
