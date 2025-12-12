#[macro_use]
extern crate derivative;

pub mod client;
pub mod common;
pub mod opus;
pub mod session;
pub mod whep;
pub mod wrtc;

pub use webrtc::ice_transport::ice_server::RTCIceServer;
pub use wrtc::{WebRTCServer, WebRTCServerConfig};

#[derive(Clone)]
pub enum PacketData {
    Video {
        timestamp: std::time::Instant,
        data: bytes::Bytes,
    },
    Audio {
        timestamp: std::time::Instant,
        duration: std::time::Duration,
        data: bytes::Bytes,
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

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Addr parse error: {0}")]
    AddrParseError(#[from] std::net::AddrParseError),

    #[error("cannot get local description")]
    CanNotGetLocalDescription,

    #[error("missing whitespace")]
    MissingWhitespace,

    #[error("missing colon")]
    MissingColon,

    #[error("TLS configuration error: {0}")]
    TlsConfigError(String),
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

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("H264 decoder error: {0}")]
    H264DecoderError(String),

    #[error("Opus coder error: {0}")]
    OpusCoderError(#[from] crate::opus::OpusCoderError),

    #[error("YUV to RGB conversion error: {0}")]
    YuvToRgbError(String),

    #[error("H264 data too short")]
    H264DataTooShort,

    #[error("Failed to decode any H264 frame from the input data")]
    H264DecodeFailed,

    #[error("WebRTC error: {0}")]
    WebRTCError(#[from] ::webrtc::error::Error),

    #[error("WebRTC util error: {0}")]
    WebRTCUtilError(#[from] ::webrtc::util::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("HTTP request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("SDP parse error: {0}")]
    SdpParseError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Channel send error: {0}")]
    ChannelError(String),

    #[error("Missing local description")]
    MissingLocalDescription,
}
