//! Local DiT V2: transformer backbone used as the flow-matching velocity estimator.

use crate::config::MiniCpm4Config;
use crate::minicpm4::MiniCpmModel;
use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;
use crate::minicpm4::silu_stable as silu;
use burn::tensor::TensorData;

/// Sinusoidal timestep embedding (even dim required).
#[derive(Module, Debug)]
pub struct SinusoidalPosEmb {
    pub dim: usize,
    _phantom: core::marker::PhantomData<()>,
}

impl SinusoidalPosEmb {
    pub fn new(dim: usize) -> Self {
        assert_eq!(dim % 2, 0, "SinusoidalPosEmb requires even dim");
        Self { dim, _phantom: core::marker::PhantomData }
    }

    /// `x`: `[N]` (scalar timesteps). Returns `[N, dim]`.
    pub fn forward(&self, x: Tensor<1>, scale: f64) -> Tensor<2> {
        let device = x.device();
        let half = self.dim / 2;
        let emb_scale = (10000f64).ln() / (half as f64 - 1.0);
        let freqs: Vec<f32> = (0..half).map(|i| (-emb_scale * i as f64).exp() as f32).collect();
        let freqs = Tensor::<1>::from_data(TensorData::new(freqs, [half]), &device);
        let angles = x.unsqueeze_dim::<2>(1).mul_scalar(scale) * freqs.unsqueeze::<2>();
        Tensor::cat(vec![angles.clone().sin(), angles.cos()], 1)
    }
}

#[derive(Module, Debug)]
pub struct TimestepEmbedding {
    pub linear_1: Linear,
    pub linear_2: Linear,
}

impl TimestepEmbedding {
    pub fn new(in_channels: usize, time_embed_dim: usize, out_dim: Option<usize>, device: &Device) -> Self {
        let out = out_dim.unwrap_or(time_embed_dim);
        Self {
            linear_1: LinearConfig::new(in_channels, time_embed_dim).init(device),
            linear_2: LinearConfig::new(time_embed_dim, out).init(device),
        }
    }

    pub fn forward(&self, sample: Tensor<2>) -> Tensor<2> {
        let h = self.linear_1.forward(sample);
        let h = silu(h);
        self.linear_2.forward(h)
    }
}

#[derive(Module, Debug)]
pub struct VoxCpmLocDiTV2 {
    pub in_proj: Linear,
    pub cond_proj: Linear,
    pub out_proj: Linear,
    pub time_embeddings: SinusoidalPosEmb,
    pub time_mlp: TimestepEmbedding,
    pub delta_time_mlp: TimestepEmbedding,
    pub decoder: MiniCpmModel,
    pub in_channels: usize,
}

/// Precomputed, timestep-invariant DiT inputs. In a diffusion solve (euler
/// integration over N timesteps), `mu`, `cond`, and — when `dt` is zero
/// (non-mean-mode) — `dt_emb` don't change across timesteps, so we project
/// them once per solve instead of once per timestep.
#[derive(Debug)]
pub struct LocDitCache {
    mu: Tensor<3>,     // [N, mu_tokens, hidden]
    cond: Tensor<3>,   // [N, prefix, hidden]  (already projected)
    dt_emb: Tensor<2>, // [N, hidden]
    prefix_len: usize,
    mu_tokens: usize,
}

impl VoxCpmLocDiTV2 {
    pub fn new(config: MiniCpm4Config, in_channels: usize, device: &Device) -> Self {
        assert_eq!(config.vocab_size, 0, "vocab_size must be 0 for local DiT");
        let hidden = config.hidden_size;
        Self {
            in_proj: LinearConfig::new(in_channels, hidden).init(device),
            cond_proj: LinearConfig::new(in_channels, hidden).init(device),
            out_proj: LinearConfig::new(hidden, in_channels).init(device),
            time_embeddings: SinusoidalPosEmb::new(hidden),
            time_mlp: TimestepEmbedding::new(hidden, hidden, None, device),
            delta_time_mlp: TimestepEmbedding::new(hidden, hidden, None, device),
            decoder: MiniCpmModel::new(config, device),
            in_channels,
        }
    }

    /// * `x`: `[N, C, T]` noisy sample.
    /// * `mu`: `[N, C_mu]` (will be reshaped to `[N, C_mu/H, H]`).
    /// * `t`, `dt`: `[N]`.
    /// * `cond`: `[N, C, T']`.
    pub fn forward(
        &self,
        x: Tensor<3>,
        mu: Tensor<2>,
        t: Tensor<1>,
        cond: Tensor<3>,
        dt: Tensor<1>,
    ) -> Tensor<3> {
        let cache = self.prepare(mu, cond, dt);
        self.forward_cached(x, t, &cache)
    }

    /// Precompute timestep-invariant projections. Call once per diffusion
    /// solve, then reuse via [`forward_cached`] for each timestep.
    pub fn prepare(
        &self,
        mu: Tensor<2>,
        cond: Tensor<3>,
        dt: Tensor<1>,
    ) -> LocDitCache {
        let cond = self.cond_proj.forward(cond.swap_dims(1, 2));
        let hidden = cond.dims()[2];
        let prefix_len = cond.dims()[1];

        let dt_emb = self.time_embeddings.forward(dt, 1000.0);
        let dt_emb = self.delta_time_mlp.forward(dt_emb);

        let n = mu.dims()[0];
        let mu_tokens = mu.dims()[1] / hidden;
        let mu = mu.reshape([n, mu_tokens, hidden]);

        LocDitCache { mu, cond, dt_emb, prefix_len, mu_tokens }
    }

    /// Run one diffusion step using a pre-prepared cache. `x` and `t` are the
    /// only per-timestep inputs.
    pub fn forward_cached(
        &self,
        x: Tensor<3>,
        t: Tensor<1>,
        cache: &LocDitCache,
    ) -> Tensor<3> {
        let [_n, _c, time_len] = x.dims();
        let x = self.in_proj.forward(x.swap_dims(1, 2));

        let t_emb = self.time_embeddings.forward(t, 1000.0);
        let t_emb = self.time_mlp.forward(t_emb);
        let t_emb = t_emb + cache.dt_emb.clone();
        let t_tok: Tensor<3> = t_emb.unsqueeze_dim(1);

        let seq = Tensor::cat(vec![cache.mu.clone(), t_tok, cache.cond.clone(), x], 1);
        let (hidden_out, _) = self.decoder.forward(seq, false);

        let skip = cache.prefix_len + cache.mu_tokens + 1;
        let hidden_out = hidden_out.narrow(1, skip, time_len);
        let out = self.out_proj.forward(hidden_out);
        out.swap_dims(1, 2)
    }
}
