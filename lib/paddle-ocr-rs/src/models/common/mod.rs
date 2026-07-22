pub mod modules;
pub mod generate;
pub mod sample;


use candle_core::Tensor;

/// Multimodal model feature data
#[derive(Clone, Debug)]
pub struct MultiModalData {
    pub data_vec: Vec<Option<Tensor>>,
}

impl MultiModalData {
    pub fn new(data_vec: Vec<Option<Tensor>>) -> Self {
        Self { data_vec }
    }
}

/// Inference model trait
pub trait InferenceModel {
    /// Initial forward pass (considering multimodal input)
    fn forward_initial(
        &mut self,
        input_ids: &Tensor,
        seqlen_offset: usize,
        _data: MultiModalData,
    ) -> Result<Tensor, crate::Error> {
        Self::forward_step(self, input_ids, seqlen_offset)
    }

    /// Subsequent forward pass (autoregressive step)
    fn forward_step(&mut self, input_ids: &Tensor, seqlen_offset: usize) -> Result<Tensor, crate::Error>;

    /// Clear KV cache
    fn clear_cache(&mut self);

    /// Get stop token IDs
    fn stop_token_ids(&self) -> Vec<u32>;
}