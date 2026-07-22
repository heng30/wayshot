//! Neural-network architecture of VoxCPM2: the top-level [`VoxCpm2Model`] that
//! composes the base and residual MiniCPM-4 LMs, the local feature encoder, the
//! diffusion decoder, the scalar-quantization layer, the projection layers, the
//! stop-prediction head, and the AudioVAE.

use crate::audiovae::AudioVae;
use crate::config::VoxCpm2Config;
use crate::fsq::ScalarQuantizationLayer;
use crate::locdit::{UnifiedCfm, VoxCpmLocDiTV2};
use crate::locenc::VoxCpmLocEnc;
use crate::minicpm4::MiniCpmModel;
use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;

/// Special audio tokens added to the text vocabulary.
pub const AUDIO_START_TOKEN: i64 = 101;
pub const AUDIO_END_TOKEN: i64 = 102;
pub const REF_AUDIO_START_TOKEN: i64 = 103;
pub const REF_AUDIO_END_TOKEN: i64 = 104;

#[derive(Module, Debug)]
#[module(skip = config)]
pub struct VoxCpm2Model<B: Backend> {
    pub base_lm: MiniCpmModel<B>,
    pub residual_lm: MiniCpmModel<B>,
    pub feat_encoder: VoxCpmLocEnc<B>,
    pub feat_decoder: UnifiedCfm<B>,
    pub fsq_layer: ScalarQuantizationLayer<B>,

    pub enc_to_lm_proj: Linear<B>,
    pub lm_to_dit_proj: Linear<B>,
    pub res_to_dit_proj: Linear<B>,
    pub fusion_concat_proj: Linear<B>,

    pub stop_proj: Linear<B>,
    pub stop_head: Linear<B>,

    pub audio_vae: AudioVae<B>,
    pub config: VoxCpm2Config,
}

impl<B: Backend> VoxCpm2Model<B> {
    pub fn new(config: VoxCpm2Config, device: &B::Device) -> Self {
        let lm_hidden = config.lm_config.hidden_size;
        let enc_hidden = config.encoder_config.hidden_dim;
        let dit_hidden = config.dit_config.hidden_dim;

        let audio_vae_config = config
            .audio_vae_config
            .clone()
            .unwrap_or_default();

        let base_lm = MiniCpmModel::new(config.lm_config.clone(), device);
        let residual_lm = MiniCpmModel::new(config.residual_lm_config(), device);

        let feat_encoder = VoxCpmLocEnc::new(config.encoder_lm_config(), config.feat_dim, device);

        let estimator = VoxCpmLocDiTV2::new(config.dit_lm_config(), config.feat_dim, device);
        let feat_decoder = UnifiedCfm::new(
            config.feat_dim,
            estimator,
            config.dit_config.cfm_config.sigma_min as f64,
            config.dit_config.cfm_config.inference_cfg_rate as f64,
            config.dit_config.dit_mean_mode,
        );

        let fsq_layer = ScalarQuantizationLayer::new(
            lm_hidden,
            lm_hidden,
            config.scalar_quantization_latent_dim,
            config.scalar_quantization_scale,
            device,
        );

        let enc_to_lm_proj = LinearConfig::new(enc_hidden, lm_hidden).init(device);
        let lm_to_dit_proj = LinearConfig::new(lm_hidden, dit_hidden).init(device);
        let res_to_dit_proj = LinearConfig::new(lm_hidden, dit_hidden).init(device);
        let fusion_concat_proj = LinearConfig::new(lm_hidden * 2, lm_hidden).init(device);

        let stop_proj = LinearConfig::new(lm_hidden, lm_hidden).init(device);
        let stop_head = LinearConfig::new(lm_hidden, 2).with_bias(false).init(device);

        let audio_vae = AudioVae::new(audio_vae_config, device);

        Self {
            base_lm,
            residual_lm,
            feat_encoder,
            feat_decoder,
            fsq_layer,
            enc_to_lm_proj,
            lm_to_dit_proj,
            res_to_dit_proj,
            fusion_concat_proj,
            stop_proj,
            stop_head,
            audio_vae,
            config,
        }
    }

