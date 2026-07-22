pub mod api;
pub mod proto;
pub mod types;

pub use api::{get_all_danmaku, get_all_danmaku_with_limit, get_danmaku_segment, get_video_pages, DEFAULT_TIMEOUT};
pub use types::{DanmakuElem, VideoPage};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("protobuf decode error: {0}")]
    Proto(#[source] DecodeError),

    #[error("api error (code={code}): {message}")]
    Api { code: i64, message: String },

    #[error("no video page found")]
    NoPage,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError {
    #[error("unexpected end of data")]
    UnexpectedEof,

    #[error("varint too long")]
    VarintTooLong,

    #[error("unknown wire type {0}")]
    UnknownWireType(u8),
}

impl From<DecodeError> for Error {
    fn from(e: DecodeError) -> Self {
        Error::Proto(e)
    }
}
