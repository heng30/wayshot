
use candle_core::{D, DType, Device, IndexOp, Tensor};
use candle_transformers::models::deepseek2::SplitOp;

/// Compute default RoPE parameters
pub fn compute_default_rope_parameters(dim: usize, base: f32) -> Vec<f32> {
    let inv_freq: Vec<f32> = (0..dim)
        .step_by(2)
        .map(|i| 1.0_f32 / base.powf(i as f32 / dim as f32))
        .collect();
    inv_freq
}

/// Rotate half of the tensor for RoPE
pub fn rotate_half(x: &Tensor) -> Result<Tensor, crate::Error> {
    let half_dim = x.dim(D::Minus1)? / 2;
    let x1 = x.narrow(D::Minus1, 0, half_dim)?;
    let x2 = x.narrow(D::Minus1, half_dim, half_dim)?;
    let x2 = x2.affine(-1.0, 0.0)?;
    let rotate_x = Tensor::cat(&[&x2, &x1], D::Minus1)?.contiguous()?;
    Ok(rotate_x)
}

/// Apply multimodal rotary position embedding
pub fn apply_multimodel_rotary_pos_emb(
    q: &Tensor,
    k: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
    mrope_section: Vec<usize>,
) -> Result<(Tensor, Tensor), crate::Error> {
    let mrope_section = mrope_section.repeat(2);
    let cos_select: Vec<Tensor> = cos
        .split(&mrope_section, D::Minus1)?
        .iter()
        .enumerate()
        .map(|(i, m)| m.i(i % 3).unwrap())
        .collect();
    let cos = Tensor::cat(&cos_select, D::Minus1)?
        .unsqueeze(1)?
        .contiguous()?;
    let sin_select: Vec<Tensor> = sin
        .split(&mrope_section, D::Minus1)?
        .iter()
        .enumerate()
        .map(|(i, m)| m.i(i % 3).unwrap())
        .collect();
    let sin = Tensor::cat(&sin_select, D::Minus1)?
        .unsqueeze(1)?
        .contiguous()?;
    let q_embed = q
        .broadcast_mul(&cos)?
        .add(&rotate_half(q)?.broadcast_mul(&sin)?)?;
    let k_embed = k
        .broadcast_mul(&cos)?
        .add(&rotate_half(k)?.broadcast_mul(&sin)?)?;
    Ok((q_embed, k_embed))
}

/// Apply rotary position embedding for vision
pub fn apply_rotary_pos_emb_vision(
    q: &Tensor,
    k: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
) -> Result<(Tensor, Tensor), crate::Error> {
    // q, k -> (seq_len, num_heads, head_dim)
    // cos, sin -> (seq_len, head_dim) -> (seq_len, 1, head_dim)
    let cos = cos.unsqueeze(D::Minus2)?;
    let sin = sin.unsqueeze(D::Minus2)?;
    let cos = cos.to_dtype(q.dtype())?;
    let sin = sin.to_dtype(q.dtype())?;
    let q_embed = q
        .broadcast_mul(&cos)?
        .add(&rotate_half(q)?.broadcast_mul(&sin)?)?;
    let k_embed = k
        .broadcast_mul(&cos)?
        .add(&rotate_half(k)?.broadcast_mul(&sin)?)?;
    Ok((q_embed, k_embed))
}