    pub fn sample_rate(&self) -> usize {
        self.audio_vae.out_sample_rate()
    }

    pub fn patch_size(&self) -> usize {
        self.config.patch_size
    }

    pub fn latent_dim(&self) -> usize {
        self.config.audio_vae_config.as_ref().map(|c| c.latent_dim).unwrap_or(64)
    }

    fn scale_emb(&self) -> f64 {
        if self.config.lm_config.use_mup {
            self.config.lm_config.scale_emb as f64
        } else {
            1.0
        }
    }

    /// Run text + prompt-feat prefill through the base + residual LMs and
    /// build the per-call autoregressive state used by [`Self::dit_step`] /
    /// [`Self::lm_step`].
    ///
    /// `max_len` sizes the static KV caches so they cover the prefill plus
    /// up to `max_len` AR steps.
    pub fn prefill(
        &self,
        text_token: Tensor<B, 2, burn::tensor::Int>,
        text_mask: Tensor<B, 2>,
        feat: Tensor<B, 4>,
        feat_mask: Tensor<B, 2>,
        max_len: usize,
    ) -> InferenceState<B> {
        self.prefill_with_lengths(text_token, text_mask, feat, feat_mask, max_len, None)
    }

    /// Same as [`Self::prefill`] but accepts an optional `prefill_lengths`
    /// vector — the per-batch real (unpadded) prefill length. When `Some`,
    /// the inputs are assumed to be right-padded to a common max-S, and:
    ///   - the last hidden state per batch element is extracted at index
    ///     `lengths[b] - 1` (instead of `S - 1`),
    ///   - a key-padding mask is built and stored in the resulting state
    ///     so subsequent [`Self::lm_step`] calls can mask out the
    ///     `[lengths[b]..S]` gap in the KV caches.
    ///
    /// When `None` the path is identical to the unbatched/serial code.
    pub fn prefill_with_lengths(
        &self,
        text_token: Tensor<B, 2, burn::tensor::Int>,
        text_mask: Tensor<B, 2>,
        feat: Tensor<B, 4>,
        feat_mask: Tensor<B, 2>,
        max_len: usize,
        prefill_lengths: Option<Vec<usize>>,
    ) -> InferenceState<B> {
        let device = feat.device();
        let [_b, _s, _p, _d] = feat.dims();

        // 1) Encode audio feature patches.
        let feat_embed = self.feat_encoder.forward(feat.clone()); // [B, S, enc_h]
        let feat_embed = self.enc_to_lm_proj.forward(feat_embed); // [B, S, lm_h]

        // 2) Embed text tokens.
        let scale = self.scale_emb();
        let text_embed = self.base_lm.embed(text_token).mul_scalar(scale); // [B, S, lm_h]

        // 3) Combine via masks.
        let text_mask3: Tensor<B, 3> = text_mask.clone().unsqueeze_dim(2);
        let feat_mask3: Tensor<B, 3> = feat_mask.clone().unsqueeze_dim(2);
        let combined = text_embed * text_mask3.clone() + feat_embed.clone() * feat_mask3.clone();

        // 4) Prefix feat cond (last patch).
        let s = feat.dims()[1];
        let prefix_feat: Tensor<B, 3> = feat.clone().narrow(1, s - 1, 1).squeeze_dim::<3>(1); // [B, P, D]

        // 5) Base LM prefill.
        let (enc_outputs, base_kv) = self.base_lm.forward(combined, true);
        let enc_outputs = self.fsq_layer.forward(enc_outputs.clone()) * feat_mask3.clone()
            + enc_outputs * text_mask3;
        let lm_hidden_prefill = enc_outputs.clone();

        // 6) Residual LM prefill.
        let residual_input = self.fusion_concat_proj.forward(Tensor::cat(
            vec![enc_outputs, feat_embed.clone() * feat_mask3],
            2,
        ));
        let (residual_outputs, residual_kv) = self.residual_lm.forward(residual_input, true);

        // Seed caches with the prefill K/V.
        let s_ctx = lm_hidden_prefill.dims()[1];
        let batch = lm_hidden_prefill.dims()[0];
        let lm_config = self.config.lm_config.clone();
        let max_ctx = self.config.max_length.max(s_ctx + max_len);
        let mut base_cache = crate::minicpm4::StaticKvCache::new(
            lm_config.num_hidden_layers,
            lm_config.num_key_value_heads,
            lm_config.head_dim(),
            batch,
            max_ctx,
            &device,
        );
        base_cache.fill(base_kv);
        let res_cfg = self.config.residual_lm_config();
        let mut res_cache = crate::minicpm4::StaticKvCache::new(
            res_cfg.num_hidden_layers,
            res_cfg.num_key_value_heads,
            res_cfg.head_dim(),
            batch,
            max_ctx,
            &device,
        );
        res_cache.fill(residual_kv);

        // Take the last position for autoregressive start. Shape [B, lm_h].
        // For batched padded inputs (`prefill_lengths` set), each batch
        // element's real last position is `lengths[b] - 1`; we extract
        // per-element with a small narrow loop and re-cat. For B=1 / serial,
        // this collapses to the original `narrow(s_ctx-1, 1)` path.
        let lm_h = lm_hidden_prefill.dims()[2];
        let (lm_hidden, residual_hidden) = match &prefill_lengths {
            None => (
                lm_hidden_prefill.clone().narrow(1, s_ctx - 1, 1).squeeze_dim::<2>(1),
                residual_outputs.clone().narrow(1, s_ctx - 1, 1).squeeze_dim::<2>(1),
            ),
            Some(lens) => {
                assert_eq!(lens.len(), batch, "prefill_lengths.len() must equal batch size");
                let mut lm_rows = Vec::with_capacity(batch);
                let mut res_rows = Vec::with_capacity(batch);
                for (b, &len) in lens.iter().enumerate() {
                    assert!(len >= 1 && len <= s_ctx, "prefill_lengths[{b}]={len} out of range (S_ctx={s_ctx})");
                    lm_rows.push(
                        lm_hidden_prefill.clone()
                            .slice([b..b + 1, len - 1..len, 0..lm_h])
                            .squeeze_dim::<2>(1),
                    );
                    res_rows.push(
                        residual_outputs.clone()
                            .slice([b..b + 1, len - 1..len, 0..lm_h])
                            .squeeze_dim::<2>(1),
                    );
                }
                (Tensor::cat(lm_rows, 0), Tensor::cat(res_rows, 0))
            }
        };

        // Build the cache key-padding mask once: mask out `[lengths[b]..s_ctx]`
        // for each batch element. New decode positions (>= s_ctx) are always
        // unmasked. We size the mask to `max_ctx` (full cache length) so it
        // can be sliced to current_len at each step.
        let key_padding_mask: Option<Tensor<B, 2, burn::tensor::Bool>> = match &prefill_lengths {
            None => None,
            Some(lens) => {
                if lens.iter().all(|&l| l == s_ctx) {
                    None // all rows full — no padding to mask
                } else {
                    let mut data = vec![false; batch * max_ctx];
                    for (b, &len) in lens.iter().enumerate() {
                        for j in len..s_ctx {
                            data[b * max_ctx + j] = true;
                        }
                        // [s_ctx..max_ctx] are future decode slots: keep false (unmasked).
                    }
                    Some(Tensor::<B, 1, burn::tensor::Bool>::from_data(
                        burn::tensor::TensorData::new(data, [batch * max_ctx]),
                        &device,
                    )
                    .reshape([batch, max_ctx]))
                }
            }
        };

        InferenceState {
            lm_hidden,
            residual_hidden,
            prefix_feat_cond: prefix_feat,
            base_cache,
            res_cache,
            steps_taken: 0,
            key_padding_mask,
        }
    }

