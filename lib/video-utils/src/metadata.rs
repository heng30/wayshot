use crate::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::path::Path;

/// Video file metadata
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    /// File path
    pub path: String,

    /// Format name (e.g., "mov,mp4,m4a,3gp,3g2,mj2")
    pub format_name: String,

    /// Duration in seconds
    pub duration: f64,

    /// Total bitrate in bits per second
    pub bitrate: u64,

    /// File size in bytes
    pub size: u64,

    /// Video streams count
    pub video_streams_count: usize,

    /// Audio streams count
    pub audio_streams_count: usize,
}

/// Get metadata for a video file
pub fn get_metadata<P: AsRef<Path>>(path: P) -> Result<VideoMetadata> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy().to_string();

    if !path.exists() {
        return Err(Error::IO(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path_str),
        )));
    }

    // Get file size
    let size = std::fs::metadata(path)?.len();

    // Initialize FFmpeg
    ffmpeg::init().map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    // Open input file
    let input_ctx = ffmpeg::format::input(&path_str)
        .map_err(|e| Error::FFmpeg(format!("Failed to open input file: {}", e)))?;

    // Get basic info
    let format = input_ctx.format();
    let format_name = format.name().to_string();
    let duration = input_ctx.duration() as f64 / 1_000_000.0; // microseconds to seconds
    let bitrate = input_ctx.bit_rate() as u64;

    // Count streams
    let video_streams_count = if input_ctx.streams().best(ffmpeg::media::Type::Video).is_some() {
        1
    } else {
        0
    };

    let audio_streams_count = if input_ctx.streams().best(ffmpeg::media::Type::Audio).is_some() {
        1
    } else {
        0
    };

    Ok(VideoMetadata {
        path: path_str,
        format_name,
        duration,
        bitrate,
        size,
        video_streams_count,
        audio_streams_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_metadata_exists() {
        // Test requires actual video file
        // let metadata = get_metadata("test.mp4").unwrap();
        // assert!(metadata.duration > 0.0);
    }
}
