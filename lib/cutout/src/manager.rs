use crate::CutoutError;
use ndarray::Array;
use ort::{
    session::{
        Session,
        builder::{GraphOptimizationLevel, SessionBuilder},
    },
    value::Value,
};
use std::path::Path;

pub struct ModelManager {
    session: Session,
}

impl ModelManager {
    /// Create a new model manager from an ONNX model file path.
    pub fn from_file(model_path: &Path) -> Result<Self, CutoutError> {
        let session = SessionBuilder::new()
            .map_err(|e| CutoutError::OnnxError(e.into()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| CutoutError::OnnxError(e.into()))?
            .with_intra_threads(4)
            .map_err(|e| CutoutError::OnnxError(e.into()))?
            .commit_from_file(model_path)?;

        Ok(Self { session })
    }

    /// Run inference on preprocessed input
    pub fn run_inference(
        &mut self,
        input: &ndarray::Array4<f32>,
    ) -> Result<ndarray::Array4<f32>, CutoutError> {
        let shape = input.shape().to_vec();
        let data: Vec<f32> = input.iter().copied().collect();

        let input_tensor = Value::from_array((shape.as_slice(), data))?;

        let input_name: String = self.session.inputs()[0].name().to_string();
        let output_name: String = self.session.outputs()[0].name().to_string();

        let outputs = self
            .session
            .run(ort::inputs![&input_name => input_tensor])?;

        let output = outputs
            .get(&output_name)
            .ok_or_else(|| CutoutError::TensorError("No output from model".to_string()))?;

        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| CutoutError::TensorError(format!("Failed to extract tensor: {}", e)))?;

        let shape_vec: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();

        let output_array =
            Array::from_shape_vec(shape_vec.as_slice(), data.to_vec()).map_err(|e| {
                CutoutError::TensorError(format!("Failed to create output array: {}", e))
            })?;

        let output_shape = output_array.shape();
        let output_4d = if output_shape.len() == 4 {
            output_array.into_dimensionality()?
        } else if output_shape.len() == 3 {
            output_array
                .insert_axis(ndarray::Axis(0))
                .into_dimensionality()?
        } else if output_shape.len() == 2 {
            output_array
                .insert_axis(ndarray::Axis(0))
                .insert_axis(ndarray::Axis(0))
                .into_dimensionality()?
        } else {
            return Err(CutoutError::TensorError(format!(
                "Unexpected output shape: {:?}",
                output_shape
            )));
        };

        Ok(output_4d)
    }
}

