//! Neural-network building blocks reused across models.

use crate::rope::rope::apply_rotary_pos_emb;
use crate::util::tensor_utils::repeat_kv;
use crate::error::Result;
use candle_core::{D, Tensor};
use candle_nn::{Activation, Conv1d, LayerNorm, Linear, Module, RmsNorm, VarBuilder, conv1d, linear_b, rms_norm};

// ── Gate-UpDown MLP (SwiGLU style) ──────────────────────────────────

pub struct GateUpDownMLP {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
    act: Activation,
}

impl GateUpDownMLP {
    pub fn new(
        vb: VarBuilder,
        hidden_size: usize,
        intermediate_size: usize,
        act: Activation,
        bias: bool,
        gate_name: Option<&str>,
        up_name: Option<&str>,
        down_name: Option<&str>,
    ) -> Result<Self> {
        let gate_proj = linear_b(
            hidden_size,
            intermediate_size,
            bias,
            vb.pp(gate_name.unwrap_or("gate_proj")),
        )?;
        let up_proj = linear_b(
            hidden_size,
            intermediate_size,
            bias,
            vb.pp(up_name.unwrap_or("up_proj")),
        )?;
        let down_proj = linear_b(
            intermediate_size,
            hidden_size,
            bias,
            vb.pp(down_name.unwrap_or("down_proj")),
        )?;
        Ok(Self {
            gate_proj,
            up_proj,
            down_proj,
            act,
        })
    }

    pub fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let gate = self.gate_proj.forward(xs)?.apply(&self.act)?;
        let up = self.up_proj.forward(xs)?;
        let xs = gate.mul(&up)?;
        let xs = self.down_proj.forward(&xs)?;
        Ok(xs)
    }
}

// ── Two-linear MLP block ────────────────────────────────────────────

pub struct TwoLinearMLP {
    fc1: Linear,
    fc2: Linear,
    act: Activation,
}

impl TwoLinearMLP {
    pub fn new(
        vb: VarBuilder,
        in_size: usize,
        hidden_size: usize,
        out_size: usize,
        act: Activation,
        bias: bool,
        fc1_name: &str,
        fc2_name: &str,
    ) -> Result<Self> {
        let fc1 = linear_b(in_size, hidden_size, bias, vb.pp(fc1_name))?;
        let fc2 = linear_b(hidden_size, out_size, bias, vb.pp(fc2_name))?;
        Ok(Self { fc1, fc2, act })
    }

    pub fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let xs = self.fc1.forward(xs)?.apply(&self.act)?;
        let xs = self.fc2.forward(&xs)?;
        Ok(xs)
    }
}

// ── Naive attention (standard MHA) ──────────────────────────────────
// Used by the vision encoder — no KV cache.

pub struct NaiveAttention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    num_heads: usize,
    num_kv_heads: usize,
    num_kv_groups: usize,
    head_dim: usize,
    scaling: f64,
}

