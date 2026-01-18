use crate::error::{FunAsrError, Result};
use candle_core::{D, Tensor};
use candle_nn::{
    Activation, BatchNorm, BatchNormConfig, Conv1d, Conv1dConfig, Conv2d, Conv2dConfig, LayerNorm,
    LayerNormConfig, Linear, Module, RmsNorm, VarBuilder, batch_norm, conv1d, conv1d_no_bias,
    conv2d, conv2d_no_bias, layer_norm, linear_b, rms_norm,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::position_embed::rope::apply_rotary_pos_emb;
use tensor_utils::repeat_kv;

#[derive(Debug, Clone)]
pub struct GateUpDownMLP {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
    act_fn: Activation,
}

impl GateUpDownMLP {
    pub fn new(
        vb: VarBuilder,
        hidden_size: usize,
        intermediate_size: usize,
        act_fn: Activation,
        bias: bool,
    ) -> Result<Self> {
        let gate_proj = linear_b(hidden_size, intermediate_size, bias, vb.pp("gate_proj"))?;
        let up_proj = linear_b(hidden_size, intermediate_size, bias, vb.pp("up_proj"))?;
        let down_proj = linear_b(intermediate_size, hidden_size, bias, vb.pp("down_proj"))?;
        Ok(Self {
            gate_proj,
            up_proj,
            down_proj,
            act_fn,
        })
    }
}

impl Module for GateUpDownMLP {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let lhs = xs.apply(&self.gate_proj)?.apply(&self.act_fn)?;
        let rhs = xs.apply(&self.up_proj)?;
        (lhs * rhs)?.apply(&self.down_proj)
    }
}

pub struct TwoLinearMLP {
    linear1: Linear,
    linear2: Linear,
    act: Activation,
}

impl TwoLinearMLP {
    pub fn new(
        vb: VarBuilder,
        in_dim: usize,
        middle_dim: usize,
        out_dim: usize,
        act: Activation,
        bias: bool,
        linear1_pp_name: &str,
        linear2_pp_name: &str,
    ) -> Result<Self> {
        let linear1 = linear_b(in_dim, middle_dim, bias, vb.pp(linear1_pp_name))?;
        let linear2 = linear_b(middle_dim, out_dim, bias, vb.pp(linear2_pp_name))?;

        Ok(Self {
            linear1,
            linear2,
            act,
        })
    }
    pub fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let xs = xs
            .apply(&self.linear1)?
            .apply(&self.act)?
            .apply(&self.linear2)?;
        Ok(xs)
    }
}

#[derive(Debug, Clone)]
pub struct NaiveAttention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    num_heads: usize,
    num_kv_heads: usize,
    num_kv_groups: usize,
    head_dim: usize,
    middle_size: usize,
    kv_cache: Option<(Tensor, Tensor)>,
}

impl NaiveAttention {
    pub fn new(
        vb: VarBuilder,
        hidden_size: usize,
        num_attention_heads: usize,
        num_key_value_heads: usize,
        head_dim: Option<usize>,
        bias: bool,
        q_proj_pp_name: Option<&str>,
        k_proj_pp_name: Option<&str>,
        v_proj_pp_name: Option<&str>,
        o_proj_pp_name: Option<&str>,
    ) -> Result<Self> {
        let num_kv_groups = num_attention_heads / num_key_value_heads;
        let head_dim = match head_dim {
            None => hidden_size / num_attention_heads,
            Some(dim) => dim,
        };
        let q_proj_pp_name = q_proj_pp_name.unwrap_or("q_proj");
        let k_proj_pp_name = k_proj_pp_name.unwrap_or("k_proj");
        let v_proj_pp_name = v_proj_pp_name.unwrap_or("v_proj");
        let o_proj_pp_name = o_proj_pp_name.unwrap_or("o_proj");
        let q_proj = linear_b(
            hidden_size,
            num_attention_heads * head_dim,
            bias,
            vb.pp(q_proj_pp_name),
        )?;
        let k_proj = linear_b(
            hidden_size,
            num_key_value_heads * head_dim,
            bias,
            vb.pp(k_proj_pp_name),
        )?;
        let v_proj = linear_b(
            hidden_size,
            num_key_value_heads * head_dim,
            bias,
            vb.pp(v_proj_pp_name),
        )?;
        let o_proj = linear_b(
            num_attention_heads * head_dim,
            hidden_size,
            bias,
            vb.pp(o_proj_pp_name),
        )?;

        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            num_heads: num_attention_heads,
            num_kv_heads: num_key_value_heads,
            num_kv_groups,
            head_dim,
            middle_size: num_attention_heads * head_dim,
            kv_cache: None,
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
        let (query_states, key_states) = if let Some(cos) = cos
            && let Some(sin) = sin
        {
            apply_rotary_pos_emb(&query_states, &key_states, cos, sin, tof32)?
        } else {
            (query_states, key_states)
        };

        let scale = 1f64 / f64::sqrt(self.head_dim as f64);
        let attn_output = eager_attention_forward(
            &query_states,
            &key_states,
            &value_states,
            Some(self.num_kv_groups),
            attention_mask,
            scale,
        )?;
        let attn_output = attn_output.reshape((b_sz, q_len, self.middle_size))?;
        let attn_output = attn_output.apply(&self.o_proj)?;
        Ok(attn_output)
    }

