mod chat;
mod request;
mod response;

pub use chat::{Chat, ChatConfig};
pub use request::{APIConfig, HistoryChat};
pub use response::StreamTextItem;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request Error {0}")]
    Request(#[from] reqwest::Error),
}
