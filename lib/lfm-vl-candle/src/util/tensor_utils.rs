//! Tensor manipulation utilities.

use crate::error::{LfmError, Result};
use candle_core::{D, DType, Device, IndexOp, Tensor};

/// Build a causal attention mask.
pub fn prepare_causal_attention_mask(
    b_size: usize,
    tgt_len: usize,
    seqlen_offset: usize,
    device: &Device,
) -> Result<Tensor> {
    let arange = Tensor::arange(0u32, tgt_len as u32, device)?;
    let arange = arange.unsqueeze(1)?.broadcast_as((tgt_len, tgt_len))?;
    let upper_triangle = arange.t()?.gt(&arange)?;
    let mask = upper_triangle.where_cond(
        &Tensor::new(f32::NEG_INFINITY, device)?.broadcast_as(arange.shape())?,
        &Tensor::new(0f32, device)?.broadcast_as(arange.shape())?,
    )?;
    let mask = if seqlen_offset > 0 {
        let mask0 = Tensor::zeros((tgt_len, seqlen_offset), DType::F32, device)?;
        Tensor::cat(&[&mask0, &mask], D::Minus1)?
    } else {
        mask
    };
    let mask = mask
        .expand((b_size, 1, tgt_len, tgt_len + seqlen_offset))?
        .to_dtype(DType::F32)?;
    Ok(mask)
}

/// Prepare pixel-attention mask for vision encoder.
///
/// Creates a 4D mask `(B, 1, Q, K)` where padded key positions are set to
/// `-inf`.  Padded query positions can still attend to real keys (so the
/// softmax produces valid weights), but their outputs are zeroed out by
/// the caller after the attention computation.
pub fn prepare_mask(mask: &Tensor) -> Result<Tensor> {
    let seq_len = mask.dim(1)?;
    let mask = mask.unsqueeze(1)?.unsqueeze(1)?;
    let mask = mask.repeat((1, 1, seq_len, 1))?;
    let on_true = mask.zeros_like()?.to_dtype(DType::F32)?;
    let on_false = Tensor::new(f32::NEG_INFINITY, mask.device())?.broadcast_as(mask.shape())?;
    let mask = mask.where_cond(&on_true, &on_false)?;
    Ok(mask)
}

/// Repeat KV heads for GQA.
///
/// Uses `expand` (broadcast) + reshape rather than `Tensor::cat` to
/// avoid allocating and copying data for the repeated heads.
pub fn repeat_kv(xs: &Tensor, n_rep: usize) -> Result<Tensor> {
    if n_rep == 1 {
        Ok(xs.clone())
    } else {
        let (b_sz, n_kv_head, seq_len, head_dim) = xs.dims4()?;
        // expand + reshape produces a broadcasted view (zero-copy).
        // However, matmul requires contiguous input, so we must
        // make it contiguous — but this is still cheaper than cat
        // which both allocates AND copies the original data n_rep times.
        let xs = xs
            .unsqueeze(2)?
            .expand((b_sz, n_kv_head, n_rep, seq_len, head_dim))?
            .reshape((b_sz, n_kv_head * n_rep, seq_len, head_dim))?
            .contiguous()?;
        Ok(xs)
    }
}

/// Depthwise conv1d (group = dim, stride = 1).
pub fn conv1d_depthwise(input: &Tensor, weight: &Tensor, bias: Option<&Tensor>) -> Result<Tensor> {
    let len_in = input.dim(2)?;
    let weight = weight.squeeze(1)?.to_dtype(input.dtype())?;
    let kernel_size = weight.dim(1)?;
    let len_out = len_in - kernel_size + 1;
    let mut out = input
        .narrow(2, 0, len_out)?
        .broadcast_mul(&weight.narrow(1, 0, 1)?.unsqueeze(0)?)?;
    for k in 1..kernel_size {
        out = (out
            + input
                .narrow(2, k, len_out)?
                .broadcast_mul(&weight.narrow(1, k, 1)?.unsqueeze(0)?)?)?;
    }
    match bias {
        None => Ok(out),
        Some(bias) => {
            let b = bias.dims1()?;
            let bias = bias.reshape((1, b, 1))?.to_dtype(input.dtype())?;
            Ok(out.broadcast_add(&bias)?)
        }
    }
}

/// Get a mask where elements equal `token_ids` are 1.
pub fn get_equal_mask(input_ids: &Tensor, token_ids: u32) -> Result<Tensor> {
    let token_tensor =
        Tensor::new(vec![token_ids], input_ids.device())?.to_dtype(input_ids.dtype())?;
    let mask = input_ids
        .broadcast_eq(&token_tensor)?
        .to_dtype(DType::U32)?;
    Ok(mask)
}

/// Scatter-replace rows in `original` with rows from `replace` at positions
/// where `mask` is non-zero.
pub fn masked_scatter_dim0(original: &Tensor, replace: &Tensor, mask: &Tensor) -> Result<Tensor> {
    if original.dim(0)? != 1 || mask.dim(0)? != 1 {
        return Err(LfmError::ImageProcessing("masked_scatter_dim0: original and mask batch dim must be 1".into()));
    }
    let mut original = original.squeeze(0)?;
    let mask = mask.squeeze(0)?;
    let slices = nonzero_slice(&mask)?;
    let mut sub_start = 0usize;
    let mut sub_end;
    for (start, end) in slices {
        sub_end = sub_start + (end - start);
        let sub_replace = replace.i((sub_start..sub_end, ..))?;
        original = original.slice_assign(&[(start..end), (0..original.dim(1)?)], &sub_replace)?;
        sub_start = sub_end;
    }
    original = original.unsqueeze(0)?;
    Ok(original)
}

/// Find contiguous non-zero intervals in a 1-D mask.
fn nonzero_slice(mask: &Tensor) -> Result<Vec<(usize, usize)>> {
    let mask_vec = mask.to_vec1::<u32>()?;
    let indices: Vec<usize> = mask_vec
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| if v != 0 { Some(i) } else { None })
        .collect();
    match indices.len() {
        0 => Ok(vec![]),
        1 => Ok(vec![(indices[0], indices[0] + 1)]),
        _ => {
            let mut slices = vec![];
            let mut start = indices[0];
            let mut last = start;
            for &i in &indices[1..] {
                if i == last + 1 {
                    last = i;
                } else {
                    slices.push((start, last + 1));
                    start = i;
                    last = i;
                }
            }
            slices.push((start, last + 1));
            Ok(slices)
        }
    }
}
