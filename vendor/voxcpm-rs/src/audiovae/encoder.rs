//! Causal encoder used in AudioVAE v2 (for reference-audio encoding).

use crate::audiovae::layers::{CausalConv1d, CausalResidualUnit, Snake1d};
use burn::prelude::*;

#[derive(Module, Debug)]
pub struct CausalEncoderBlock<B: Backend> {
    pub res1: CausalResidualUnit<B>,
    pub res2: CausalResidualUnit<B>,
    pub res3: CausalResidualUnit<B>,
    pub snake: Snake1d<B>,
    pub down: CausalConv1d<B>,
}

impl<B: Backend> CausalEncoderBlock<B> {
    pub fn new(input_dim: usize, output_dim: usize, stride: usize, groups: usize, device: &B::Device) -> Self {
        let kernel = 2 * stride;
        let pad = (stride as f64 / 2.0).ceil() as usize;
        let out_pad = stride % 2;
        Self {
            res1: CausalResidualUnit::new(input_dim, 1, groups, device),
            res2: CausalResidualUnit::new(input_dim, 3, groups, device),
            res3: CausalResidualUnit::new(input_dim, 9, groups, device),
            snake: Snake1d::new(input_dim, device),
            down: CausalConv1d::new(
                input_dim, output_dim, kernel, stride, 1, 1, pad, out_pad, true, device,
            ),
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let x = self.res1.forward(x);
        let x = self.res2.forward(x);
        let x = self.res3.forward(x);
        let x = self.snake.forward(x);
        self.down.forward(x)
    }
}

#[derive(Module, Debug)]
pub struct CausalEncoder<B: Backend> {
    pub first: CausalConv1d<B>,
    pub blocks: Vec<CausalEncoderBlock<B>>,
    pub fc_mu: CausalConv1d<B>,
    pub fc_logvar: CausalConv1d<B>,
}

impl<B: Backend> CausalEncoder<B> {
    pub fn new(
        d_model: usize,
        latent_dim: usize,
        strides: &[usize],
        depthwise: bool,
        device: &B::Device,
    ) -> Self {
        let first = CausalConv1d::new(1, d_model, 7, 1, 1, 1, 3, 0, true, device);
        let mut blocks = Vec::with_capacity(strides.len());
        let mut d = d_model;
        for &stride in strides {
            let next_d = d * 2;
            let groups = if depthwise { next_d / 2 } else { 1 };
            blocks.push(CausalEncoderBlock::new(d, next_d, stride, groups, device));
            d = next_d;
        }
        Self {
            first,
            blocks,
            fc_mu: CausalConv1d::new(d, latent_dim, 3, 1, 1, 1, 1, 0, true, device),
            fc_logvar: CausalConv1d::new(d, latent_dim, 3, 1, 1, 1, 1, 0, true, device),
        }
    }

    /// Returns `mu` only (inference path).
    pub fn forward_mu(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let mut h = self.first.forward(x);
        for b in &self.blocks {
            h = b.forward(h);
        }
        self.fc_mu.forward(h)
    }
}
