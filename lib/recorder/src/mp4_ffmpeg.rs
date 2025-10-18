use crate::{MergeTracksConfig, ProgressState, RecorderError};
use ffmpeg_sidecar::{
    command::FfmpegCommand,
    event::{FfmpegDuration, FfmpegEvent, FfmpegProgress},
};
use std::sync::atomic::Ordering;

/// Check if FFmpeg is installed and available in the system PATH.
///
/// This function verifies that FFmpeg is properly installed and can be
/// executed from the command line, which is required for FFmpeg-based
/// MP4 track combining operations.
///
/// # Returns
///
/// `true` if FFmpeg is installed and available, `false` otherwise.
///
/// # Examples
///
/// ```no_run
/// use recorder::is_installed;
///
/// if is_installed() {
///     println!("FFmpeg is available for MP4 creation");
/// } else {
///     println!("FFmpeg is not available, using built-in MP4 writer");
/// }
/// ```
pub fn is_ffmpeg_installed() -> bool {
    ffmpeg_sidecar::command::ffmpeg_is_installed()
}

/// Combine H.264 video and WAV audio tracks into MP4 using FFmpeg.
///
/// This function uses FFmpeg to combine separate H.264 video and WAV audio files
/// into a standard MP4 container. It supports mixing multiple audio tracks
/// and provides real-time progress reporting.
///
/// # Arguments
///
/// * `config` - Configuration specifying input files and output parameters
/// * `progress_cb` - Callback function for progress reporting (0.0 to 1.0)
///
/// # Returns
///
/// `Ok(ProgressState)` indicating the final state of the operation,
/// or `Err(RecorderError)` if combination failed.
///
/// # Features
///
/// - H.264 video encoding with libx264
/// - AAC audio encoding with proper codec settings
/// - Audio mixing for multiple input sources
/// - Real-time progress monitoring
/// - Proper pixel format (yuv420p) for compatibility
/// - Automatic file existence checking
///
/// # Examples
///
/// ```no_run
/// use recorder::{MergeTracksConfig, FPS, ProgressState};
/// use std::sync::Arc;
/// use std::sync::atomic::AtomicBool;
///
/// let config = MergeTracksConfig {
///     h264_path: "video.h264".into(),
///     input_wav_path: Some("audio.wav".into()),
///     speaker_wav_path: Some("speaker.wav".into()),
///     output_path: "output.mp4".into(),
///     fps: FPS::Fps30,
///     stop_sig: Arc::new(AtomicBool::new(false)),
/// };
///
/// let result = recorder::merge_tracks(config, |progress| {
///     println!("Progress: {:.1}%", progress * 100.0);
/// });
///
/// match result {
///     Ok(ProgressState::Finished) => println!("MP4 file created successfully"),
///     Ok(ProgressState::Stopped) => println!("Operation was stopped"),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn merge_tracks(
    config: MergeTracksConfig,
    mut progress_cb: impl FnMut(f32),
) -> Result<ProgressState, RecorderError> {
    if !config.h264_path.exists() {
        return Err(RecorderError::Ffmpeg(format!(
            "No found h264 file `{}`",
            config.h264_path.display()
        )));
    }

    let mut total_duration = None;
    let mut cmd = FfmpegCommand::new();

    cmd.input(config.h264_path.display().to_string());

    if let Some(ref wav_path) = config.input_wav_path {
        if !wav_path.exists() {
            return Err(RecorderError::Ffmpeg(format!(
                "No found input WAV file `{}`",
                wav_path.display()
            )));
        }
        cmd.input(&wav_path.display().to_string());
    }

    if let Some(ref wav_path) = config.speaker_wav_path {
        if !wav_path.exists() {
            return Err(RecorderError::Ffmpeg(format!(
                "No found speaker WAV file `{}`",
                wav_path.display()
            )));
        }
        cmd.input(&wav_path.display().to_string());
    }

    cmd.args(&["-c:v", "copy"]);

    if config.input_wav_path.is_some() && config.speaker_wav_path.is_some() {
        cmd.args(&[
            "-filter_complex",
            if config.convert_input_wav_to_mono {
                "[1:a]pan=mono|c0=0.5*FL+0.5*FR[a1];[a1][2:a]amix=inputs=2:duration=longest[a]"
            } else {
                "[1:a][2:a]amix=inputs=2:duration=longest[a]"
            },
            "-map",
            "0:v",
            "-map",
            "[a]",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
        ]);
    } else if config.input_wav_path.is_some() {
        if config.convert_input_wav_to_mono {
            cmd.args(&[
                "-filter_complex",
                "[1:a]pan=mono|c0=0.5*FL+0.5*FR[a]",
                "-map",
                "0:v",
                "-map",
                "[a]",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
            ]);
        } else {
            cmd.args(&["-map", "0:v", "-map", "1:a", "-c:a", "aac", "-b:a", "128k"]);
        }
    } else if config.speaker_wav_path.is_some() {
        cmd.args(&["-map", "0:v", "-map", "1:a", "-c:a", "aac", "-b:a", "128k"]);
    } else {
        unimplemented!();
    }

    let mut child_process = cmd
        .overwrite()
        .output(config.output_path.display().to_string())
        .print_command()
        .spawn()
        .map_err(|e| RecorderError::Ffmpeg(format!("ffmpeg spawn child process failed. {e}")))?;

    let iter = child_process
        .iter()
        .map_err(|e| RecorderError::Ffmpeg(format!("ffmpeg iter failed. {e}")))?;

    let start_time = std::time::Instant::now();

    for event in iter.into_iter() {
        if config.stop_sig.load(Ordering::Relaxed) {
            if let Err(e) = child_process.kill() {
                log::warn!("Failed to kill ffmpeg process after stopped. {e}");
            } else {
                log::info!("Exit ffmpeg child process after cancelled");
            }

            return Ok(ProgressState::Stopped);
        }

        match event {
            FfmpegEvent::ParsedDuration(FfmpegDuration { duration, .. }) => {
                total_duration = Some((duration * 1000.0) as u64);
            }
            FfmpegEvent::Progress(FfmpegProgress { time, .. }) => match timestamp_to_ms(&time) {
                Ok(ms) if ms > 0 => {
                    if let Some(duration) = total_duration {
                        progress_cb(ms as f32 / duration as f32);
                    }
                }
                Err(e) => log::warn!("{e}"),
                _ => (),
            },
            _ => (),
        }
    }

    if let Err(e) = child_process.kill() {
        log::warn!("Failed to kill ffmpeg process after finishing iteration: {e}");
    } else {
        log::info!(
            "Exit ffmpeg child process after finishing iteration. spent: {:.2?}",
            start_time.elapsed()
        );
    }

    progress_cb(1.0);

    Ok(ProgressState::Finished)
}

