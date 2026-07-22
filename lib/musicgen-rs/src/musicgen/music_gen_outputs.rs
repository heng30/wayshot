use crate::musicgen::logits::Logits;
use ort::{session::SessionOutputs, value::DynValue};

pub struct MusicGenOutputs<'r> {
    outputs: SessionOutputs<'r>,
}

impl<'r> MusicGenOutputs<'r> {
    pub fn new(outputs: SessionOutputs<'r>) -> Self {
        Self { outputs }
    }

    pub fn take_logits(&mut self) -> ort::Result<Logits> {
        Logits::from_3d_dyn_value(&self.outputs.remove("logits").unwrap())
    }

    pub fn take_present_decoder_key(&mut self, i: usize) -> DynValue {
        let key = format!("present.{i}.decoder.key");
        self.outputs
            .remove(key.as_str())
            .unwrap_or_else(|| panic!("{key} was already taken from outputs"))
    }

    pub fn take_present_decoder_value(&mut self, i: usize) -> DynValue {
        let value = format!("present.{i}.decoder.value");
        self.outputs
            .remove(value.as_str())
            .unwrap_or_else(|| panic!("{value} was already taken from outputs"))
    }

    pub fn take_present_encoder_key(&mut self, i: usize) -> DynValue {
        let key = format!("present.{i}.encoder.key");
        self.outputs
            .remove(key.as_str())
            .unwrap_or_else(|| panic!("{key} was already taken from outputs"))
    }

    pub fn take_present_encoder_value(&mut self, i: usize) -> DynValue {
        let value = format!("present.{i}.encoder.value");
        self.outputs
            .remove(value.as_str())
            .unwrap_or_else(|| panic!("{value} was already taken from outputs"))
    }
}
