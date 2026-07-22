//! MiniCPM-4 transformer backbone used by the text-semantic LM, the residual
//! acoustic LM, the local encoder, and the local DiT estimator.

pub mod attention;
pub mod cache;
pub mod layer;
pub mod mlp;
pub mod model;
pub mod rope;

pub use attention::MiniCpmAttention;
pub use cache::StaticKvCache;
pub use layer::MiniCpmDecoderLayer;
pub use mlp::MiniCpmMlp;
pub use model::MiniCpmModel;

use burn::tensor::activation::sigmoid;

/// Numerically-stable SiLU computed as `x * sigmoid(x)` in f32 then cast back
/// to the input dtype.
///
/// `burn::tensor::activation::silu` produces wildly inflated outputs on the
/// cubecl Vulkan/SPIR-V bf16 path (e.g. `silu(23.75) ≈ 3616`). This is not a
/// silu bug per se: bf16 *elementwise multiply* is broken in that backend
/// (`10*2 = 2560`, see `examples/bf16_probe.rs`) and silu = `x * sigmoid(x)`
/// inherits the broken multiply. Doing both the mul and sigmoid in f32 here
/// sidesteps it. On pure-f32 backends every `cast` is a no-op so this stays
/// free.
pub fn silu_stable<B: Backend, const D: usize>(x: Tensor<B, D>) -> Tensor<B, D> {
    let orig: burn::tensor::FloatDType = x.dtype().into();
    let xf = x.cast(burn::tensor::FloatDType::F32);
    (xf.clone() * sigmoid(xf)).cast(orig)
}
pub use rope::MiniCpmLongRope;

use burn::module::Param;
use burn::prelude::*;

/// MiniCPM's RMSNorm.
#[derive(Module, Debug)]
pub struct MiniCpmRmsNorm<B: Backend> {
    pub weight: Param<Tensor<B, 1>>,
    pub eps: f64,
}

impl<B: Backend> MiniCpmRmsNorm<B> {
    pub fn new(hidden_size: usize, eps: f64, device: &B::Device) -> Self {
        Self {
            weight: Param::from_tensor(Tensor::ones([hidden_size], device)),
            eps,
        }
    }

    pub fn forward<const D: usize>(&self, x: Tensor<B, D>) -> Tensor<B, D> {
        // Match Python reference (`rms_layernorm`): variance and
        // normalization in f32 for numerical stability when the active dtype
        // is bf16/f16. We also promote the weight to f32 before the final
        // scale, then cast the whole result back. On pure-f32 backends every
        // `cast` is a no-op so this stays free.
        let orig_dtype: burn::tensor::FloatDType = x.dtype().into();
        let x_f32 = x.cast(burn::tensor::FloatDType::F32);
        let x_sq = x_f32.clone() * x_f32.clone();
        let variance = x_sq.mean_dim(D - 1);
        let inv_rms = variance.add_scalar(self.eps).sqrt().recip();
        let normed_f32 = x_f32 * inv_rms;
        let weight_f32 = self.weight.val().cast(burn::tensor::FloatDType::F32);
        (normed_f32 * weight_f32.unsqueeze()).cast(orig_dtype)
    }
}
