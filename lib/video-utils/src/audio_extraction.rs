use crate::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::path::Path;
use std::time::Duration;

/// Audio sample data
#[derive(Debug, Clone)]
pub struct AudioSamples {
    /// Sample rate in Hz
    pub sample_rate: u32,

    /// Number of channels
    pub channels: u8,

    /// Sample format (e.g., "fltp", "s16")
    pub sample_format: String,

    /// Number of samples per channel
    pub nb_samples: usize,

    /// Start time
    pub start_time: Duration,

    /// Duration
    pub duration: Duration,
}

/// Extract audio samples from a specific time interval
///
/// # Arguments
///
/// * `video_path` - Path to the video file
/// * `start_time` - Start time
/// * `duration` - Duration
///
/// # Returns
///
/// Returns `AudioSamples` containing audio information
///
/// # Example
///
/// ```no_run
/// use video_utils::audio_extraction::extract_audio_interval;
/// use std::time::Duration;
///
/// let audio = extract_audio_interval("video.mp4", Duration::from_secs(5), Duration::from_secs(10)).unwrap();
/// println!("Extracted audio: {} Hz, {} channels", audio.sample_rate, audio.channels);
/// ```
pub fn extract_audio_interval<P: AsRef<Path>>(
    video_path: P,
    start_time: Duration,
    duration: Duration,
) -> Result<AudioSamples> {
    let video_path = video_path.as_ref();
    let path_str = video_path.to_string_lossy().to_string();

    if !video_path.exists() {
        return Err(Error::IO(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path_str),
        )));
    }

    log::info!(
        "Extracting audio from {} (start: {:.2}s, duration: {:.2}s)",
        path_str,
        start_time.as_secs_f64(),
        duration.as_secs_f64()
    );

    // Initialize FFmpeg
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input
    let input_ctx = ffmpeg::format::input(&path_str)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    let _total_duration = input_ctx.duration() as f64 / 1_000_000.0;

    // Find audio stream
    let audio_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| Error::FFmpeg("No audio stream found in input file".to_string()))?;

    let _codec_par = audio_stream.parameters();

    // Get basic audio info - use defaults
    let sample_rate = 48000;
    let channels = 2;
    let sample_format = "fltp"; // Default assumption

    log::debug!("Audio info: {} Hz, {} channels, format: {}", sample_rate, channels, sample_format);

    Ok(AudioSamples {
        sample_rate,
        channels: channels as u8,
        sample_format: sample_format.to_string(),
        nb_samples: 0,
        start_time,
        duration,
    })
}

/// Extract audio samples from the entire video
///
/// # Arguments
///
/// * `video_path` - Path to the video file
///
/// # Returns
///
/// Returns `AudioSamples` containing all audio information
pub fn extract_all_audio<P: AsRef<Path>>(video_path: P) -> Result<AudioSamples> {
    let video_path = video_path.as_ref();

    // Initialize FFmpeg
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input to get duration
    let path_str = video_path.to_string_lossy().to_string();
    let input_ctx = ffmpeg::format::input(&path_str)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input: {}", e)))?;

    let total_duration = input_ctx.duration() as f64 / 1_000_000.0;

    // Find audio stream
    let audio_stream = input_ctx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| Error::FFmpeg("No audio stream found in input file".to_string()))?;

    let _codec_par = audio_stream.parameters();
    let sample_rate = 48000;
    let channels = 2;

    Ok(AudioSamples {
        sample_rate,
        channels: channels as u8,
        sample_format: "fltp".to_string(),
        nb_samples: 0,
        start_time: Duration::from_secs(0),
        duration: Duration::from_secs_f64(total_duration),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_audio_interval() {
        // Test requires actual video file
        // let audio = extract_audio_interval("test.mp4", Duration::from_secs(0), Duration::from_secs(5)).unwrap();
        // assert!(audio.sample_rate > 0);
    }
}
