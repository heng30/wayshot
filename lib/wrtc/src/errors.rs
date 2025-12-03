use audiopus::error::Error as OpusError;
use bytesio::errors::{BytesIOError, BytesReadError, BytesWriteError};
use commonlib::errors::AuthError;
use fdk_aac::enc::EncoderError as AacEncoderError;
use std::num::ParseIntError;
use std::str::Utf8Error;
use tokio::sync::oneshot::error::RecvError;
use webrtc::{error::Error as RTCError, util::Error as RTCUtilError};

#[derive(Debug, thiserror::Error)]
pub enum WebRTCError {
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

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("net io error: {0:?}")]
    BytesIOError(#[from] BytesIOError),

    #[error("bytes read error: {0:?}")]
    BytesReadError(#[from] BytesReadError),

    #[error("bytes write error: {0:?}")]
    BytesWriteError(#[from] BytesWriteError),

    #[error("Utf8Error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("webrtc error: {0}")]
    RTCError(#[from] RTCError),

    #[error("tokio: oneshot receiver err: {0}")]
    RecvError(#[from] RecvError),

    #[error("Auth err: {0:?}")]
    AuthError(#[from] AuthError),

    #[error("stream hub event send error")]
    StreamHubEventSendErr,

    #[error("cannot receive frame data from stream hub")]
    CannotReceiveFrameData,

    #[error("Http Request path error")]
    HttpRequestPathError,

    #[error("Not supported")]
    HttpRequestNotSupported,

    #[error("Empty sdp data")]
    HttpRequestEmptySdp,

    #[error("Cannot find Content-Length")]
    HttpRequestNoContentLength,

    #[error("Channel receive error")]
    ChannelRecvError,
}

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("no app name")]
    NoAppName,

    #[error("no stream name")]
    NoStreamName,

    #[error("no app or stream name")]
    NoAppOrStreamName,

    #[error("exists")]
    Exists,

    #[error("send error")]
    SendError,

    #[error("send video error")]
    SendVideoError,

    #[error("send audio error")]
    SendAudioError,

    #[error("bytes read error: {0}")]
    BytesReadError(#[from] BytesReadError),

    #[error("bytes write error: {0}")]
    BytesWriteError(#[from] BytesWriteError),

    #[error("not correct data sender type")]
    NotCorrectDataSenderType,

    #[error("the client session error: {0}")]
    RtspClientSessionError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Opus2AacError {
    #[error("opus error: {0}")]
    OpusError(#[from] OpusError),

    #[error("aac encoder error: {0}")]
    AacEncoderError(AacEncoderError),
}

impl From<AacEncoderError> for Opus2AacError {
    fn from(error: AacEncoderError) -> Self {
        Opus2AacError::AacEncoderError(error)
    }
}
