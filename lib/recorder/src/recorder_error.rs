use thiserror::Error;

/// Error types for recording operations.
///
/// This enum represents all possible errors that can occur during
/// screen recording, video encoding, audio recording, and file operations.
/// It uses the `thiserror` crate to provide automatic error formatting
/// and conversion from other error types.
///
/// # Examples
///
/// ```no_run
/// use recorder::{RecordingSession, RecorderError};
///
/// match RecordingSession::init("eDP-1") {
///     Ok(()) => println!("Initialization successful"),
///     Err(RecorderError::CaptureFailed(e)) => eprintln!("Capture failed: {}", e),
///     Err(RecorderError::VideoEncodingFailed(msg)) => eprintln!("Encoding failed: {}", msg),
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// ```
#[derive(Error, Debug)]
pub enum RecorderError {
    /// Screenshot capture failed
    #[error("Screenshot capture failed: {0}")]
    CaptureFailed(#[from] capture::Error),

    /// Image processing failed (resizing, format conversion, etc.)
    #[error("Image processing failed: {0}")]
    ImageProcessingFailed(String),

    /// Video encoding to H.264 format failed
    #[error("Video encoding failed: {0}")]
    VideoEncodingFailed(String),

    /// Video decoding from H.264 format failed
    #[error("Video decoding failed: {0}")]
    VideoDecodingFailed(String),

    /// File operation failed (reading, writing, creating files)
    #[error("File operation failed: {0}")]
    FileOperationFailed(#[from] std::io::Error),

    /// Invalid configuration parameters provided
    #[error("Invalid configuration parameters: {0}")]
    InvalidConfig(String),

    /// Crossbeam channel queue operation failed
    #[error("Queue operation failed: {0}")]
    QueueError(String),

    /// Input audio recording failed
    #[error("Audio recording failed: {0}")]
    AudioError(String),

    /// Speaker output recording failed
    #[error("Speaker recording failed: {0}")]
    SpeakerError(String),

    #[error("Denoise failed: {0}")]
    DenoiseError(String),

    /// FFmpeg operation failed during track combining
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),

    /// Other unspecified error
    #[error("{0}")]
    Other(String),
}
