use candle_core::{D, DType, Device, IndexOp, Tensor};

use crate::Error;

/// Prepare causal attention mask
pub fn prepare_causal_attention_mask(
    b_size: usize,
    tgt_len: usize,
    seqlen_offset: usize,
    device: &Device,
) -> Result<Tensor, Error> {
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

/// Repeat KV tensor for GQA
pub fn repeat_kv(xs: Tensor, n_rep: usize) -> Result<Tensor, Error> {
    if n_rep == 1 {
        Ok(xs)
    } else {
        let (b_sz, n_kv_head, seq_len, head_dim) = xs.dims4()?;
        let kv = Tensor::cat(&vec![&xs; n_rep], 2)?.reshape((
            b_sz,
            n_kv_head * n_rep,
            seq_len,
            head_dim,
        ))?;
        Ok(kv)
    }
}

/// Get indices of non-zero elements in 1D tensor
pub fn nonzero_index_vec(mask: &Tensor) -> Result<Vec<u32>, Error> {
    let mut mask = mask.clone();
    if mask.dtype() != DType::U32 {
        mask = mask.to_dtype(DType::U32)?;
    }
    if mask.rank() != 1 {
        return Err(Error::InvalidRank { expected: 1, got: mask.rank() });
    }
    let mask_vector = mask.to_vec1::<u32>()?;
    let indices: Vec<u32> = mask_vector
        .iter()
        .enumerate()
        .filter_map(|(idx, &val)| if val != 0 { Some(idx as u32) } else { None })
        .collect();
    Ok(indices)
}

/// Get tensor of indices of non-zero elements
pub fn nonzero_index(mask: &Tensor) -> Result<Tensor, Error> {
    let index_vec = nonzero_index_vec(mask)?;
    Tensor::from_slice(&index_vec, index_vec.len(), mask.device())?;
    Ok(Tensor::from_slice(&index_vec, index_vec.len(), mask.device())?)
}

/// Get indices of zero elements in 1D tensor
pub fn zero_index_vec(mask: &Tensor) -> Result<Vec<u32>, Error> {
    let mut mask = mask.clone();
    if mask.dtype() != DType::U32 {
        mask = mask.to_dtype(DType::U32)?;
    }
    if mask.rank() != 1 {
        return Err(Error::InvalidRank { expected: 1, got: mask.rank() });
    }
    let mask_vector = mask.to_vec1::<u32>()?;
    let indices: Vec<u32> = mask_vector
        .iter()
        .enumerate()
        .filter_map(|(idx, &val)| if val == 0 { Some(idx as u32) } else { None })
        .collect();
    Ok(indices)
}

/// Get tensor of indices of zero elements
pub fn zero_index(mask: &Tensor) -> Result<Tensor, Error> {
    let index_vec = zero_index_vec(mask)?;
    Ok(Tensor::from_slice(&index_vec, index_vec.len(), mask.device())?)
}

/// Get non-zero slice ranges (start, end) from mask
pub fn nonzero_slice(mask: &Tensor) -> Result<Vec<(usize, usize)>, Error> {
    let mut index_vec = nonzero_index_vec(mask)?;
    match index_vec.len() {
        0 => Ok(vec![]),
        1 => Ok(vec![(index_vec[0] as usize, (index_vec[0] + 1) as usize)]),
        _ => {
            let mut vec_slice = vec![];
            let mut start = index_vec.remove(0);
            let mut last = start;

            for i in index_vec {
                if i == (last + 1) {
                    last = i;
                    continue;
                } else {
                    vec_slice.push((start as usize, (last + 1) as usize));
                    start = i;
                    last = i;
                }
            }
            vec_slice.push((start as usize, (last + 1) as usize));
            Ok(vec_slice)
        }
    }
}

/// Masked scatter along dimension 0
pub fn masked_scatter_dim0(original: &Tensor, replace: &Tensor, mask: &Tensor) -> Result<Tensor, Error> {
    if original.dim(0)? != 1 || mask.dim(0)? != 1 {
        return Err(Error::InvalidBatchSize {
            original_bs: original.dim(0)?,
            mask_bs: mask.dim(0)?,
        });
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

/// Get mask where tensor equals a specific token id
pub fn get_equal_mask(input_ids: &Tensor, token_ids: u32) -> Result<Tensor, Error> {
    let image_token_id_tensor =
        Tensor::new(vec![token_ids], input_ids.device())?.to_dtype(input_ids.dtype())?;
    let mask = input_ids
        .broadcast_eq(&image_token_id_tensor)?
        .to_dtype(DType::U32)?;
    Ok(mask)
}

/// Get indices where tensor equals a specific token id
pub fn get_eq_indices(input_ids: &Tensor, token_id: u32) -> Result<Tensor, Error> {
    let mask = get_equal_mask(input_ids, token_id)?;
    nonzero_index(&mask)
}

/// Get indices after vision token positions
pub fn get_vision_next_indices(input_ids: &Tensor, token_id: u32) -> Result<Tensor, Error> {
    let indices = get_eq_indices(input_ids, token_id)?;
    let indices = indices.broadcast_add(&Tensor::new(vec![1u32], input_ids.device())?)?;
    Ok(indices)
}