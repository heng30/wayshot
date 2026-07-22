//! Gated MLP used inside each MiniCPM-4 decoder layer.

use crate::config::MiniCpm4Config;
use crate::minicpm4::silu_stable;
use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;

#[derive(Module, Debug)]
pub struct MiniCpmMlp<B: Backend> {
    /// Fused `[gate_proj | up_proj]` along the output dim. Output is
    /// `2 * intermediate_size`; we slice it into the gate and up halves.
    pub gate_up_proj: Linear<B>,
    pub down_proj: Linear<B>,
    inter: usize,
}

impl<B: Backend> MiniCpmMlp<B> {
    pub fn new(config: &MiniCpm4Config, device: &B::Device) -> Self {
        let hidden = config.hidden_size;
        let inter = config.intermediate_size;
        Self {
            gate_up_proj: LinearConfig::new(hidden, 2 * inter).with_bias(false).init(device),
            down_proj: LinearConfig::new(inter, hidden).with_bias(false).init(device),
            inter,
        }
    }

    pub fn forward<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        let gu = self.gate_up_proj.forward(x);
        let last = D - 1;
        let gate = gu.clone().narrow(last, 0, self.inter);
        let up = gu.narrow(last, self.inter, self.inter);
        self.down_proj.forward(silu_stable(gate) * up)
    }
}
