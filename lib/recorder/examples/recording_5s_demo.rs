use recorder::{AudioRecorder, FPS, RecorderConfig, RecordingSession};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::debug!("Recording for exactly 5 seconds...");

    let audio_recorder = AudioRecorder::new(Some(1024))?;
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

    RecordingSession::init(&screen_infos[0].name)?;

    log::debug!("screen_infos: {screen_infos:?}");

    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        RecorderConfig::make_filename("target"),
    )
    .with_enable_frame_channel_user(true)
    .with_enable_audio_channel_user(true)
    .with_enable_speaker_channel_user(true)
    // .with_enable_denoise(true)
    // .with_disable_save_file(true)
    .with_audio_device_name(Some(default_input.name))
    .with_enable_recording_speaker(true)
    // .with_convert_input_wav_to_mono(true)
    .with_resolution(recorder::Resolution::Original((
        screen_infos[0].logical_size.width as u32,
        screen_infos[0].logical_size.height as u32,
    )))
    .with_fps(FPS::Fps30);

    log::debug!("Recording configuration: {:#?}", config);

    let mut session = RecordingSession::new(config);
    session.start()?;

    let stop_sig = session.stop_sig().clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        log::debug!("5 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    let frame_receiver_user = session.get_frame_receiver_user();
    thread::spawn(move || {
        if let Some(rx) = frame_receiver_user {
            while let Ok(frame) = rx.recv() {
                if let Some(frame) = frame.frame {
                    log::debug!(
                        "frame_receiver_user frame len: {} bytes",
                        frame.cb_data.data.pixel_data.len()
                    );
                }
            }
            log::debug!("exit frame_receiver_user");
        } else {
            log::debug!("frame_receiver_user is none");
        }
    });

    let audio_level_receiver_user = session.get_audio_level_receiver_user();
    thread::spawn(move || {
        if let Some(rx) = audio_level_receiver_user {
            while let Ok(db) = rx.recv() {
                log::debug!("audio_level_receiver_user db level: {db:.0}",);
            }
            log::debug!("exit audio_level_receiver_user");
        } else {
            log::debug!("audio_level_receiver_user is none");
        }
    });

    let speaker_level_receiver_user = session.get_speaker_level_receiver_user();
    thread::spawn(move || {
        if let Some(rx) = speaker_level_receiver_user {
            while let Ok(db) = rx.recv() {
                log::debug!("speaker_level_receiver_user db level: {db:.0}",);
            }
            log::debug!("exit speaker_level_receiver_user");
        } else {
            log::debug!("speaker_level_receiver_user is none");
        }
    });

    session.wait(
        None::<Box<dyn FnMut(f32)>>,
        // Some(move |v| {
        //     let v = (v * 100.0) as u32;
        //     log::debug!("denoise progress: {v}%");
        // }),
        move |v| {
            let v = (v * 100.0) as u32;
            log::debug!("combine tracks progress: {v}%");
        },
    )?;

    log::debug!("Recording completed successfully!");

    Ok(())
}
