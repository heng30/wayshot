use thiserror::Error;

#[derive(Error, Debug)]
pub enum StemError {
    #[error("Inference error: {0}")]
    Inference(String),

    #[error("DSP error: {0}")]
    Dsp(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<ort::Error> for StemError {
    fn from(e: ort::Error) -> Self {
        StemError::Inference(e.to_string())
    }
}

impl From<ort::Error<ort::session::builder::SessionBuilder>> for StemError {
    fn from(e: ort::Error<ort::session::builder::SessionBuilder>) -> Self {
        StemError::Inference(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, StemError>;

