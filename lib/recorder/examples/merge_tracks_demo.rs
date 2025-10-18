use recorder::{FPS, MergeTracksConfig, merge_tracks};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let h264_path = PathBuf::from("target/video-track.h264");
    let wav_path = PathBuf::from("target/audio-track.wav");
    let output_path = PathBuf::from("target/combine-tracks.mp4");

    if !h264_path.exists() {
        log::warn!("video track not found: {}", h264_path.display());
        return Ok(());
    }

    if !wav_path.exists() {
        log::warn!("audio track not found: {}", wav_path.display());
        return Ok(());
    }

    let config = MergeTracksConfig {
        h264_path,
        input_wav_path: Some(wav_path),
        speaker_wav_path: None,
        output_path: output_path.clone(),
        fps: FPS::Fps30,
        stop_sig: Arc::new(AtomicBool::new(false)),
        convert_input_wav_to_mono: false,
    };

    let now = std::time::Instant::now();
    merge_tracks(config, move |v| {
        let v = (v * 100.0) as u32;
        log::debug!("combine tracks progress: {v}%");
    })?;
    log::debug!("combine tracks time: {:.2?}", now.elapsed());
    log::info!("save to: {}", output_path.display());

    Ok(())
}