    pub fn forward_with_cache(
        &mut self,
        xs: &Tensor,
        cos: &Tensor,
        sin: &Tensor,
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
        let (query_states, key_states) =
            apply_rotary_pos_emb(&query_states, &key_states, cos, sin, tof32)?;
        let (key_states, value_states) = match &self.kv_cache {
            None => (key_states, value_states),
            Some((prev_k, prev_v)) => {
                let key_states = Tensor::cat(&[prev_k, &key_states], 2)?;
                let value_states = Tensor::cat(&[prev_v, &value_states], 2)?;
                (key_states, value_states)
            }
        };

        self.kv_cache = Some((key_states.clone(), value_states.clone()));
        let scale = 1f64 / f64::sqrt(self.head_dim as f64);
        let attn_output = eager_attention_forward(
            &query_states,
            &key_states,
            &value_states,
            Some(self.num_kv_groups),
            attention_mask,
            scale,
        )?;
        let attn_output = attn_output.reshape((b_sz, q_len, self.middle_size))?;
        let attn_output = attn_output.apply(&self.o_proj)?;
        Ok(attn_output)
    }

    pub fn clear_kv_cache(&mut self) {
        self.kv_cache = None
    }
}

pub struct NaiveAttnGateUpDownMLPBlock {
    self_attn: NaiveAttention,
    mlp: GateUpDownMLP,
    input_layernorm: RmsNorm,
    post_attention_layernorm: RmsNorm,
}

impl NaiveAttnGateUpDownMLPBlock {
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
        norm_eps: f64,
        input_norm_pp_name: &str,
        post_norm_pp_name: &str,
    ) -> Result<Self> {
        let num_key_value_heads = match num_key_value_heads {
            Some(heads) => heads,
            None => num_attention_heads,
        };
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
        let mlp = GateUpDownMLP::new(
            vb.pp(mlp_pp_name),
            hidden_size,
            intermediate_size,
            hidden_act,
            mlp_bias,
        )?;
        let input_layernorm = rms_norm(hidden_size, norm_eps, vb.pp(input_norm_pp_name))?;
        let post_attention_layernorm = rms_norm(hidden_size, norm_eps, vb.pp(post_norm_pp_name))?;
        Ok(Self {
            self_attn,
            mlp,
            input_layernorm,
            post_attention_layernorm,
        })
    }

    pub fn forward(
        &mut self,
        xs: &Tensor,
        cos: &Tensor,
        sin: &Tensor,
        attention_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let residual = xs.clone();
        let xs = self.input_layernorm.forward(xs)?;
        let xs = self
            .self_attn
            .forward_with_cache(&xs, cos, sin, attention_mask, false)?;
        let residual = residual.add(&xs)?;
        let xs = self.post_attention_layernorm.forward(&residual)?;
        let xs = self.mlp.forward(&xs)?;
        let xs = residual.add(&xs)?;
        Ok(xs)
    }
    pub fn clear_kv_cache(&mut self) {
        self.self_attn.clear_kv_cache()
    }
}

