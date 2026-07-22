use crate::model::ModelConfig;
use ort::{
    memory::Allocator,
    value::{DynTensor, DynValue, Tensor, TensorElementType},
};

/// KV cache for the decoder, holding both convolution and attention caches.
pub struct KvCache {
    /// Conv layer caches: `past_conv.{i}` → [1, hidden_size, 3]
    pub conv_caches: Vec<DynValue>,
    /// Attention layer caches: `past_key_values.{i}.key` and `.value`
    pub attn_caches: Vec<(DynValue, DynValue)>,
}

/// Convert a DynValue (f32 tensor) to an owned copy by extracting and recreating it.
fn copy_f32_tensor(value: &DynValue) -> crate::Result<DynValue> {
    let (shape, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|e| crate::Error::Inference(format!("Failed to copy tensor: {:?}", e)))?;
    let shape_vec: Vec<i64> = shape.iter().copied().collect();

    if shape_vec.iter().any(|&d| d == 0) {
        let shape_usize: Vec<usize> = shape_vec.iter().map(|&d| d as usize).collect();
        let tensor = DynTensor::new(
            &Allocator::default(),
            TensorElementType::Float32,
            shape_usize,
        )?;
        return Ok(tensor.into());
    }

    let data_vec: Vec<f32> = data.to_vec();
    let tensor: Tensor<f32> = Tensor::from_array((shape_vec, data_vec.into_boxed_slice()))?;
    Ok(tensor.upcast().into())
}

impl KvCache {
    /// Initialize an empty KV cache.
    pub fn new(config: &ModelConfig) -> crate::Result<Self> {
        let mut conv_caches = Vec::with_capacity(config.conv_layers.len());
        let mut attn_caches = Vec::with_capacity(config.attn_layers.len());

        for _ in &config.conv_layers {
            let data = vec![0.0f32; config.hidden_size * 3];
            let tensor: Tensor<f32> = Tensor::from_array((
                vec![1i64, config.hidden_size as i64, 3i64],
                data.into_boxed_slice(),
            ))?;
            conv_caches.push(tensor.upcast().into());
        }

        for _ in &config.attn_layers {
            let key = DynTensor::new(
                &Allocator::default(),
                TensorElementType::Float32,
                vec![1, config.num_kv_heads, 0, config.head_dim],
            )?;
            let value = DynTensor::new(
                &Allocator::default(),
                TensorElementType::Float32,
                vec![1, config.num_kv_heads, 0, config.head_dim],
            )?;
            attn_caches.push((key.into(), value.into()));
        }

        Ok(Self {
            conv_caches,
            attn_caches,
        })
    }

    /// Build the named inputs for the decoder from the current cache state.
    pub fn to_inputs(&self, config: &ModelConfig) -> Vec<(String, DynValue)> {
        let mut inputs =
            Vec::with_capacity(config.conv_layers.len() + config.attn_layers.len() * 2);

        for (i, layer_idx) in config.conv_layers.iter().enumerate() {
            if let Ok(copy) = copy_f32_tensor(&self.conv_caches[i]) {
                inputs.push((format!("past_conv.{}", layer_idx), copy));
            }
        }

        for (i, layer_idx) in config.attn_layers.iter().enumerate() {
            let (key, value) = &self.attn_caches[i];
            if let Ok(copy_key) = copy_f32_tensor(key) {
                inputs.push((format!("past_key_values.{}.key", layer_idx), copy_key));
            }
            if let Ok(copy_value) = copy_f32_tensor(value) {
                inputs.push((format!("past_key_values.{}.value", layer_idx), copy_value));
            }
        }

        inputs
    }

    /// Update the cache from decoder outputs.
    pub fn update_from_outputs(
        &mut self,
        outputs: &ort::session::SessionOutputs,
        config: &ModelConfig,
    ) {
        for (i, layer_idx) in config.conv_layers.iter().enumerate() {
            if let Some(value) = outputs.get(&format!("present_conv.{}", layer_idx)) {
                if let Ok(copy) = copy_f32_tensor(value) {
                    self.conv_caches[i] = copy;
                }
            }
        }

        for (i, layer_idx) in config.attn_layers.iter().enumerate() {
            if let (Some(key), Some(value)) = (
                outputs.get(&format!("present.{}.key", layer_idx)),
                outputs.get(&format!("present.{}.value", layer_idx)),
            ) {
                if let (Ok(copy_key), Ok(copy_value)) =
                    (copy_f32_tensor(key), copy_f32_tensor(value))
                {
                    self.attn_caches[i] = (copy_key, copy_value);
                }
            }
        }
    }
}
