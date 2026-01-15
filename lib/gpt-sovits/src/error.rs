use {
    ndarray::ShapeError,
    ort::Error as OrtError,
    pest::error::Error as PestError,
    rodio::decoder::DecoderError,
    std::{
        error::Error,
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        hash::Hash,
        io::Error as IoError,
        time::SystemTimeError,
    },
};

macro_rules! format_error {
    ($f:expr, $name:expr, $msg:expr) => {
        write!($f, "{}Error: {}", $name, $msg)
    };
    ($f:expr, $name:expr, $fmt:expr, $($arg:tt)*) => {
        write!($f, concat!("{}Error: ", $fmt), $name, $($arg)*)
    };
}

#[derive(Debug)]
pub enum GSVError {
    Box(Box<dyn Error + Send + Sync>),
    Decoder(DecoderError),
    DecodeTokenFailed,
    GeneratePhonemesOrBertFeaturesFailed(String),
    InputEmpty,
    InternalError(String),
    Io(IoError),
    Ort(OrtError),
    Pest(String),
    Shape(ShapeError),
    SystemTime(SystemTimeError),
    UnknownRuleAll(String),
    UnknownRuleIdent(String),
    UnknownRuleWord(String),
    UnknownGreekLetter(String),
    UnknownOperator(String),
    UnknownFlag(String),
    UnknownRuleInPercent(String),
    UnknownDigit(String),
    UnknownRuleInNum(String),
    UnknownRuleInSigns(String),
}

impl Error for GSVError {}

impl Display for GSVError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Box(e) => Display::fmt(e, f),
            Self::Decoder(e) => Display::fmt(e, f),
            Self::Io(e) => Display::fmt(e, f),
            Self::Ort(e) => Display::fmt(e, f),
            Self::Shape(e) => Display::fmt(e, f),
            Self::SystemTime(e) => Display::fmt(e, f),
            Self::DecodeTokenFailed => {
                format_error!(f, "DecodeTokenFailed", "Can't decode output audio.")
            }
            Self::GeneratePhonemesOrBertFeaturesFailed(s) => {
                format_error!(
                    f,
                    "GeneratePhonemesOrBertFeaturesFailed",
                    "No phonemes or BERT features could be generated for the text: {}",
                    s
                )
            }
            Self::InputEmpty => {
                format_error!(f, "InputEmpty", "Input data is empty.")
            }
            Self::InternalError(s) => {
                format_error!(f, "Internal", "{}", s)
            }
            Self::Pest(s) => {
                format_error!(f, "Pest", "{}", s)
            }
            Self::UnknownRuleAll(s) => {
                format_error!(f, "UnknownRuleAll", "Unknown rule in all: {:?}", s)
            }
            Self::UnknownRuleIdent(s) => {
                format_error!(f, "UnknownRuleIdent", "Unknown rule in ident: {:?}", s)
            }
            Self::UnknownRuleWord(s) => {
                format_error!(f, "UnknownRuleWord", "Unknown rule in word: {:?}", s)
            }
            Self::UnknownGreekLetter(s) => {
                format_error!(f, "UnknownGreekLetter", "Unknown Greek letter: {:?}", s)
            }
            Self::UnknownOperator(s) => {
                format_error!(f, "UnknownOperator", "Unknown operator: {:?}", s)
            }
            Self::UnknownFlag(s) => {
                format_error!(f, "UnknownFlag", "Unknown flag: {:?}", s)
            }
            Self::UnknownRuleInPercent(s) => {
                format_error!(
                    f,
                    "UnknownRuleInPercent",
                    "Unknown rule in percent: {:?}",
                    s
                )
            }
            Self::UnknownDigit(s) => {
                format_error!(f, "UnknownDigit", "Unknown digit: {:?}", s)
            }
            Self::UnknownRuleInNum(s) => {
                format_error!(f, "UnknownRuleInNum", "Unknown rule in num: {:?}", s)
            }
            Self::UnknownRuleInSigns(s) => {
                format_error!(f, "UnknownRuleInSigns", "Unknown rule in signs: {:?}", s)
            }
        }
    }
}

impl From<OrtError> for GSVError {
    fn from(value: OrtError) -> Self {
        Self::Ort(value)
    }
}

impl From<IoError> for GSVError {
    fn from(value: IoError) -> Self {
        Self::Io(value)
    }
}

impl From<ShapeError> for GSVError {
    fn from(value: ShapeError) -> Self {
        Self::Shape(value)
    }
}

impl From<SystemTimeError> for GSVError {
    fn from(value: SystemTimeError) -> Self {
        Self::SystemTime(value)
    }
}

impl From<Box<dyn Error + Send + Sync>> for GSVError {
    fn from(value: Box<dyn Error + Send + Sync>) -> Self {
        Self::Box(value)
    }
}

impl<R> From<PestError<R>> for GSVError
where
    R: Copy + Debug + Hash + Ord,
{
    fn from(value: PestError<R>) -> Self {
        Self::Pest(format!("{}", value))
    }
}

impl From<DecoderError> for GSVError {
    fn from(value: DecoderError) -> Self {
        Self::Decoder(value)
    }
}
