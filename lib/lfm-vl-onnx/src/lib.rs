pub mod cache;
pub mod generate;
pub mod image;
pub mod model;
pub mod tokenizer;

pub use cache::KvCache;
pub use generate::generate;
pub use model::{LfmVlModel, ModelConfig, Precision};
pub use tokenizer::LfmTokenizer;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ONNX Runtime error: {0}")]
    Ort(#[from] ort::Error),

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Image processing error: {0}")]
    Image(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
