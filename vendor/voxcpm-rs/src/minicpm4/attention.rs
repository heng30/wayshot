//! Grouped-query multi-head attention with rotary embeddings.

use crate::config::MiniCpm4Config;
use crate::minicpm4::rope::apply_rotary_pos_emb;
use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;
use burn::tensor::activation::softmax;
use burn::tensor::TensorData;

/// `(key_cache, value_cache)` for a single layer, shape `[B, num_kv_heads, S_max, head_dim]`.
pub type LayerKv<B> = (Tensor<B, 4>, Tensor<B, 4>);

#[derive(Module, Debug)]
pub struct MiniCpmAttention<B: Backend> {
    /// Fused Q/K/V projection: `hidden -> (num_heads + 2*num_kv_heads) * head_dim`.
    ///
    /// Stored in the checkpoint as three separate `q_proj`/`k_proj`/`v_proj`
    /// weights; the loader concatenates them along the output dim at load
    /// time (see `src/weights.rs`). Fusing here saves 2 of every 3 linear
    /// kernel launches per attention layer — a significant win on WGPU where
    /// per-op launch overhead dominates at batch=1/seq=1 decode.
    pub qkv_proj: Linear<B>,
    pub o_proj: Linear<B>,
    pub num_heads: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub scale: f64,
    /// Cached split offsets for narrowing the fused qkv output.
    pub q_size: usize,  // num_heads * head_dim
    pub kv_size: usize, // num_kv_heads * head_dim
}

impl<B: Backend> MiniCpmAttention<B> {
    pub fn new(config: &MiniCpm4Config, device: &B::Device) -> Self {
        let head_dim = config.head_dim();
        let num_heads = config.num_attention_heads;
        let num_kv_heads = config.num_key_value_heads;
        let hidden = config.hidden_size;
        let q_size = num_heads * head_dim;
        let kv_size = num_kv_heads * head_dim;

        let qkv_proj = LinearConfig::new(hidden, q_size + 2 * kv_size).with_bias(false).init(device);
        let o_proj = LinearConfig::new(q_size, hidden).with_bias(false).init(device);

        Self {
            qkv_proj, o_proj,
            num_heads, num_kv_heads, head_dim,
            scale: 1.0 / (head_dim as f64).sqrt(),
            q_size, kv_size,
        }
    }

    /// Prefill pass. Returns `(out, (k, v))` where (k, v) are pre-RoPE-applied
    /// per-head tensors of shape `[B, Hkv, S, D]`.
    pub fn forward(
        &self,
        hidden_states: Tensor<B, 3>,
        position_emb: Option<(Tensor<B, 2>, Tensor<B, 2>)>,
        is_causal: bool,
    ) -> (Tensor<B, 3>, LayerKv<B>) {
        let [bsz, q_len, _] = hidden_states.dims();

        // Fused qkv: one matmul instead of three. Split along the last dim.
        let qkv = self.qkv_proj.forward(hidden_states);
        let q = qkv.clone().slice([0..bsz, 0..q_len, 0..self.q_size]);
        let k = qkv.clone().slice([0..bsz, 0..q_len, self.q_size..self.q_size + self.kv_size]);
        let v = qkv.slice([0..bsz, 0..q_len, self.q_size + self.kv_size..self.q_size + 2 * self.kv_size]);

        let q = q.reshape([bsz, q_len, self.num_heads, self.head_dim]).swap_dims(1, 2);
        let k = k.reshape([bsz, q_len, self.num_kv_heads, self.head_dim]).swap_dims(1, 2);
        let v = v.reshape([bsz, q_len, self.num_kv_heads, self.head_dim]).swap_dims(1, 2);

        let (q, k) = if let Some((cos, sin)) = position_emb {
            apply_rotary_pos_emb(q, k, cos, sin)
        } else {
            (q, k)
        };

        let n_rep = self.num_heads / self.num_kv_heads;
        let k_full = repeat_kv(k.clone(), n_rep);
        let v_full = repeat_kv(v.clone(), n_rep);

        let attn = self.sdpa(q, k_full, v_full, is_causal, None);

        let attn = attn.swap_dims(1, 2).reshape([bsz, q_len, self.num_heads * self.head_dim]);
        let out = self.o_proj.forward(attn);
        (out, (k, v))
    }

