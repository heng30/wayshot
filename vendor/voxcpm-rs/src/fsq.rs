//! Scalar quantization layer used between the base LM and residual LM.

use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;
use burn::tensor::activation::tanh;

#[derive(Module, Debug)]
pub struct ScalarQuantizationLayer<B: Backend> {
    pub in_proj: Linear<B>,
    pub out_proj: Linear<B>,
    pub scale: f64,
}

impl<B: Backend> ScalarQuantizationLayer<B> {
    pub fn new(in_dim: usize, out_dim: usize, latent_dim: usize, scale: usize, device: &B::Device) -> Self {
        Self {
            in_proj: LinearConfig::new(in_dim, latent_dim).init(device),
            out_proj: LinearConfig::new(latent_dim, out_dim).init(device),
            scale: scale as f64,
        }
    }

    /// Inference-only: tanh → round-quantize → out_proj.
    pub fn forward<const D: usize>(&self, hidden: Tensor<B, D>) -> Tensor<B, D> {
        let h = self.in_proj.forward(hidden);
        let h = tanh(h);
        let h = h.mul_scalar(self.scale).round().div_scalar(self.scale);
        self.out_proj.forward(h)
    }
}
