//! Local encoder: a cls-pooled MiniCPM-4 over `[B, T, P, D]`.

use crate::config::MiniCpm4Config;
use crate::minicpm4::MiniCpmModel;
use burn::module::Param;
use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;

#[derive(Module, Debug)]
pub struct VoxCpmLocEnc<B: Backend> {
    pub special_token: Param<Tensor<B, 4>>, // [1, 1, 1, hidden]
    pub in_proj: Linear<B>,
    pub encoder: MiniCpmModel<B>,
}

impl<B: Backend> VoxCpmLocEnc<B> {
    pub fn new(config: MiniCpm4Config, input_dim: usize, device: &B::Device) -> Self {
        assert_eq!(config.vocab_size, 0, "vocab_size must be 0 for local encoder");
        let hidden = config.hidden_size;
        Self {
            special_token: Param::from_tensor(Tensor::random(
                [1, 1, 1, hidden],
                burn::tensor::Distribution::Normal(0.0, 1.0),
                device,
            )),
            in_proj: LinearConfig::new(input_dim, hidden).init(device),
            encoder: MiniCpmModel::new(config, device),
        }
    }

    /// `x`: `[B, T, P, D]`. Returns `[B, T, hidden]` (cls-pooled per frame).
    pub fn forward(&self, x: Tensor<B, 4>) -> Tensor<B, 3> {
        let [b, t, p, _d] = x.dims();
        let hidden = self.encoder.config.hidden_size;

        let x = self.in_proj.forward(x); // [B, T, P, H]
        let special = self.special_token.val().expand([b as i32, t as i32, 1, hidden as i32]);
        let x = Tensor::cat(vec![special, x], 2); // [B, T, P+1, H]

        let x: Tensor<B, 3> = x.reshape([b * t, p + 1, hidden]);
        let (outputs, _) = self.encoder.forward(x, false); // [B*T, P+1, H]
        let cls: Tensor<B, 2> = outputs.narrow(1, 0, 1).squeeze_dim::<2>(1);
        cls.reshape([b, t, hidden])
    }
}