    /// Run one diffusion sample + stop-head check from the current state.
    ///
    /// Updates `state.prefix_feat_cond` to the newly predicted patch (so the
    /// next DiT step sees it as context) but does NOT advance the LM caches —
    /// call [`Self::lm_step`] with the returned `pred_feat` to do that before
    /// the next [`Self::dit_step`].
    pub fn dit_step(
        &self,
        state: &mut InferenceState<B>,
        inference_timesteps: usize,
        cfg_value: f64,
    ) -> DitStep<B> {
        let patch_size = self.patch_size();

        // DiT inputs: concat(lm_to_dit(lm), res_to_dit(res))
        let dit1 = self.lm_to_dit_proj.forward(state.lm_hidden.clone());
        let dit2 = self.res_to_dit_proj.forward(state.residual_hidden.clone());
        let dit_hidden = Tensor::cat(vec![dit1, dit2], 1); // [B, 2*dit_h]

        // Diffusion sample: [B, D, P] -> [B, P, D]
        let pred = self.feat_decoder.forward(
            dit_hidden,
            inference_timesteps,
            patch_size,
            state.prefix_feat_cond.clone().swap_dims(1, 2),
            1.0,
            cfg_value,
            1.0,
            true,
        );
        let pred_feat = pred.swap_dims(1, 2);
        let pred4: Tensor<B, 4> = pred_feat.clone().unsqueeze_dim(1);
        state.prefix_feat_cond = pred_feat;

        // Stop check (cheap GPU→CPU sync via argmax). One bit per batch
        // element — for B=1 this is a 1-element vec.
        let stop_logits = self
            .stop_head
            .forward(crate::minicpm4::silu_stable(self.stop_proj.forward(state.lm_hidden.clone())));
        let stops: Vec<bool> = stop_logits
            .argmax(1)
            .into_data()
            .iter::<i64>()
            .map(|v| v == 1)
            .collect();

        DitStep { pred_feat: pred4, stops }
    }

