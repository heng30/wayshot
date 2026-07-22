use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    EmptyId,
    InvalidJson {
        format: &'static str,
        message: String,
    },
    InvalidMoc3 {
        message: String,
    },
    UnsupportedVersion {
        format: &'static str,
        version: u32,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("id cannot be empty"),
            Self::InvalidJson { format, message } => {
                write!(formatter, "invalid {format}: {message}")
            }
            Self::InvalidMoc3 { message } => write!(formatter, "invalid moc3: {message}"),
            Self::UnsupportedVersion { format, version } => {
                write!(formatter, "unsupported {format} version {version}")
            }
        }
    }
}

impl std::error::Error for Error {}
