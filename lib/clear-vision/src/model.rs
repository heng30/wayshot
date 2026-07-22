use ndarray::Array4;
use ort::session::{Session, builder::GraphOptimizationLevel};
use thiserror::Error;

const SWINIR_FILENAME: &str = "003_realSR_BSRGAN_DFO_s64w8_SwinIR-M_x4_GAN.onnx";
const SWINIR_URL: &str = "https://huggingface.co/rocca/swin-ir-onnx/resolve/main/003_realSR_BSRGAN_DFO_s64w8_SwinIR-M_x4_GAN.onnx";
const SWINIR_SIZE_BYTES: u64 = 18_000_000;

pub const TILE_SIZE: usize = 64;
pub const TILE_PAD: usize = 8;

#[derive(Clone, Copy, Debug)]
pub enum Model {
    SwinIR,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        vec![Self::SwinIR]
    }

    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::SwinIR => SWINIR_FILENAME,
        }
    }

    pub fn try_from_filename(filename: &str) -> Option<Self> {
        match filename {
            SWINIR_FILENAME => Some(Model::SwinIR),
            _ => None,
        }
    }

    pub fn try_from_url(url: &str) -> Option<Self> {
        match url {
            SWINIR_URL => Some(Model::SwinIR),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::SwinIR => SWINIR_URL,
        }
    }

    pub fn model_size_bytes(&self) -> u64 {
        match self {
            Self::SwinIR => SWINIR_SIZE_BYTES,
        }
    }
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Failed to create ONNX session: {0}")]
    SessionBuilder(String),
    #[error("Failed to load ONNX model from {path}")]
    LoadModel {
        #[source]
        source: ort::Error,
        path: String,
    },
    #[error("Failed to extract output tensor")]
    ExtractTensor(#[source] ort::Error),
    #[error("Failed to reshape output tensor to [N,C,H,W]")]
    ReshapeOutput(#[source] ndarray::ShapeError),
    #[error("Inference error")]
    Inference(#[source] ort::Error),
    #[error("Failed to create input value from array")]
    InputValue(#[source] ort::Error),
}

pub fn load_session(path: &std::path::Path) -> Result<Session, ModelError> {
    let session = Session::builder()
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .with_optimization_level(GraphOptimizationLevel::All)
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .with_intra_threads(0)
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .with_dimension_override("h", TILE_SIZE as i64)
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .with_dimension_override("w", TILE_SIZE as i64)
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .commit_from_file(path)
        .map_err(|e| ModelError::LoadModel {
            source: e,
            path: path.display().to_string(),
        })?;

    Ok(session)
}

pub fn run_inference(
    session: &mut Session,
    input_tensor: Array4<f32>,
) -> Result<Array4<f32>, ModelError> {
    let input_value =
        ort::value::Value::from_array(input_tensor).map_err(ModelError::InputValue)?;

    let outputs = session
        .run(ort::inputs![input_value])
        .map_err(ModelError::Inference)?;

    let output_value = &outputs[0];
    let (shape, data) = output_value
        .try_extract_tensor::<f32>()
        .map_err(ModelError::ExtractTensor)?;

    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    let owned_data: Vec<f32> = data.to_vec();

    ndarray::ArrayD::from_shape_vec(dims.clone(), owned_data)
        .map_err(ModelError::ReshapeOutput)?
        .into_shape_with_order((dims[0], dims[1], dims[2], dims[3]))
        .map_err(ModelError::ReshapeOutput)
}
