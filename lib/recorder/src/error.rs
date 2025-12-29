use thiserror::Error;

#[derive(Error, Debug)]
pub enum RecorderError {
    #[error("Screenshot capture failed: {0}")]
    CaptureFailed(#[from] screen_capture::ScreenCaptureError),

    #[error("Get screen info failed: {0}")]
    ScreenInfoFailed(#[from] screen_capture::ScreenInfoError),

    #[error("Image processing failed: {0}")]
    ImageProcessingFailed(String),

    #[error("Video encoding failed: {0}")]
    VideoEncodingFailed(#[from] video_encoder::EncoderError),

    #[error("Video decoding failed: {0}")]
    VideoDecodingFailed(String),

    #[error("File operation failed: {0}")]
    FileOperationFailed(#[from] std::io::Error),

    #[error("Invalid configuration parameters: {0}")]
    InvalidConfig(String),

    #[error("Queue operation failed: {0}")]
    QueueError(String),

    #[error("Audio recording failed: {0}")]
    AudioRecorderError(#[from] super::audio_recorder::AudioRecorderError),

    #[error("Speaker recording failed: {0}")]
    SpeakerRecorderError(#[from] super::speaker_recorder::SpeakerRecorderError),

    #[error("Audio mixer config builder failed: {0}")]
    AudioMixerConfigBuilderError(#[from] mp4m::audio_processor::AudioProcessorConfigBuilderError),

    #[error("Mp4 processor config builder failed: {0}")]
    Mp4ProcessorConfigBuilderError(#[from] mp4m::mp4_processor::Mp4ProcessorConfigBuilderError),

    #[error("Mp4 processor failed: {0}")]
    Mp4ProcessorError(#[from] mp4m::mp4_processor::Mp4ProcessorError),

    #[error("Rtmp Client Error failed: {0}")]
    RtmpClientError(#[from] srtmp::RtmpClientError),

    #[error("Denoise failed: {0}")]
    DenoiseError(String),

    #[error("{0}")]
    Other(String),

    #[error("Cursor tracker configuration error: {0}")]
    CursorTrackerConfigError(String),

    #[error("Cursor tracker channel error: {0}")]
    CursorTrackerChannelError(String),

    #[error("Cursor tracker validation error: {0}")]
    CursorTrackerValidationError(String),
}
