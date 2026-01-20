use crate::Result;
use candle_core::{D, DType, Device, Tensor};

pub mod rope {
    use super::*;

    pub struct RoPE {
        inv_freq: Tensor, // (1, dim / 2)
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
                device,
            )?
            .reshape((seq_len, 1))?; // (seq_len, 1)
            let freqs = positions.matmul(&self.inv_freq)?; // (seq_len, dim / 2)
            let emb = Tensor::cat(&[&freqs, &freqs], D::Minus1)?.contiguous()?; // (seq_len, dim)
            let cos = emb.cos()?;
            let sin = emb.sin()?;
            Ok((cos, sin))
        }
    }

    pub fn compute_default_rope_parameters(dim: usize, base: f32) -> Vec<f32> {
        let inv_freq: Vec<f32> = (0..dim)
            .step_by(2)
            .map(|i| 1.0_f32 / base.powf(i as f32 / dim as f32))
            .collect();
        inv_freq
    }

    pub fn rotate_half(x: &Tensor) -> Result<Tensor> {
        let half_dim = x.dim(D::Minus1)? / 2;
        let x1 = x.narrow(D::Minus1, 0, half_dim)?;
        let x2 = x.narrow(D::Minus1, half_dim, half_dim)?;
        let x2 = x2.affine(-1.0, 0.0)?;
        let rotate_x = Tensor::cat(&[&x2, &x1], D::Minus1)?.contiguous()?;
        Ok(rotate_x)
    }

    pub fn apply_rotary_pos_emb(
        q: &Tensor,
        k: &Tensor,
        cos: &Tensor,
        sin: &Tensor,
        tof32: bool,
    ) -> Result<(Tensor, Tensor)> {
        // sin/cos: to (bs, 1, seq_len, head_dim)
        // q/k: (bs, n_head, seq_len, head_dim)
        let mut cos = cos.clone();
        let mut sin = sin.clone();
        if cos.rank() == 2 {
            // (seq_len, head_dim) -> (1, 1, seq_len, head_dim)
            cos = cos.unsqueeze(0)?.unsqueeze(0)?;
            sin = sin.unsqueeze(0)?.unsqueeze(0)?;
        }
        if cos.rank() == 3 {
            // (bs, seq_len, head_dim) -> (bs, 1, seq_len, head_dim)
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
}

pub mod sinusoidal_pe {
    use super::rope::compute_default_rope_parameters;
    use super::*;

    pub struct SinusoidalPositionEncoderCat {
        inv_freq: Option<Tensor>, // (1, dim / 2)
    }

    impl SinusoidalPositionEncoderCat {
        pub fn new(dim: Option<usize>, save_freq: bool, device: &Device) -> Result<Self> {
            let inv_freq = if save_freq && let Some(dim) = dim {
                let inv_freq = compute_default_rope_parameters(dim, 10000.0);
                let inv_freq = Tensor::from_slice(&inv_freq, (1, inv_freq.len()), device)?;
                Some(inv_freq)
            } else {
                None
            };

            Ok(Self { inv_freq })
        }

        pub fn encode(
            &self,
            seqlen_offset: usize,
            seq_len: usize,
            head_dim: usize,
            device: &Device,
            dtype: DType,
        ) -> Result<Tensor> {
            let positions = Tensor::arange(
                seqlen_offset as f32,
                (seqlen_offset + seq_len) as f32,
                device,
            )?
            .reshape((seq_len, 1))?; // (seq_len, 1)
            let inv_freq = if self.inv_freq.is_none() {
                let inv_freq = compute_default_rope_parameters(head_dim, 10000.0);
                Tensor::from_slice(&inv_freq, (1, inv_freq.len()), device)?
            } else {
                self.inv_freq.as_ref().unwrap().clone()
            };
            let freqs = positions.matmul(&inv_freq)?; // (seq_len, dim / 2)
            let sin = freqs.sin()?;
            let cos = freqs.cos()?;
            let pos_embed = Tensor::cat(&[sin, cos], D::Minus1)?
                .contiguous()?
                .to_dtype(dtype)?;

            Ok(pos_embed)
        }
        pub fn forward(&self, xs: &Tensor, seqlen_offset: usize) -> Result<Tensor> {
            let (_, seq_len, head_dim) = xs.dims3()?;
            let pos_embed = self
                .encode(seqlen_offset, seq_len, head_dim, xs.device(), xs.dtype())?
                .unsqueeze(0)?;
            let xs = xs.broadcast_add(&pos_embed)?;
            Ok(xs)
        }
    }
}
