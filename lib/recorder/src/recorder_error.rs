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
    #[error("Screenshot capture failed: {0}")]
    CaptureFailed(#[from] capture::Error),

    #[error("Image processing failed: {0}")]
    ImageProcessingFailed(String),

    #[error("Video encoding failed: {0}")]
    VideoEncodingFailed(String),

    #[error("Video decoding failed: {0}")]
    VideoDecodingFailed(String),

    #[error("File operation failed: {0}")]
    FileOperationFailed(#[from] std::io::Error),

    #[error("Invalid configuration parameters: {0}")]
    InvalidConfig(String),

    #[error("Queue operation failed: {0}")]
    QueueError(String),

    #[error("Audio recording failed: {0}")]
    AudioError(#[from] super::record_audio::AudioError),

    #[error("Speaker recording failed: {0}")]
    SpeakerError(#[from] super::record_speaker::SpeakerError),

    #[error("Audio mixer config builder failed: {0}")]
    AudioMixerConfigBuilderError(#[from] mp4m::audio_processor::AudioProcessorConfigBuilderError),

    #[error("Mp4 processor config builder failed: {0}")]
    Mp4ProcessorConfigBuilderError(#[from] mp4m::mp4_processor::Mp4ProcessorConfigBuilderError),

    #[error("Mp4 processor failed: {0}")]
    Mp4ProcessorError(#[from] mp4m::mp4_processor::Mp4ProcessorError),

    #[error("Denoise failed: {0}")]
    DenoiseError(String),

    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),

    #[error("{0}")]
    Other(String),
}
