pub mod common;
pub mod session;
pub mod webrtc;
pub mod whep;

#[derive(Clone)]
pub enum PacketData {
    Video {
        timestamp: u32,
        data: bytes::BytesMut,
    },
    Audio {
        timestamp: u32,
        data: bytes::BytesMut,
    },
}

#[derive(Clone, Debug)]
pub enum Event {
    LocalClosed(String),
    PeerClosed(String),
    PeerConnected(String),
    PeerConnecting(String),
}

pub type PacketDataSender = tokio::sync::broadcast::Sender<PacketData>;
pub type PacketDataReceiver = tokio::sync::broadcast::Receiver<PacketData>;

pub type EventSender = tokio::sync::broadcast::Sender<Event>;
pub type EventReceiver = tokio::sync::broadcast::Receiver<Event>;

#[derive(Debug, thiserror::Error)]
pub enum WebRTCError {
    #[error("webrtc error: {0}")]
    RTCError(#[from] ::webrtc::error::Error),

    #[error("webrtc util error: {0}")]
    RTCUtilError(#[from] ::webrtc::util::Error),

    #[error("parse int error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("cannot get local description")]
    CanNotGetLocalDescription,

    #[error("missing whitespace")]
    MissingWhitespace,

    #[error("missing colon")]
    MissingColon,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("net io error: {0:?}")]
    BytesIOError(#[from] bytesio::errors::BytesIOError),

    #[error("bytes read error: {0:?}")]
    BytesReadError(#[from] bytesio::errors::BytesReadError),

    #[error("bytes write error: {0:?}")]
    BytesWriteError(#[from] bytesio::errors::BytesWriteError),

    #[error("Utf8Error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("webrtc error: {0}")]
    RTCError(#[from] ::webrtc::error::Error),

    #[error("Auth err: {0:?}")]
    AuthError(#[from] crate::common::auth::AuthError),

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
