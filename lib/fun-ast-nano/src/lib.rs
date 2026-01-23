pub mod device;
pub mod model;
pub mod position_embed;
pub mod tokenizer;

pub const INPUT_AUDIO_CHANNELS: u32 = 1;
pub const INPUT_AUDIO_SAMPLE_RATE: u32 = 16_000;
pub const ENGLISH_PUNCTUATIONS: &[char] = &[',', '.', '!', '?'];
pub const CHINESE_PUNCTUATIONS: &[char] = &['，', '。', '！', '？'];

pub use audio_utils::vad::{AudioSegment, VadConfig, detect_speech_segments};
pub use hound::SampleFormat;
pub use model::{
    Model,
    fun_asr_nano::generate::{
        FunASRModelConfig, FunAsrNanoGenerateModel, SegmentInfo, StreamChunk, TranscriptionRequest,
        TranscriptionResponse, load_audio_file,
    },
};

pub type Result<T> = std::result::Result<T, FunAsrError>;

#[derive(thiserror::Error, Debug)]
pub enum FunAsrError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tensor error: {0}")]
    Tensor(#[from] candle_core::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Audio processing error: {0}")]
    Audio(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Transcribe cancelled")]
    TranscribeCancelled,
}

impl From<audio_utils::AudioProcessError> for FunAsrError {
    fn from(err: audio_utils::AudioProcessError) -> Self {
        match err {
            audio_utils::AudioProcessError::Audio(msg) => FunAsrError::Audio(msg),
            audio_utils::AudioProcessError::Io(e) => FunAsrError::Io(e),
            audio_utils::AudioProcessError::Candle(e) => FunAsrError::Tensor(e),
        }
    }
}

impl From<tensor_utils::TensorUtilsError> for FunAsrError {
    fn from(err: tensor_utils::TensorUtilsError) -> Self {
        match err {
            tensor_utils::TensorUtilsError::InvalidInput(msg) => FunAsrError::InvalidInput(msg),
            tensor_utils::TensorUtilsError::Candle(e) => FunAsrError::Tensor(e),
        }
    }
}