    /// Advance the base + residual LMs by one position using `pred_feat`
    /// (`[1, 1, P, D]`, the patch returned by [`Self::dit_step`]). Caller
    /// should only call this if it intends to take another DiT step.
    pub fn lm_step(&self, state: &mut InferenceState<B>, pred_feat: Tensor<B, 4>) {
        let curr_embed = self.feat_encoder.forward(pred_feat); // [B, 1, enc_h]
        let curr_embed = self.enc_to_lm_proj.forward(curr_embed); // [B, 1, lm_h]
        let curr_embed2: Tensor<B, 2> = curr_embed.squeeze_dim::<2>(1); // [B, lm_h]

        let pos = state.base_cache.step();
        let mut lm_hidden = self.base_lm.forward_step_masked(
            curr_embed2.clone(),
            pos,
            &mut state.base_cache,
            state.key_padding_mask.clone(),
        );
        lm_hidden = self.fsq_layer.forward(lm_hidden);

        let res_input2 = self
            .fusion_concat_proj
            .forward(Tensor::cat(vec![lm_hidden.clone(), curr_embed2], 1));
        let pos = state.res_cache.step();
        let residual_hidden = self.residual_lm.forward_step_masked(
            res_input2,
            pos,
            &mut state.res_cache,
            state.key_padding_mask.clone(),
        );

        state.lm_hidden = lm_hidden;
        state.residual_hidden = residual_hidden;
        state.steps_taken += 1;
    }

    /// Stack a sequence of predicted latent patches `[1, 1, P, D]` into the
    /// AudioVAE input shape `[1, D, T*P]`.
    pub fn stack_pred_feats(pred_feats: &[Tensor<B, 4>]) -> Tensor<B, 3> {
        let feats = Tensor::cat(pred_feats.to_vec(), 1);
        let [b, t, p, d] = feats.dims();
        feats.swap_dims(1, 3).swap_dims(2, 3).reshape([b, d, t * p])
    }

