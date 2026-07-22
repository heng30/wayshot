//! Rotary position embedding (RoPE).

use crate::error::Result;
use candle_core::{D, DType, Device, Tensor};

/// Compute default inverse-frequency vector for RoPE.
fn compute_default_rope_parameters(dim: usize, base: f32) -> Vec<f32> {
    (0..dim)
        .step_by(2)
        .map(|i| 1.0_f32 / base.powf(i as f32 / dim as f32))
        .collect()
}

fn rotate_half(x: &Tensor) -> Result<Tensor> {
    let half_dim = x.dim(D::Minus1)? / 2;
    let x1 = x.narrow(D::Minus1, 0, half_dim)?;
    let x2 = x.narrow(D::Minus1, half_dim, half_dim)?;
    let x2 = x2.affine(-1.0, 0.0)?;
    let rotate_x = Tensor::cat(&[&x2, &x1], D::Minus1)?.contiguous()?;
    Ok(rotate_x)
}

/// Apply rotary position embedding to query/key tensors.
///
/// `q`/`k` shape: `(bs, num_heads, seq_len, head_dim)`
/// `cos`/`sin` shape: `(seq_len, head_dim)` — will be broadcast.
pub fn apply_rotary_pos_emb(
    q: &Tensor,
    k: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
    tof32: bool,
) -> Result<(Tensor, Tensor)> {
    let mut cos = cos.clone();
    let mut sin = sin.clone();
    if cos.rank() == 2 {
        cos = cos.unsqueeze(0)?.unsqueeze(0)?;
        sin = sin.unsqueeze(0)?.unsqueeze(0)?;
    }
    if cos.rank() == 3 {
        cos = cos.unsqueeze(1)?;
        sin = sin.unsqueeze(1)?;
    }
    let orig_dtype = q.dtype();
    let q = if tof32 { &q.to_dtype(DType::F32)? } else { q };
    let k = if tof32 { &k.to_dtype(DType::F32)? } else { k };
    let cos = cos.to_dtype(q.dtype())?;
    let sin = sin.to_dtype(q.dtype())?;
    let q_embed = q
        .broadcast_mul(&cos)?
        .add(&rotate_half(q)?.broadcast_mul(&sin)?)?
        .to_dtype(orig_dtype)?;
    let k_embed = k
        .broadcast_mul(&cos)?
        .add(&rotate_half(k)?.broadcast_mul(&sin)?)?
        .to_dtype(orig_dtype)?;
    Ok((q_embed, k_embed))
}

/// Standard RoPE module.
pub struct RoPE {
    inv_freq: Tensor,
}

impl RoPE {
    pub fn new(dim: usize, theta_base: f32, device: &Device) -> Result<Self> {
        let inv_freq = compute_default_rope_parameters(dim, theta_base);
        let inv_freq = Tensor::from_slice(&inv_freq, (1, inv_freq.len()), device)?;
        Ok(Self { inv_freq })
    }

    pub fn forward(
        &self,
        seqlen_offset: usize,
        seq_len: usize,
        device: &Device,
    ) -> Result<(Tensor, Tensor)> {
        let positions = Tensor::arange(
            seqlen_offset as f32,
            (seqlen_offset + seq_len) as f32,
            self.inv_freq.device(),
        )?
        .reshape((seq_len, 1))?;
        let freqs = positions.matmul(&self.inv_freq)?;
        let emb = Tensor::cat(&[&freqs, &freqs], D::Minus1)?
            .contiguous()?
            .to_device(device)?;
        let cos = emb.cos()?;
        let sin = emb.sin()?;
        Ok((cos, sin))
    }
}
