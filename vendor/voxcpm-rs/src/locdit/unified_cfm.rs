//! Unified Conditional Flow-Matching sampler (inference-only).

use crate::locdit::local_dit::VoxCpmLocDiTV2;
use burn::prelude::*;
use burn::tensor::TensorData;

#[derive(Module, Debug)]
#[module(skip = sigma_min, inference_cfg_rate)]
pub struct UnifiedCfm<B: Backend> {
    pub estimator: VoxCpmLocDiTV2<B>,
    pub in_channels: usize,
    pub sigma_min: f64,
    pub mean_mode: bool,
    pub inference_cfg_rate: f64,
}

impl<B: Backend> UnifiedCfm<B> {
    pub fn new(
        in_channels: usize,
        estimator: VoxCpmLocDiTV2<B>,
        sigma_min: f64,
        inference_cfg_rate: f64,
        mean_mode: bool,
    ) -> Self {
        Self {
            estimator,
            in_channels,
            sigma_min,
            mean_mode,
            inference_cfg_rate,
        }
    }

    pub fn forward(
        &self,
        mu: Tensor<B, 2>,
        n_timesteps: usize,
        patch_size: usize,
        cond: Tensor<B, 3>,
        temperature: f64,
        cfg_value: f64,
        sway_sampling_coef: f64,
        use_cfg_zero_star: bool,
    ) -> Tensor<B, 3> {
        let [b, _] = mu.dims();
        let device = mu.device();

        let z = if std::env::var("VOXCPM_Z_ZERO").is_ok() {
            Tensor::<B, 3>::zeros([b, self.in_channels, patch_size], &device)
        } else {
            Tensor::<B, 3>::random(
                [b, self.in_channels, patch_size],
                burn::tensor::Distribution::Normal(0.0, 1.0),
                &device,
            )
            .mul_scalar(temperature)
        };

        let n = n_timesteps + 1;
        let step = 1.0 / n_timesteps as f64;
        let mut t_vec: Vec<f32> = (0..n).map(|i| (1.0 - i as f64 * step) as f32).collect();
        for t in t_vec.iter_mut() {
            let tt = *t as f64;
            let transformed =
                tt + sway_sampling_coef * ((std::f64::consts::FRAC_PI_2 * tt).cos() - 1.0 + tt);
            *t = transformed as f32;
        }

        self.solve_euler(z, &t_vec, mu, cond, cfg_value, use_cfg_zero_star)
    }

    fn solve_euler(
        &self,
        mut x: Tensor<B, 3>,
        t_span: &[f32],
        mu: Tensor<B, 2>,
        cond: Tensor<B, 3>,
        cfg_value: f64,
        use_cfg_zero_star: bool,
    ) -> Tensor<B, 3> {
        let device = x.device();
        let mut t = t_span[0] as f64;
        let mut dt = (t_span[0] - t_span[1]) as f64;

        let zero_init_steps = ((t_span.len() as f64) * 0.04).max(1.0) as usize;

        let [b, c, time_len] = x.dims();
        let mu_dim = mu.dims()[1];

        // Loop-invariant CFG inputs: `mu` (zeroed for uncond branch) and
        // `cond` (duplicated as-is) don't change across diffusion steps, so
        // build the batched tensors once instead of re-concatenating every
        // step. NOTE: only `mu` is zeroed for the unconditional branch; the
        // prefix-feat `cond` is duplicated as-is. (Matches Python's
        // `cond_in[:b], cond_in[b:] = cond, cond`.)
        let mu_zeros = Tensor::<B, 2>::zeros([b, mu_dim], &device);
        let mu_in = Tensor::cat(vec![mu.clone(), mu_zeros], 0);
        let cond_in = Tensor::cat(vec![cond.clone(), cond.clone()], 0);

        // In non-mean-mode, `dt` is zero every step, so `dt_emb` is constant
        // and we can prepare all timestep-invariant DiT inputs (projected
        // `mu`, `cond`, `dt_emb`) once per solve. The inner loop then only
        // re-projects `x` and `t`. Saves ~9× redundant linear/embed ops
        // per diffusion solve.
        let dit_cache = if !self.mean_mode {
            let dt_zeros = Tensor::<B, 1>::zeros([2 * b], &device);
            Some(self.estimator.prepare(mu_in.clone(), cond_in.clone(), dt_zeros))
        } else {
            None
        };

        for step in 1..t_span.len() {
            let dphi_dt: Tensor<B, 3> = if use_cfg_zero_star && step <= zero_init_steps {
                Tensor::zeros([b, c, time_len], &device)
            } else {
                let x_in = Tensor::cat(vec![x.clone(), x.clone()], 0);
                let t_tensor = Tensor::<B, 1>::from_data(
                    TensorData::new(vec![t as f32; 2 * b], [2 * b]),
                    &device,
                );

                let out = if let Some(cache) = &dit_cache {
                    self.estimator.forward_cached(x_in, t_tensor, cache)
                } else {
                    // mean_mode: dt varies per step, fall back to full forward.
                    let dt_tensor = Tensor::<B, 1>::from_data(
                        TensorData::new(vec![dt as f32; 2 * b], [2 * b]),
                        &device,
                    );
                    self.estimator.forward(x_in, mu_in.clone(), t_tensor, cond_in.clone(), dt_tensor)
                };
                let pos = out.clone().narrow(0, 0, b);
                let neg = out.narrow(0, b, b);

                if use_cfg_zero_star {
                    let pos_flat: Tensor<B, 2> = pos.clone().reshape([b, c * time_len]);
                    let neg_flat: Tensor<B, 2> = neg.clone().reshape([b, c * time_len]);
                    let dot = (pos_flat.clone() * neg_flat.clone()).sum_dim(1); // [B, 1]
                    let sq = (neg_flat.clone() * neg_flat.clone()).sum_dim(1).add_scalar(1e-8);
                    let st: Tensor<B, 2> = dot / sq;
                    let st: Tensor<B, 3> = st.unsqueeze_dim(2);
                    let st_b: Tensor<B, 3> = st.expand([b as i32, c as i32, time_len as i32]);
                    // Algebra: neg*st + (pos - neg*st)*cfg
                    //        = neg*st*(1-cfg) + pos*cfg
                    // Saves one elementwise kernel per step (1 mul, 1 sub avoided
                    // in favour of a fused mul_scalar). At ~10 timesteps * ~25 AR
                    // steps that's ~250 fewer kernel launches per generation.
                    let scaled_st = st_b.mul_scalar(1.0 - cfg_value);
                    neg * scaled_st + pos.mul_scalar(cfg_value)
                } else {
                    neg.clone() + (pos - neg).mul_scalar(cfg_value)
                }
            };

            x = (x - dphi_dt.mul_scalar(dt)).clamp(-50.0, 50.0);
            t -= dt;
            if step < t_span.len() - 1 {
                dt = t - t_span[step + 1] as f64;
            }
        }

        x
    }
}