    /// Single-step decoding against a cached KV tensor.
    ///
    /// `key_padding_mask`, when present, has shape `[B, S_max]` and entries
    /// `true` indicate cache positions that should be masked out (i.e.
    /// padding from a batched prefill where different rows had different
    /// real lengths). The mask is sliced to the populated cache range
    /// before being applied.
    pub fn forward_step(
        &self,
        hidden_states: Tensor<B, 2>,
        position_emb: Option<(Tensor<B, 2>, Tensor<B, 2>)>,
        position_id: usize,
        kv_cache: &mut Option<LayerKv<B>>,
        key_padding_mask: Option<Tensor<B, 2, burn::tensor::Bool>>,
    ) -> Tensor<B, 2> {
        let [bsz, _] = hidden_states.dims();
        let device = hidden_states.device();

        // Fused qkv: one matmul, split along the last (hidden) dim.
        let qkv = self.qkv_proj.forward(hidden_states);
        let q = qkv.clone().slice([0..bsz, 0..self.q_size]);
        let k = qkv.clone().slice([0..bsz, self.q_size..self.q_size + self.kv_size]);
        let v = qkv.slice([0..bsz, self.q_size + self.kv_size..self.q_size + 2 * self.kv_size]);

        let q: Tensor<B, 4> = q.reshape([bsz, 1, self.num_heads, self.head_dim]).swap_dims(1, 2);
        let k: Tensor<B, 4> = k.reshape([bsz, 1, self.num_kv_heads, self.head_dim]).swap_dims(1, 2);
        let v: Tensor<B, 4> = v.reshape([bsz, 1, self.num_kv_heads, self.head_dim]).swap_dims(1, 2);

        let (q, k) = if let Some((cos, sin)) = position_emb {
            apply_rotary_pos_emb(q, k, cos, sin)
        } else {
            (q, k)
        };

        // Take ownership of the cache buffers so refcount drops to 1 and
        // `slice_assign` can reuse the underlying GPU allocation in place.
        let (key_cache, value_cache) = kv_cache.take().expect("cache layer present");
        let key_cache = key_cache.slice_assign(
            [0..bsz, 0..self.num_kv_heads, position_id..position_id + 1, 0..self.head_dim],
            k,
        );
        let value_cache = value_cache.slice_assign(
            [0..bsz, 0..self.num_kv_heads, position_id..position_id + 1, 0..self.head_dim],
            v,
        );

        let max_len = key_cache.dims()[2];

        let n_rep = self.num_heads / self.num_kv_heads;
        // Slice cache to the populated range so SDPA only attends over actual
        // positions (avoids O(max_length) compute every step).
        let cur_len = position_id + 1;
        let k_view = key_cache
            .clone()
            .slice([0..bsz, 0..self.num_kv_heads, 0..cur_len, 0..self.head_dim]);
        let v_view = value_cache
            .clone()
            .slice([0..bsz, 0..self.num_kv_heads, 0..cur_len, 0..self.head_dim]);
        let k_full = repeat_kv(k_view, n_rep);
        let v_full = repeat_kv(v_view, n_rep);

        // Restore the (single-ref) buffers to the cache before consuming the
        // clones in SDPA — this lets the next step's slice_assign be in-place.
        *kv_cache = Some((key_cache, value_cache));

        // Build the per-batch attention mask from key_padding_mask (if any).
        // Shape after reshape: [B, 1, 1, cur_len]. sdpa's `expand` will
        // broadcast across heads and the singleton query dim.
        let attn_mask: Option<Tensor<B, 4, burn::tensor::Bool>> = key_padding_mask.map(|m| {
            // m is [B, S_max]; slice to populated range.
            let m = m.slice([0..bsz, 0..cur_len]);
            m.reshape([bsz, 1, 1, cur_len])
        });
        let _ = (max_len, device);
        let attn = self.sdpa(q, k_full, v_full, false, attn_mask);

        let attn = attn.swap_dims(1, 2).reshape([bsz, self.num_heads * self.head_dim]);
        self.o_proj.forward(attn)
    }

    fn sdpa(
        &self,
        q: Tensor<B, 4>,
        k: Tensor<B, 4>,
        v: Tensor<B, 4>,
        is_causal: bool,
        attn_mask: Option<Tensor<B, 4, burn::tensor::Bool>>,
    ) -> Tensor<B, 4> {
        let [b, h, s_q, _d] = q.dims();
        let s_k = k.dims()[2];
        let scores = q.matmul(k.swap_dims(2, 3)).mul_scalar(self.scale);

        let scores = if is_causal {
            let device = scores.device();
            let mut data = vec![false; s_q * s_k];
            for i in 0..s_q {
                for j in 0..s_k {
                    data[i * s_k + j] = j > i;
                }
            }
            let mask = Tensor::<B, 1, burn::tensor::Bool>::from_data(
                TensorData::new(data, [s_q * s_k]),
                &device,
            )
            .reshape([1usize, 1, s_q, s_k]);
            let bcast: Tensor<B, 4, burn::tensor::Bool> = mask.expand([b, h, s_q, s_k]);
            scores.mask_fill(bcast, f32::NEG_INFINITY)
        } else if let Some(mask) = attn_mask {
            let bcast: Tensor<B, 4, burn::tensor::Bool> = mask.expand([b, h, s_q, s_k]);
            scores.mask_fill(bcast, f32::NEG_INFINITY)
        } else {
            scores
        };

        let attn = softmax(scores, 3);
        attn.matmul(v)
    }
}

/// Repeat the key/value heads along the head axis so GQA can be done as a normal MHA.
pub fn repeat_kv<B: Backend>(x: Tensor<B, 4>, n_rep: usize) -> Tensor<B, 4> {
    if n_rep == 1 {
        return x;
    }
    let [b, hkv, s, d] = x.dims();
    let x: Tensor<B, 5> = x.unsqueeze_dim(2);
    let x = x.expand([b, hkv, n_rep, s, d]);
    x.reshape([b, hkv * n_rep, s, d])
}
