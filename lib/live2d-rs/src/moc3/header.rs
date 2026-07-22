use crate::{Error, Result};

const FORMAT: &str = "moc3";
const HEADER_SIZE: usize = 64;
const MAGIC: &[u8; 4] = b"MOC3";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Moc3Header {
    version: Moc3Version,
    endianness: Endianness,
}

impl Moc3Header {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(invalid_header("header is shorter than 64 bytes"));
        }

        if &bytes[0..4] != MAGIC {
            return Err(invalid_header("magic must be MOC3"));
        }

        let version = Moc3Version::from_raw(bytes[4]).ok_or_else(|| Error::UnsupportedVersion {
            format: FORMAT,
            version: bytes[4] as u32,
        })?;
        let endianness = Endianness::from_raw(bytes[5])?;

        Ok(Self {
            version,
            endianness,
        })
    }

    pub fn version(&self) -> Moc3Version {
        self.version
    }

    pub fn endianness(&self) -> Endianness {
        self.endianness
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Moc3Version {
    V3_0_0,
    V3_3_0,
    V4_0_0,
    V4_2_0,
    V5_0_0,
    V5_3_0,
}

impl Moc3Version {
    fn from_raw(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::V3_0_0),
            2 => Some(Self::V3_3_0),
            3 => Some(Self::V4_0_0),
            4 => Some(Self::V4_2_0),
            5 => Some(Self::V5_0_0),
            6 => Some(Self::V5_3_0),
            _ => None,
        }
    }

    pub(crate) fn count_info_word_count(self) -> usize {
        match self {
            Self::V3_0_0 | Self::V3_3_0 | Self::V4_0_0 => 23,
            Self::V4_2_0 => 32,
            Self::V5_0_0 | Self::V5_3_0 => 35,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

impl Endianness {
    fn from_raw(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Little),
            1 => Ok(Self::Big),
            _ => Err(invalid_header("endianness flag must be 0 or 1")),
        }
    }
}

fn invalid_header(message: impl Into<String>) -> Error {
    Error::InvalidMoc3 {
        message: message.into(),
    }
}