/// Apply rotary position embedding
pub fn apply_rotary_pos_emb(
    q: &Tensor,
    k: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
    tof32: bool,
) -> Result<(Tensor, Tensor), crate::Error> {
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

/// Qwen2.5-VL Text Rotary Embedding for PaddleOCR-VL1.5
#[derive(Debug, Clone)]
pub struct Qwen2_5VLTextRotaryEmbedding {
    inv_freq: Vec<f32>,
}

impl Qwen2_5VLTextRotaryEmbedding {
    pub fn new(dim: usize, theta_base: f32) -> Self {
        let inv_freq = compute_default_rope_parameters(dim, theta_base);
        Self { inv_freq }
    }

    pub fn forward(
        &self,
        position_ids: &Tensor,
        dtype: DType,
        mrope_section: Vec<usize>,
    ) -> Result<(Tensor, Tensor), crate::Error> {
        // position_ids shape: (3, bs, position) -> (3, bs, 1, position)
        let position_ids_expanded = position_ids
            .unsqueeze(D::Minus2)?
            .to_dtype(DType::F32)?
            .contiguous()?;
        // inv_freq Vec<f32> -> Tensor(1, 1, head_dim / 2, 1) -> (3, bs, head_dim / 2, 1)
        let inv_freq_expanded = Tensor::from_vec(
            self.inv_freq.clone(),
            (1, 1, self.inv_freq.len(), 1),
            position_ids.device(),
        )?
        .broadcast_as((3, position_ids.dim(1)?, self.inv_freq.len(), 1))?
        .to_dtype(DType::F32)?
        .contiguous()?;

        // (3, bs, head_dim / 2, 1) matmul (3, bs, 1, position)
        //    -> (3, bs, head_dim / 2, seq_len) -> (3, bs, seq_len, head_dim / 2)
        let freqs = inv_freq_expanded
            .matmul(&position_ids_expanded)?
            .transpose(2, 3)?;
        // (3, bs, seq_len, head_dim / 2) -> (3, bs, seq_len, head_dim)
        let emb = Tensor::cat(&[&freqs, &freqs], D::Minus1)?.contiguous()?;
        let cos = emb.cos()?;
        let sin = emb.sin()?;
        let mrope_section = mrope_section.repeat(2);
        let cos_select: Vec<Tensor> = cos
            .split(&mrope_section, D::Minus1)?
            .iter()
            .enumerate()
            .map(|(i, m)| m.i(i % 3).unwrap())
            .collect();
        // (bs, seq_len, head_dim) -> (bs, 1, seq_len, head_dim)
        let cos = Tensor::cat(&cos_select, D::Minus1)?
            .unsqueeze(1)?
            .contiguous()?;
        let sin_select: Vec<Tensor> = sin
            .split(&mrope_section, D::Minus1)?
            .iter()
            .enumerate()
            .map(|(i, m)| m.i(i % 3).unwrap())
            .collect();
        // (bs, seq_len, head_dim) -> (bs, 1, seq_len, head_dim)
        let sin = Tensor::cat(&sin_select, D::Minus1)?
            .unsqueeze(1)?
            .contiguous()?;
        Ok((cos.to_dtype(dtype)?, sin.to_dtype(dtype)?))
    }
}

/// Qwen2.5 Vision Rotary Embedding for PaddleOCR-VL1.5
#[derive(Debug, Clone)]
pub struct Qwen2_5VisionRotaryEmbedding {
    inv_freq: Vec<f32>,
}

impl Qwen2_5VisionRotaryEmbedding {
    pub fn new(dim: usize, theta_base: Option<f32>) -> Self {
        let theta_base = theta_base.unwrap_or(10000.0_f32);
        let inv_freq = compute_default_rope_parameters(dim, theta_base);
        Self { inv_freq }
    }

    pub fn forward(&self, seqlen: usize, device: &Device) -> Result<Tensor, crate::Error> {
        let seq = Tensor::arange(0.0_f32, seqlen as f32, device)?.reshape((seqlen, 1))?;
        let inv_freq =
            Tensor::from_vec(self.inv_freq.clone(), (1, self.inv_freq.len()), device)?;
        let freqs = seq.matmul(&inv_freq)?;
        Ok(freqs)
    }
}

/// Get XD cos/sin for cross-dimensional RoPE (currently unused, reserved for future use)
#[allow(dead_code)]
pub fn get_xd_cos_sin(
    cos: &Tensor,
    sin: &Tensor,
    position_ids: &Tensor,
    xdrope_section: Vec<usize>,
) -> Result<(Tensor, Tensor), crate::Error> {
    let x_dim = xdrope_section.len();
    // position_ids: (bs, 4, seq_len)
    let mut cos_vec = vec![];
    let mut sin_vec = vec![];
    let bs = position_ids.dim(0)?;
    for i in 0..bs {
        let pos_i = position_ids.i(i)?;
        let cos_i = index_select_2d(cos, &pos_i)?;
        let sin_i = index_select_2d(sin, &pos_i)?;
        cos_vec.push(cos_i);
        sin_vec.push(sin_i);
    }
    // (bs, 4, seq_len, dim) -> (bs, seq_len, 4, dim)
    let cos = Tensor::stack(&cos_vec, 0)?
        .permute((0, 2, 1, 3))?
        .contiguous()?;
    let sin = Tensor::stack(&sin_vec, 0)?
        .permute((0, 2, 1, 3))?
        .contiguous()?;
    let xdrope_section: Vec<usize> = xdrope_section.iter().map(|&i| i * 2).collect();
    // Use Tensor::split instead of split_tensor
    let cos_splits: Vec<Tensor> = cos.split(&xdrope_section, D::Minus1)?
        .into_iter()
        .collect();
    let cos_select: Vec<Tensor> = cos_splits
        .iter()
        .enumerate()
        .map(|(i, m)| m.i((.., .., i % x_dim)).unwrap())
        .collect();
    let sin_splits: Vec<Tensor> = sin.split(&xdrope_section, D::Minus1)?
        .into_iter()
        .collect();
    let sin_select: Vec<Tensor> = sin_splits
        .iter()
        .enumerate()
        .map(|(i, m)| m.i((.., .., i % x_dim)).unwrap())
        .collect();

    let cos = Tensor::cat(&cos_select, D::Minus1)?;
    let sin = Tensor::cat(&sin_select, D::Minus1)?;
    Ok((cos, sin))
}

fn index_select_2d(t: &Tensor, index: &Tensor) -> Result<Tensor, crate::Error> {
    if t.rank() != 2 && index.rank() != 2 {
        return Err(crate::Error::InvalidIndexSelectRank);
    }
    let mut res_vec = Vec::new();
    let index_dim0 = index.dim(0)?;
    for i in 0..index_dim0 {
        let index_i = index.i(i)?;
        let rel_i = t.index_select(&index_i, 0)?;
        res_vec.push(rel_i);
    }
    let res = Tensor::stack(&res_vec, 0)?;
    Ok(res)
}