/// Convert FFmpeg timestamp format to milliseconds.
///
/// FFmpeg uses a specific timestamp format (HH:MM:SS.mmm) for progress
/// reporting. This function converts that format to milliseconds for
/// easier progress calculation.
///
/// # Arguments
///
/// * `timestamp` - FFmpeg timestamp string in format "HH:MM:SS.mmm"
///
/// # Returns
///
/// `Ok(u64)` containing the timestamp in milliseconds,
/// or `Err(RecorderError)` if the timestamp format is invalid.
///
/// # Examples
///
/// ```
/// use recorder::timestamp_to_ms;
///
/// let ms = timestamp_to_ms("00:01:30.500").unwrap();
/// assert_eq!(ms, 90500); // 1 minute 30.5 seconds = 90500 ms
/// ```
fn timestamp_to_ms(timestamp: &str) -> Result<u64, RecorderError> {
    let parts: Vec<&str> = timestamp.split(':').collect();
    if parts.len() != 3 {
        return Err(RecorderError::Other(format!(
            "Invalid timestamp format: {timestamp}"
        )));
    }

    let seconds_parts: Vec<&str> = parts[2].split('.').collect();
    if seconds_parts.len() != 2 {
        return Err(RecorderError::Other(format!(
            "Invalid seconds format: {:?}",
            seconds_parts
        )));
    }

    let hours: u64 = parts[0]
        .parse()
        .map_err(|e| RecorderError::Other(format!("Invalid hours {e}")))?;
    let minutes: u64 = parts[1]
        .parse()
        .map_err(|e| RecorderError::Other(format!("Invalid minutes {e}")))?;
    let seconds: u64 = seconds_parts[0]
        .parse()
        .map_err(|e| RecorderError::Other(format!("Invalid seconds {e}")))?;
    let milliseconds: u64 = seconds_parts[1]
        .parse()
        .map_err(|e| RecorderError::Other(format!("Invalid milliseconds {e}")))?;

    let total_ms = (hours * 3600 + minutes * 60 + seconds) * 1000 + milliseconds;

    Ok(total_ms)
}