    /// Core inference loop.
    ///
    /// Runs the text+audio-mask prefill through the base LM and residual LM,
    /// then iteratively samples audio feature patches via the diffusion
    /// decoder until the stop head fires (or `max_len` is reached).
    ///
    /// * `text_token`: `[B, S]` int tokens.
    /// * `text_mask`, `feat_mask`: `[B, S]` float masks (0/1) indicating which
    ///   positions are text and which are audio patches.
    /// * `feat`: `[B, S, P, D]` audio latent patches (zeros at text positions).
    ///
    /// Returns `(latent, stop_steps)` where `latent` is `[B, D, T_max*P]`
    /// and `stop_steps[b]` is the number of valid latent patches for
    /// batch element `b` (= the index of the patch where its stop fired,
    /// inclusive). For `B=1` the loop short-circuits when stop fires so
    /// `T_max == stop_steps[0]`. For `B>1` the loop continues until ALL
    /// elements have stopped (or `max_len`); callers must slice each
    /// element to its own valid prefix before decoding through the AudioVAE.
    pub fn inference(
        &self,
        text_token: Tensor<B, 2, burn::tensor::Int>,
        text_mask: Tensor<B, 2>,
        feat: Tensor<B, 4>,
        feat_mask: Tensor<B, 2>,
        min_len: usize,
        max_len: usize,
        inference_timesteps: usize,
        cfg_value: f64,
        cancel: Option<&dyn Fn() -> bool>,
    ) -> crate::Result<(Tensor<B, 3>, Vec<usize>)> {
        self.inference_with_lengths(
            text_token, text_mask, feat, feat_mask,
            min_len, max_len, inference_timesteps, cfg_value, cancel, None,
        )
    }

    /// Like [`Self::inference`] but accepts an optional `prefill_lengths`
    /// vector for batched/right-padded inputs. See
    /// [`Self::prefill_with_lengths`].
    pub fn inference_with_lengths(
        &self,
        text_token: Tensor<B, 2, burn::tensor::Int>,
        text_mask: Tensor<B, 2>,
        feat: Tensor<B, 4>,
        feat_mask: Tensor<B, 2>,
        min_len: usize,
        max_len: usize,
        inference_timesteps: usize,
        cfg_value: f64,
        cancel: Option<&dyn Fn() -> bool>,
        prefill_lengths: Option<Vec<usize>>,
    ) -> crate::Result<(Tensor<B, 3>, Vec<usize>)> {
        let batch = text_token.dims()[0];
        let mut state = self.prefill_with_lengths(
            text_token, text_mask, feat, feat_mask, max_len, prefill_lengths,
        );
        let mut pred_feats: Vec<Tensor<B, 4>> = Vec::new();
        let mut stopped = vec![false; batch];
        let mut stop_steps = vec![max_len; batch];

        let profile = std::env::var("VOXCPM_PROFILE").is_ok();
        let mut t_dit_ns: u128 = 0;
        let mut t_lm_ns: u128 = 0;
        let mut n_steps: usize = 0;

        // Helper closure: force a GPU→CPU sync by reading a tiny scalar.
        let sync_barrier = |t: Tensor<B, 2>| {
            let _ = t.slice([0..1, 0..1]).into_data();
        };

        for i in 0..max_len {
            // Cancellation check (cheap atomic load via callback). Bails
            // before launching the next DiT sample so latency = at most
            // one in-flight diffusion step (~200ms on wgpu).
            if let Some(c) = cancel
                && c()
            {
                return Err(crate::Error::Cancelled);
            }

            let t0 = profile.then(std::time::Instant::now);
            let DitStep { pred_feat, stops } =
                self.dit_step(&mut state, inference_timesteps, cfg_value);
            pred_feats.push(pred_feat.clone());
            if profile {
                sync_barrier(pred_feat.clone().squeeze_dim::<3>(1).narrow(2, 0, 1).squeeze_dim(2));
            }
            let t1 = profile.then(std::time::Instant::now);

            let mut all_done = false;
            if i > min_len {
                for (b, &s) in stops.iter().enumerate() {
                    if s && !stopped[b] {
                        stopped[b] = true;
                        stop_steps[b] = i + 1; // include the stop-firing patch
                    }
                }
                all_done = stopped.iter().all(|s| *s);
            }

            if all_done {
                if let (Some(t0), Some(t1)) = (t0, t1) {
                    t_dit_ns += t1.duration_since(t0).as_nanos();
                    n_steps += 1;
                }
                break;
            }

            self.lm_step(&mut state, pred_feat);

            if let (Some(t0), Some(t1)) = (t0, t1) {
                sync_barrier(state.residual_hidden.clone());
                let t2 = std::time::Instant::now();
                t_dit_ns += t1.duration_since(t0).as_nanos();
                t_lm_ns += t2.duration_since(t1).as_nanos();
                n_steps += 1;
            }
        }

        // Any element that never stopped: treat its valid length as the
        // full number of patches we produced.
        let produced = pred_feats.len();
        for (b, s) in stop_steps.iter_mut().enumerate() {
            if !stopped[b] || *s > produced {
                *s = produced;
            }
        }

        if profile && n_steps > 0 {
            let ms = |ns: u128| (ns as f64) / 1e6;
            eprintln!(
                "[profile] AR steps={} dit+stop={:.1}ms lm_tail={:.1}ms avg_per_step: dit+stop={:.2}ms lm={:.2}ms",
                n_steps, ms(t_dit_ns), ms(t_lm_ns),
                ms(t_dit_ns) / n_steps as f64,
                ms(t_lm_ns) / n_steps as f64,
            );
        }

        Ok((Self::stack_pred_feats(&pred_feats), stop_steps))
    }
}

