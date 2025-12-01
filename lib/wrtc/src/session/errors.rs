#![allow(non_local_definitions)]
use streamhub::errors::StreamHubError;
use {
    bytesio::bytes_errors::BytesReadError,
    bytesio::{bytes_errors::BytesWriteError, bytesio_errors::BytesIOError},
    commonlib::errors::AuthError,
    std::str::Utf8Error,
    tokio::sync::oneshot::error::RecvError,
    webrtc::error::Error as RTCError,
};

#[derive(Debug)]
pub struct SessionError {
    pub value: SessionErrorValue,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionErrorValue {
    #[error("net io error: {0:?}")]
    BytesIOError(BytesIOError),
    #[error("bytes read error: {0:?}")]
    BytesReadError(BytesReadError),
    #[error("bytes write error: {0:?}")]
    BytesWriteError(BytesWriteError),
    #[error("Utf8Error: {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("event execute error: {0:?}")]
    ChannelError(StreamHubError),
    #[error("webrtc error: {0}")]
    RTCError(#[from] RTCError),
    #[error("tokio: oneshot receiver err: {0}")]
    RecvError(#[from] RecvError),
    #[error("Auth err: {0:?}")]
    AuthError(AuthError),
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

impl From<RTCError> for SessionError {
    fn from(error: RTCError) -> Self {
        SessionError {
            value: SessionErrorValue::RTCError(error),
        }
    }
}

impl From<BytesIOError> for SessionError {
    fn from(error: BytesIOError) -> Self {
        SessionError {
            value: SessionErrorValue::BytesIOError(error),
        }
    }
}

impl From<BytesReadError> for SessionError {
    fn from(error: BytesReadError) -> Self {
        SessionError {
            value: SessionErrorValue::BytesReadError(error),
        }
    }
}

impl From<BytesWriteError> for SessionError {
    fn from(error: BytesWriteError) -> Self {
        SessionError {
            value: SessionErrorValue::BytesWriteError(error),
        }
    }
}

impl From<Utf8Error> for SessionError {
    fn from(error: Utf8Error) -> Self {
        SessionError {
            value: SessionErrorValue::Utf8Error(error),
        }
    }
}

impl From<StreamHubError> for SessionError {
    fn from(error: StreamHubError) -> Self {
        SessionError {
            value: SessionErrorValue::ChannelError(error),
        }
    }
}

impl From<RecvError> for SessionError {
    fn from(error: RecvError) -> Self {
        SessionError {
            value: SessionErrorValue::RecvError(error),
        }
    }
}

impl From<AuthError> for SessionError {
    fn from(error: AuthError) -> Self {
        SessionError {
            value: SessionErrorValue::AuthError(error),
        }
    }
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.value, f)
    }
}

impl std::error::Error for SessionError {}
