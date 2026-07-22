use ort::{
    session::SessionInputs,
    value::{DynValue, Tensor},
};
use std::collections::HashMap;

pub struct MusicGenInputs {
    inputs: HashMap<String, DynValue>,
    pub use_cache_branch: bool,
}

impl MusicGenInputs {
    pub fn new() -> Self {
        Self {
            inputs: HashMap::new(),
            use_cache_branch: false,
        }
    }

    pub fn encoder_attention_mask(&mut self, v: DynValue) {
        self.inputs.insert("encoder_attention_mask".to_string(), v);
    }

    pub fn input_ids(&mut self, v: impl Into<DynValue>) {
        self.inputs.insert("input_ids".to_string(), v.into());
    }

    pub fn encoder_hidden_states(&mut self, v: DynValue) {
        self.inputs.insert("encoder_hidden_states".to_string(), v);
    }

    pub fn remove_encoder_hidden_states(&mut self) {
        self.inputs.remove("encoder_hidden_states");
    }

    pub fn past_key_value_decoder_key(&mut self, i: usize, v: DynValue) {
        self.inputs
            .insert(format!("past_key_values.{i}.decoder.key"), v);
    }

    pub fn past_key_value_decoder_value(&mut self, i: usize, v: DynValue) {
        self.inputs
            .insert(format!("past_key_values.{i}.decoder.value"), v);
    }

    pub fn past_key_value_encoder_key(&mut self, i: usize, v: DynValue) {
        self.inputs
            .insert(format!("past_key_values.{i}.encoder.key"), v);
    }

    pub fn past_key_value_encoder_value(&mut self, i: usize, v: DynValue) {
        self.inputs
            .insert(format!("past_key_values.{i}.encoder.value"), v);
    }

    pub fn use_cache_branch(&mut self, value: bool) {
        self.use_cache_branch = value;
        self.inputs.insert(
            "use_cache_branch".to_string(),
            Tensor::from_array(([1], vec![value])).unwrap().into_dyn(),
        );
    }

    /// Build `SessionInputs` using view references.
    ///
    /// Matches the original MusicGPT implementation:
    /// ```ignore
    /// SessionInputs::ValueMap(
    ///     self.inputs.iter()
    ///         .map(|(k, v)| (k.into(), v.view().into()))
    ///         .collect()
    /// )
    /// ```
    pub fn ort(&self) -> SessionInputs<'_, '_> {
        SessionInputs::ValueMap(
            self.inputs
                .iter()
                .map(|(k, v)| (k.to_string().into(), v.view().into()))
                .collect::<Vec<_>>(),
        )
    }
}