/// Per-call autoregressive state produced by [`VoxCpm2Model::prefill`] and
/// consumed by [`VoxCpm2Model::dit_step`] / [`VoxCpm2Model::lm_step`].
///
/// You only need to touch this directly if you're driving inference manually
/// (e.g. for streaming or custom early-exit logic). The high-level
/// [`crate::VoxCPM::generate`] / [`crate::VoxCPM::generate_stream`] APIs
/// manage it for you.
#[derive(Debug)]
pub struct InferenceState<B: Backend> {
    /// `[B, lm_h]` — last hidden state of the base LM (input to DiT + stop).
    pub lm_hidden: Tensor<B, 2>,
    /// `[B, lm_h]` — last hidden state of the residual LM (input to DiT).
    pub residual_hidden: Tensor<B, 2>,
    /// `[B, P, D]` — last predicted patch, used as DiT prefix for the next step.
    pub prefix_feat_cond: Tensor<B, 3>,
    /// Static KV cache for the base LM. Sized for prefill + `max_len` steps.
    pub base_cache: crate::minicpm4::StaticKvCache<B>,
    /// Static KV cache for the residual LM.
    pub res_cache: crate::minicpm4::StaticKvCache<B>,
    /// Number of [`VoxCpm2Model::lm_step`] calls applied so far.
    pub steps_taken: usize,
    /// `[B, max_ctx]` bool mask: `true` = position is padding from a
    /// batched prefill and must be excluded from attention. `None` for
    /// the unbatched/serial path.
    pub key_padding_mask: Option<Tensor<B, 2, burn::tensor::Bool>>,
}

/// Output of [`VoxCpm2Model::dit_step`].
#[derive(Debug)]
pub struct DitStep<B: Backend> {
    /// `[B, 1, P, D]` — the patch the diffusion sampler produced this step,
    /// one per batch element.
    pub pred_feat: Tensor<B, 4>,
    /// One bit per batch element: `true` if the stop head argmax fired this
    /// step for that element. The caller decides whether to honor it (e.g.
    /// ignore until `min_len` patches are out).
    pub stops: Vec<bool>,
}

