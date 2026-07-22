pub mod cutout;
pub mod manager;
pub mod model;

#[derive(thiserror::Error, Debug)]
pub enum CutoutError {
    #[error("ONNX Runtime error: {0}")]
    OnnxError(#[from] ort::Error),

    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("Shape error: {0}")]
    ShapeError(#[from] ndarray::ShapeError),

    #[error("Preprocessing error: {0}")]
    PreprocessingError(String),

    #[error("Tensor error: {0}")]
    TensorError(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

