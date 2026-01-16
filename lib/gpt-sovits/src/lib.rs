mod model;
mod sampler;
mod sovits;
mod text;

pub use futures::{Stream, StreamExt};
pub use sampler::*;
pub use sovits::*;
pub use text::*;

pub const OUTPUT_AUDIO_CHANNEL: u16 = 1;
pub const OUTPUT_AUDIO_SAMPLE_RATE: u32 = 32_000;
pub const REFERENCE_AUDIO_SAMPLE_RATE: u32 = 16_000;

pub type Result<T> = std::result::Result<T, GSVError>;

#[derive(Debug, thiserror::Error)]
pub enum GSVError {
    #[error(transparent)]
    Box(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("decoder failed: {0}")]
    Decoder(#[from] rodio::decoder::DecoderError),

    #[error("failed to decode output audio token")]
    DecodeTokenFailed,

    #[error("no phonemes or BERT features could be generated for text: {0}")]
    GeneratePhonemesOrBertFeaturesFailed(String),

    #[error("input data is empty")]
    InputEmpty,

    #[error("internal error: {0}")]
    InternalError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Ort(#[from] ort::Error),

    #[error("parse error: {0}")]
    Pest(String),

    #[error(transparent)]
    Shape(#[from] ndarray::ShapeError),

    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),

    #[error(transparent)]
    RegexError(#[from] regex::Error),

    #[error("unknown rule 'all': {0:?}")]
    UnknownRuleAll(String),

    #[error("unknown rule 'ident': {0:?}")]
    UnknownRuleIdent(String),

    #[error("unknown rule 'word': {0:?}")]
    UnknownRuleWord(String),

    #[error("unknown Greek letter: {0:?}")]
    UnknownGreekLetter(String),

    #[error("unknown operator: {0:?}")]
    UnknownOperator(String),

    #[error("unknown flag: {0:?}")]
    UnknownFlag(String),

    #[error("unknown rule in percent: {0:?}")]
    UnknownRuleInPercent(String),

    #[error("unknown digit: {0:?}")]
    UnknownDigit(String),

    #[error("unknown rule in num: {0:?}")]
    UnknownRuleInNum(String),

    #[error("unknown rule in signs: {0:?}")]
    UnknownRuleInSigns(String),
}

impl<R> From<pest::error::Error<R>> for GSVError
where
    R: Copy + std::fmt::Debug + std::hash::Hash + Ord,
{
    fn from(value: pest::error::Error<R>) -> Self {
        Self::Pest(value.to_string())
    }
}

pub(crate) fn create_session(path: impl AsRef<std::path::Path>) -> Result<ort::session::Session> {
    Ok(ort::session::Session::builder()?
        .with_prepacking(true)?
        .with_config_entry("session.enable_mem_reuse", "1")?
        .with_independent_thread_pool()?
        .with_intra_op_spinning(true)?
        .commit_from_file(path)?)
}