impl NaiveAttention {
    pub fn new(
        vb: VarBuilder,
        hidden_size: usize,
        num_heads: usize,
        num_kv_heads: Option<usize>,
        head_dim: Option<usize>,
        bias: bool,
        q_name: Option<&str>,
        k_name: Option<&str>,
        v_name: Option<&str>,
        o_name: Option<&str>,
    ) -> Result<Self> {
        let num_kv_heads = num_kv_heads.unwrap_or(num_heads);
        let num_kv_groups = num_heads / num_kv_heads;
        let head_dim = head_dim.unwrap_or(hidden_size / num_heads);
        let scaling = 1f64 / f64::sqrt(head_dim as f64);
        let q_proj = linear_b(
            hidden_size,
            num_heads * head_dim,
            bias,
            vb.pp(q_name.unwrap_or("q_proj")),
        )?;
        let k_proj = linear_b(
            hidden_size,
            num_kv_heads * head_dim,
            bias,
            vb.pp(k_name.unwrap_or("k_proj")),
        )?;
        let v_proj = linear_b(
            hidden_size,
            num_kv_heads * head_dim,
            bias,
            vb.pp(v_name.unwrap_or("v_proj")),
        )?;
        let o_proj = linear_b(
            num_heads * head_dim,
            hidden_size,
            bias,
            vb.pp(o_name.unwrap_or("o_proj")),
        )?;
        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            num_heads,
            num_kv_heads,
            num_kv_groups,
            head_dim,
            scaling,
        })
    }

    /// Vision encoder forward — no cache, no RoPE (cos/sin unused by vision,
    /// but kept for API compatibility with the original code).
    pub fn forward(
        &self,
        xs: &Tensor,
        cos: Option<&Tensor>,
        sin: Option<&Tensor>,
        attention_mask: Option<&Tensor>,
        tof32: bool,
    ) -> Result<Tensor> {
        let (b_sz, q_len, _) = xs.dims3()?;
        let query_states = self.q_proj.forward(xs)?;
        let key_states = self.k_proj.forward(xs)?;
        let value_states = self.v_proj.forward(xs)?;

        let query_states = query_states
            .reshape((b_sz, q_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let key_states = key_states
            .reshape((b_sz, q_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;
        let value_states = value_states
            .reshape((b_sz, q_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        let (query_states, key_states) = if let (Some(cos), Some(sin)) = (cos, sin) {
            apply_rotary_pos_emb(&query_states, &key_states, cos, sin, tof32)?
        } else {
            (query_states, key_states)
        };

        let attn_output = eager_attention_forward(
            &query_states,
            &key_states,
            &value_states,
            Some(self.num_kv_groups),
            attention_mask,
            self.scaling,
        )?;
        // eager_attention_forward already transposes (1,2) so attn_output
        // is (B, seq_len, num_heads, head_dim) — just reshape.
        let attn_output = attn_output
            .reshape((b_sz, q_len, self.num_heads * self.head_dim))?
            .contiguous()?;
        let attn_output = self.o_proj.forward(&attn_output)?;
        Ok(attn_output)
    }
}

// ── QK-norm attention (used by LFM2 text decoder) ───────────────────

pub struct QKNormAttention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    q_norm: RmsNorm,
    k_norm: RmsNorm,
    num_heads: usize,
    num_kv_heads: usize,
    num_kv_groups: usize,
    head_dim: usize,
    scaling: f64,
    cache: Option<(Tensor, Tensor)>,
}

impl QKNormAttention {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vb: VarBuilder,
        hidden_size: usize,
        num_heads: usize,
        head_dim: Option<usize>,
        num_kv_heads: Option<usize>,
        bias: bool,
        norm_eps: f64,
        q_name: Option<&str>,
        k_name: Option<&str>,
        v_name: Option<&str>,
        o_name: Option<&str>,
        q_norm_name: Option<&str>,
        k_norm_name: Option<&str>,
    ) -> Result<Self> {
        let head_dim = head_dim.unwrap_or(hidden_size / num_heads);
        let num_kv_heads = num_kv_heads.unwrap_or(num_heads);
        let num_kv_groups = num_heads / num_kv_heads;
        let scaling = 1f64 / f64::sqrt(head_dim as f64);
        let q_proj = linear_b(
            hidden_size,
            num_heads * head_dim,
            bias,
            vb.pp(q_name.unwrap_or("q_proj")),
        )?;
        let k_proj = linear_b(
            hidden_size,
            num_kv_heads * head_dim,
            bias,
            vb.pp(k_name.unwrap_or("k_proj")),
        )?;
        let v_proj = linear_b(
            hidden_size,
            num_kv_heads * head_dim,
            bias,
            vb.pp(v_name.unwrap_or("v_proj")),
        )?;
        let o_proj = linear_b(
            num_heads * head_dim,
            hidden_size,
            bias,
            vb.pp(o_name.unwrap_or("o_proj")),
        )?;
        let q_norm = rms_norm(head_dim, norm_eps, vb.pp(q_norm_name.unwrap_or("q_norm")))?;
        let k_norm = rms_norm(head_dim, norm_eps, vb.pp(k_norm_name.unwrap_or("k_norm")))?;
        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            q_norm,
            k_norm,
            num_heads,
            num_kv_heads,
            num_kv_groups,
            head_dim,
            scaling,
            cache: None,
        })
    }

    pub fn forward(
        &mut self,
        xs: &Tensor,
        cos: &Tensor,
        sin: &Tensor,
        attention_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let (b_sz, q_len, _) = xs.dims3()?;
        let query_states = self.q_proj.forward(xs)?;
        let key_states = self.k_proj.forward(xs)?;
        let value_states = self.v_proj.forward(xs)?;

        let query_states = query_states
            .reshape((b_sz, q_len, self.num_heads, self.head_dim))?;
        let query_states = self.q_norm.forward(&query_states)?.transpose(1, 2)?;
        let key_states = key_states
            .reshape((b_sz, q_len, self.num_kv_heads, self.head_dim))?;
        let key_states = self.k_norm.forward(&key_states)?.transpose(1, 2)?;
        let value_states = value_states
            .reshape((b_sz, q_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        let (query_states, key_states) =
            apply_rotary_pos_emb(&query_states, &key_states, cos, sin, false)?;

        let (key_states, value_states) = match &self.cache {
            None => (key_states, value_states),
            Some((prev_k, prev_v)) => {
                let k = Tensor::cat(&[prev_k, &key_states], 2)?;
                let v = Tensor::cat(&[prev_v, &value_states], 2)?;
                (k, v)
            }
        };

        let attn_output = eager_attention_forward(
            &query_states,
            &key_states,
            &value_states,
            Some(self.num_kv_groups),
            attention_mask,
            self.scaling,
        )?;

        // Store K/V for next step (after attention so we don't clone unnecessarily).
        // Tensor::clone() is cheap — it only increments the reference count.
        self.cache = Some((key_states.clone(), value_states.clone()));
        // eager_attention_forward already transposes (1,2) so attn_output
        // is (B, seq_len, num_heads, head_dim) — just reshape.
        let attn_output = attn_output
            .reshape((b_sz, q_len, self.num_heads * self.head_dim))?
            .contiguous()?;
        let attn_output = self.o_proj.forward(&attn_output)?;
        Ok(attn_output)
    }

    pub fn clear_kv_cache(&mut self) {
        self.cache = None;
    }
}

// ── NaiveAttn + TwoLinearMLP block (vision encoder layer) ───────────

pub struct NaiveAttnTwoLinearMLPBlock {
    self_attn: NaiveAttention,
    mlp: TwoLinearMLP,
    input_layernorm: LayerNorm,
    post_attention_layernorm: LayerNorm,
}

#[allow(clippy::too_many_arguments)]
impl NaiveAttnTwoLinearMLPBlock {
    pub fn new(
        vb: VarBuilder,
        hidden_size: usize,
        num_attention_heads: usize,
        num_key_value_heads: Option<usize>,
        head_dim: Option<usize>,
        attn_bias: bool,
        attn_pp_name: &str,
        o_proj_pp_name: Option<&str>,
        intermediate_size: usize,
        hidden_act: Activation,
        mlp_bias: bool,
        mlp_pp_name: &str,
        linear1_pp_name: &str,
        linear2_pp_name: &str,
        norm_eps: f64,
        input_norm_pp_name: &str,
        post_norm_pp_name: &str,
    ) -> Result<Self> {
        let self_attn = NaiveAttention::new(
            vb.pp(attn_pp_name),
            hidden_size,
            num_attention_heads,
            num_key_value_heads,
            head_dim,
            attn_bias,
            None,
            None,
            None,
            o_proj_pp_name,
        )?;
        let mlp = TwoLinearMLP::new(
            vb.pp(mlp_pp_name),
            hidden_size,
            intermediate_size,
            hidden_size,
            hidden_act,
            mlp_bias,
            linear1_pp_name,
            linear2_pp_name,
        )?;
        let input_layernorm =
            get_layer_norm(vb.pp(input_norm_pp_name), norm_eps, hidden_size, true)?;
        let post_attention_layernorm =
            get_layer_norm(vb.pp(post_norm_pp_name), norm_eps, hidden_size, true)?;
        Ok(Self {
            self_attn,
            mlp,
            input_layernorm,
            post_attention_layernorm,
        })
    }

    pub fn forward(
        &self,
        xs: &Tensor,
        cos: Option<&Tensor>,
        sin: Option<&Tensor>,
        attention_mask: Option<&Tensor>,
        tof32: bool,
    ) -> Result<Tensor> {
        let residual = xs.clone();
        let xs = self.input_layernorm.forward(xs)?;
        let xs = self.self_attn.forward(&xs, cos, sin, attention_mask, tof32)?;
        let residual = residual.add(&xs)?;
        let xs = self.post_attention_layernorm.forward(&residual)?;
        let xs = self.mlp.forward(&xs)?;
        let xs = residual.add(&xs)?;
        Ok(xs)
    }
}

// ── Helper: create LayerNorm ────────────────────────────────────────

pub fn get_layer_norm(
    vb: VarBuilder,
    eps: f64,
    size: usize,
    _affine: bool,
) -> Result<LayerNorm> {
    let ln = candle_nn::layer_norm(size, eps, vb)?;
    Ok(ln)
}

// ── Helper: get Conv1d for short-conv layers ────────────────────────

pub fn get_conv1d(
    vb: VarBuilder,
    in_c: usize,
    out_c: usize,
    kernel_size: usize,
    padding: usize,
    stride: usize,
    dilation: usize,
    groups: usize,
    bias: bool,
) -> Result<Conv1d> {
    let cfg = candle_nn::Conv1dConfig {
        padding,
        stride,
        dilation,
        groups,
        ..Default::default()
    };
    let conv = if bias {
        conv1d(in_c, out_c, kernel_size, cfg, vb)?
    } else {
        conv1d_no_bias(in_c, out_c, kernel_size, cfg, vb)?
    };
    Ok(conv)
}

fn conv1d_no_bias(
    in_c: usize,
    out_c: usize,
    k: usize,
    cfg: candle_nn::Conv1dConfig,
    vb: VarBuilder,
) -> Result<Conv1d> {
    let weight = vb.get((out_c, in_c / cfg.groups, k), "weight")?;
    Ok(Conv1d::new(weight, None, cfg))
}

// ── Eager attention forward ─────────────────────────────────────────

pub fn eager_attention_forward(
    query_states: &Tensor,
    key_states: &Tensor,
    value_states: &Tensor,
    num_kv_groups: Option<usize>,
    attention_mask: Option<&Tensor>,
    scaling: f64,
) -> Result<Tensor> {
    let key_states = match num_kv_groups {
        Some(g) => repeat_kv(&key_states, g)?,
        None => key_states.clone(),
    };
    let value_states = match num_kv_groups {
        Some(g) => repeat_kv(&value_states, g)?,
        None => value_states.clone(),
    };
    // Ensure all inputs are contiguous for matmul (required by gemm backend).
    let query_states = query_states.contiguous()?;
    let key_states_t = key_states.transpose(D::Minus2, D::Minus1)?.contiguous()?;
    let value_states = value_states.contiguous()?;

    let attn_weights = query_states.matmul(&key_states_t)?;
    let attn_weights = (attn_weights * scaling)?;
    let attn_weights = match attention_mask {
        None => attn_weights,
        Some(mask) => attn_weights.broadcast_add(&mask.to_dtype(attn_weights.dtype())?)?,
    };
    let attn_weights = candle_nn::ops::softmax_last_dim(&attn_weights)?;
    let attn_output = attn_weights.matmul(&value_states)?;
    let attn_output = attn_output.transpose(1, 2)?.contiguous()?;
    Ok(attn_output)
}
