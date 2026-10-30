//! A single MiniCPM-4 decoder layer.

use crate::config::MiniCpm4Config;
use crate::minicpm4::attention::{LayerKv, MiniCpmAttention};
use crate::minicpm4::mlp::MiniCpmMlp;
use crate::minicpm4::MiniCpmRmsNorm;
use burn::prelude::*;

#[derive(Module, Debug)]
pub struct MiniCpmDecoderLayer {
    pub self_attn: MiniCpmAttention,
    pub mlp: MiniCpmMlp,
    pub input_layernorm: MiniCpmRmsNorm,
    pub post_attention_layernorm: MiniCpmRmsNorm,
    pub residual_scale: Option<f64>,
}

impl MiniCpmDecoderLayer {
    pub fn new(config: &MiniCpm4Config, device: &Device) -> Self {
        let residual_scale = config
            .use_mup
            .then(|| config.scale_depth as f64 / (config.num_hidden_layers as f64).sqrt());
        Self {
            self_attn: MiniCpmAttention::new(config, device),
            mlp: MiniCpmMlp::new(config, device),
            input_layernorm: MiniCpmRmsNorm::new(config.hidden_size, config.rms_norm_eps as f64, device),
            post_attention_layernorm: MiniCpmRmsNorm::new(config.hidden_size, config.rms_norm_eps as f64, device),
            residual_scale,
        }
    }

    #[inline]
    fn add_residual<const D: usize>(&self, residual: Tensor<D>, branch: Tensor<D>) -> Tensor<D> {
        match self.residual_scale {
            Some(s) => residual + branch.mul_scalar(s),
            None => residual + branch,
        }
    }

    pub fn forward(
        &self,
        hidden_states: Tensor<3>,
        position_emb: Option<(Tensor<2>, Tensor<2>)>,
        is_causal: bool,
    ) -> (Tensor<3>, LayerKv) {
        let residual = hidden_states.clone();
        let h = self.input_layernorm.forward(hidden_states);
        let (h, kv) = self.self_attn.forward(h, position_emb, is_causal);
        let h = self.add_residual(residual, h);

        let residual = h.clone();
        let h = self.post_attention_layernorm.forward(h);
        let h = self.mlp.forward(h);
        (self.add_residual(residual, h), kv)
    }

    pub fn forward_step(
        &self,
        hidden_states: Tensor<2>,
        position_emb: Option<(Tensor<2>, Tensor<2>)>,
        position_id: usize,
        kv_cache: &mut Option<LayerKv>,
        key_padding_mask: Option<Tensor<2, burn::tensor::Bool>>,
    ) -> Tensor<2> {
        let residual = hidden_states.clone();
        let h = self.input_layernorm.forward(hidden_states);
        let h = self.self_attn.forward_step(h, position_emb, position_id, kv_cache, key_padding_mask);
        let h = self.add_residual(residual, h);

        let residual = h.clone();
        let h = self.post_attention_layernorm.forward(h);
        let h = self.mlp.forward(h);
        self.add_residual(residual, h)
    }
}
