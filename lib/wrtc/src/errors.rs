use {
    audiopus::error::Error as OpusError,
    fdk_aac::enc::EncoderError as AacEncoderError,
    std::num::ParseIntError,
    webrtc::{error::Error as RTCError, util::Error as RTCUtilError},
};

#[derive(Debug)]
pub struct WebRTCError {
    pub value: WebRTCErrorValue,
}

#[derive(Debug, thiserror::Error)]
pub enum WebRTCErrorValue {
    #[error("webrtc error: {0}")]
    RTCError(#[from] RTCError),
    #[error("webrtc util error: {0}")]
    RTCUtilError(#[from] RTCUtilError),
    #[error("parse int error: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("cannot get local description")]
    CanNotGetLocalDescription,
    #[error("opus2aac error")]
    Opus2AacError,
    #[error("missing whitespace")]
    MissingWhitespace,
    #[error("missing colon")]
    MissingColon,
}

impl From<RTCError> for WebRTCError {
    fn from(error: RTCError) -> Self {
        WebRTCError {
            value: WebRTCErrorValue::RTCError(error),
        }
    }
}

impl From<RTCUtilError> for WebRTCError {
    fn from(error: RTCUtilError) -> Self {
        WebRTCError {
            value: WebRTCErrorValue::RTCUtilError(error),
        }
    }
}

impl From<ParseIntError> for WebRTCError {
    fn from(error: ParseIntError) -> Self {
        WebRTCError {
            value: WebRTCErrorValue::ParseIntError(error),
        }
    }
}

impl std::fmt::Display for WebRTCError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.value, f)
    }
}

impl std::error::Error for WebRTCError {}

#[derive(Debug)]
pub struct Opus2AacError {
    pub value: Opus2AacErrorValue,
}

impl std::fmt::Display for Opus2AacError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.value {
            Opus2AacErrorValue::OpusError(err) => write!(f, "opus error: {}", err),
            Opus2AacErrorValue::AacEncoderError(err) => write!(f, "aac encoder error: {}", err),
        }
    }
}

impl std::error::Error for Opus2AacError {}

#[derive(Debug)]
pub enum Opus2AacErrorValue {
    OpusError(OpusError),
    AacEncoderError(AacEncoderError),
}

impl From<OpusError> for Opus2AacError {
    fn from(error: OpusError) -> Self {
        Opus2AacError {
            value: Opus2AacErrorValue::OpusError(error),
        }
    }
}

impl From<AacEncoderError> for Opus2AacError {
    fn from(error: AacEncoderError) -> Self {
        Opus2AacError {
            value: Opus2AacErrorValue::AacEncoderError(error),
        }
    }
}
