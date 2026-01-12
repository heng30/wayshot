pub mod model;
pub mod remover;

pub use model::Model;
pub use remover::BackgroundRemover;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Model file not found: {0}")]
    ModelNotFound(std::path::PathBuf),

    #[error("Failed to load model: {0}")]
    ModelLoadFailed(String),

    #[error("Invalid model output: {0}")]
    InvalidOutput(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(#[from] ort::Error),

    #[error("Image resize error: {0}")]
    ImageResize(#[from] fast_image_resize::ResizeError),

    #[error("Image buffer error: {0}")]
    ImageBufferError(#[from] fast_image_resize::ImageBufferError),

    #[error("{0}")]
    Generic(String),
}