pub fn eager_attention_forward(
    query_states: &Tensor,
    key_states: &Tensor,
    value_states: &Tensor,
    num_key_value_groups: Option<usize>,
    attention_mask: Option<&Tensor>,
    scaling: f64,
) -> Result<Tensor> {
    // input q shape:(b, num_head, seq_len, dim)
    // input k/v shape:(b, num_kv_head, seq_len, dim)
    let key_states = match num_key_value_groups {
        Some(g) => repeat_kv(key_states.clone(), g)?.contiguous()?,
        None => key_states.clone(),
    };
    let value_states = match num_key_value_groups {
        Some(g) => repeat_kv(value_states.clone(), g)?.contiguous()?,
        None => value_states.clone(),
    };
    let query_states = query_states.contiguous()?;
    let key_states = key_states.contiguous()?;
    let value_states = value_states.contiguous()?;

    let attn_weights = query_states.matmul(&key_states.transpose(D::Minus2, D::Minus1)?)?;
    let attn_weights = (attn_weights * scaling)?;
    let attn_weights = match attention_mask {
        None => attn_weights,
        Some(mask) => attn_weights.broadcast_add(&mask.to_dtype(attn_weights.dtype())?)?,
    };
    let attn_weights = candle_nn::ops::softmax_last_dim(&attn_weights)?;
    let attn_output = attn_weights.matmul(&value_states)?;
    //(b, n_head, seq_len, dim) -> (b, seq_len, n_head, dim)
    let attn_output = attn_output.transpose(1, 2)?.contiguous()?;

    Ok(attn_output)
}

pub fn get_conv2d(
    vb: VarBuilder,
    in_c: usize,
    out_c: usize,
    kernel_size: usize,
    padding: usize,
    stride: usize,
    dilation: usize,
    groups: usize,
    bias: bool,
) -> Result<Conv2d> {
    let cfg = Conv2dConfig {
        padding,
        stride,
        dilation,
        groups,
        cudnn_fwd_algo: None,
    };
    let conv2d = if bias {
        conv2d(in_c, out_c, kernel_size, cfg, vb)?
    } else {
        conv2d_no_bias(in_c, out_c, kernel_size, cfg, vb)?
    };
    Ok(conv2d)
}

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
    let cfg = Conv1dConfig {
        padding,
        stride,
        dilation,
        groups,
        cudnn_fwd_algo: None,
    };
    let conv1d = if bias {
        conv1d(in_c, out_c, kernel_size, cfg, vb)?
    } else {
        conv1d_no_bias(in_c, out_c, kernel_size, cfg, vb)?
    };
    Ok(conv1d)
}

pub fn get_layer_norm(vb: VarBuilder, eps: f64, dim: usize) -> Result<LayerNorm> {
    let ln_config = LayerNormConfig {
        eps,
        remove_mean: true, // true for layernorm, false for RMSNorm
        affine: true,      // true for with bias, false for without bias
    };
    let norm = layer_norm(dim, ln_config, vb)?;
    Ok(norm)
}

pub fn get_batch_norm(vb: VarBuilder, eps: f64, dim: usize) -> Result<BatchNorm> {
    let bn_config = BatchNormConfig {
        eps,
        remove_mean: true,
        affine: true,
        momentum: 0.1,
    };
    let norm = batch_norm(dim, bn_config, vb)?;
    Ok(norm)
}

pub fn conv1d_group_parallel(xs: &Tensor, conv1d: &Conv1d) -> Result<Tensor> {
    let groups = conv1d.config().groups;
    let xs = if groups == 1 {
        xs.conv1d_with_algo(
            conv1d.weight(),
            conv1d.config().padding,
            conv1d.config().stride,
            conv1d.config().dilation,
            groups,
            conv1d.config().cudnn_fwd_algo,
        )?
    } else {
        let blocks = xs.chunk(groups, 1)?;
        let kernel = conv1d.weight().chunk(groups, 0)?;
        let blocks = blocks
            .par_iter()
            .zip(&kernel)
            .map(|(block, kernel)| {
                block
                    .conv1d_with_algo(
                        kernel,
                        conv1d.config().padding,
                        conv1d.config().stride,
                        conv1d.config().dilation,
                        1,
                        conv1d.config().cudnn_fwd_algo,
                    )
                    .map_err(|e| FunAsrError::Model(format!("tensor conv1d_with_algo error:{}", e)))
            })
            .collect::<Result<Vec<Tensor>>>()?;
        Tensor::cat(&blocks, 1)?
    };
    match conv1d.bias() {
        None => Ok(xs),
        Some(bias) => {
            let b = bias.dims1()?;
            let bias = bias.reshape((1, b, 1))?;
            Ok(xs.broadcast_add(&bias)?)
        }
    }
}
