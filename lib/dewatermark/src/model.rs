use crate::pipeline::MODEL_INPUT_SIZE;
use ndarray::Array4;
use ort::session::Session;
use thiserror::Error;

const LAMA_FILENAME: &str = "lama_fp32.onnx";
const LAMA_URL: &str = "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx";
const LAMA_SIZE_BYTES: u64 = 208_000_000;

#[derive(Clone, Copy, Debug)]
pub enum Model {
    LaMa,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        vec![Self::LaMa]
    }

    pub fn to_input_size(&self) -> (u32, u32) {
        match self {
            Model::LaMa => (MODEL_INPUT_SIZE, MODEL_INPUT_SIZE),
        }
    }

    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::LaMa => LAMA_FILENAME,
        }
    }

    pub fn try_from_filename(filename: &str) -> Option<Self> {
        match filename {
            LAMA_FILENAME => Some(Model::LaMa),
            _ => None,
        }
    }

    pub fn try_from_url(url: &str) -> Option<Self> {
        match url {
            LAMA_URL => Some(Model::LaMa),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::LaMa => LAMA_URL,
        }
    }

    pub fn model_size_bytes(&self) -> u64 {
        match self {
            Self::LaMa => LAMA_SIZE_BYTES,
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

/// Load an ONNX session from the given model path.
///
/// Uses Level3 graph optimization and 4 intra-op threads for best throughput.
pub fn load_session(path: &std::path::Path) -> Result<Session, ModelError> {
    let session = Session::builder()
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .with_intra_threads(4)
        .map_err(|e| ModelError::SessionBuilder(e.to_string()))?
        .commit_from_file(path)
        .map_err(|e| ModelError::LoadModel {
            source: e,
            path: path.display().to_string(),
        })?;

    Ok(session)
}

/// Run inference with image [1,3,H,W] and mask [1,1,H,W] tensors.
///
/// Input names "image" and "mask" match the LaMa model expectations.
/// Returns the first output tensor reshaped to [1,3,H,W].
pub fn run_inference(
    session: &mut Session,
    image_tensor: Array4<f32>,
    mask_tensor: Array4<f32>,
) -> Result<Array4<f32>, ModelError> {
    let image_value =
        ort::value::Value::from_array(image_tensor).map_err(ModelError::InputValue)?;
    let mask_value = ort::value::Value::from_array(mask_tensor).map_err(ModelError::InputValue)?;

    let inputs = ort::inputs![
        "image" => image_value,
        "mask" => mask_value,
    ];

    let outputs = session.run(inputs).map_err(ModelError::Inference)?;

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
