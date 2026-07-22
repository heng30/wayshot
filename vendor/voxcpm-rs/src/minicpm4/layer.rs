//! A single MiniCPM-4 decoder layer.

use crate::config::MiniCpm4Config;
use crate::minicpm4::attention::{LayerKv, MiniCpmAttention};
use crate::minicpm4::mlp::MiniCpmMlp;
use crate::minicpm4::MiniCpmRmsNorm;
use burn::prelude::*;

#[derive(Module, Debug)]
pub struct MiniCpmDecoderLayer<B: Backend> {
    pub self_attn: MiniCpmAttention<B>,
    pub mlp: MiniCpmMlp<B>,
    pub input_layernorm: MiniCpmRmsNorm<B>,
    pub post_attention_layernorm: MiniCpmRmsNorm<B>,
    pub residual_scale: Option<f64>,
}

impl<B: Backend> MiniCpmDecoderLayer<B> {
    pub fn new(config: &MiniCpm4Config, device: &B::Device) -> Self {
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
    fn add_residual<const D: usize>(&self, residual: Tensor<B, D>, branch: Tensor<B, D>) -> Tensor<B, D> {
        match self.residual_scale {
            Some(s) => residual + branch.mul_scalar(s),
            None => residual + branch,
        }
    }

    pub fn forward(
        &self,
        hidden_states: Tensor<B, 3>,
        position_emb: Option<(Tensor<B, 2>, Tensor<B, 2>)>,
        is_causal: bool,
    ) -> (Tensor<B, 3>, LayerKv<B>) {
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
        hidden_states: Tensor<B, 2>,
        position_emb: Option<(Tensor<B, 2>, Tensor<B, 2>)>,
        position_id: usize,
        kv_cache: &mut Option<LayerKv<B>>,
        key_padding_mask: Option<Tensor<B, 2, burn::tensor::Bool>>,
    ) -> Tensor<B, 2> {
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
