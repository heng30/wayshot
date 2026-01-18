use candle_core::{D, DType, Device, IndexOp, Tensor, shape::Dim};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, TensorUtilsError>;

#[derive(Error, Debug)]
pub enum TensorUtilsError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Candle error: {0}")]
    Candle(#[from] candle_core::Error),
}

pub fn mask_filled(on_true: &Tensor, mask: &Tensor, on_false: f32) -> Result<Tensor> {
    let (mask_seq_len, _) = mask.dims2()?;
    let (_, _, seq_len, _) = on_true.dims4()?;
    assert!(
        mask_seq_len >= seq_len,
        "mask seq_len less than input data seq_len"
    );
    let mask = mask.i((..seq_len, ..seq_len))?;
    let mask = mask.broadcast_as(on_true.shape())?;
    let on_false = Tensor::new(on_false, on_true.device())?.broadcast_as(on_true.shape())?;
    let filled = mask.where_cond(on_true, &on_false)?;
    Ok(filled)
}

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

pub fn repeat_kv(xs: Tensor, n_rep: usize) -> Result<Tensor> {
    if n_rep == 1 {
        Ok(xs)
    } else {
        let (b_sz, n_kv_head, seq_len, head_dim) = xs.dims4()?;
        // Using cat is faster than a broadcast as it avoids going through a potentially
        // strided copy.
        // https://github.com/huggingface/candle/pull/2043
        let kv = Tensor::cat(&vec![&xs; n_rep], 2)?.reshape((
            b_sz,
            n_kv_head * n_rep,
            seq_len,
            head_dim,
        ))?;
        Ok(kv)
    }
}

// Split tensor along given dimension with specified split sizes
// Example: t:(25), splits: [5, 10, 5, 5] dim: 0,
// Returns vec len=4, with tensor dimensions: (5), (10), (5), (5)
pub fn split_tensor<D: Dim>(t: &Tensor, splits: &[usize], dim: D) -> Result<Vec<Tensor>> {
    let dim = dim.to_index(t.shape(), "split")?;
    let mut split_res = Vec::new();
    let mut index = 0;
    for split in splits {
        split_res.push(t.narrow(dim, index, *split)?);
        index += *split;
    }
    Ok(split_res)
}

pub fn get_equal_mask(input_ids: &Tensor, token_ids: u32) -> Result<Tensor> {
    let image_token_id_tensor =
        Tensor::new(vec![token_ids], input_ids.device())?.to_dtype(input_ids.dtype())?;
    let mask = input_ids
        .broadcast_eq(&image_token_id_tensor)?
        .to_dtype(candle_core::DType::U32)?;
    Ok(mask)
}

// Get non-zero element indices from mask matrix, return vec only handles 1D data
fn nonzero_index_vec(mask: &Tensor) -> Result<Vec<u32>> {
    let mut mask = mask.clone();
    if mask.dtype() != DType::U32 {
        mask = mask.to_dtype(DType::U32)?;
    }
    match mask.rank() {
        0 => Err(TensorUtilsError::InvalidInput(format!(
            "input rank must > 0, the input tensor rank: {}",
            mask.rank()
        ))),
        1 => {
            let mask_vector = mask.to_vec1::<u32>()?;
            let indices: Vec<u32> = mask_vector
                .iter()
                .enumerate()
                .filter_map(|(idx, &val)| if val != 0 { Some(idx as u32) } else { None })
                .collect();
            Ok(indices)
        }
        _ => Err(TensorUtilsError::InvalidInput(format!(
            "input rank not support, the input tensor rank: {}",
            mask.rank()
        ))),
    }
}

// Get non-zero element indices from mask matrix
// Get consecutive index intervals based on indices
// Example: if non-zero indices are [0, 3, 4, 5, 8, 9]
// Intervals are: [(0, 1), (3, 6), (8, 10)]
// Index is closed at start, open at end
fn nonzero_slice(mask: &Tensor) -> Result<Vec<(usize, usize)>> {
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

// Replace data in original with data from replace based on non-zero element indices in mask
// original: rank = 3: (bs, seq_len, hidden_dim)
// replace: rank = 2: (seq_len, hidden_dim)
// mask: rank = 2: (bs, seq_len)
// During inference bs=1, for convenience squeeze bs, replace, then unsqueeze
// Replace by row
pub fn masked_scatter_dim0(original: &Tensor, replace: &Tensor, mask: &Tensor) -> Result<Tensor> {
    if original.dim(0)? != 1 || mask.dim(0)? != 1 {
        return Err(TensorUtilsError::InvalidInput(format!(
            "masked_scatter_dim0 original bs: {} or mask bs :{} not equal to 1 ",
            original.dim(0)?,
            mask.dim(0)? != 1
        )));
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

pub fn index_select_2d(t: &Tensor, index: &Tensor) -> Result<Tensor> {
    if t.rank() != 2 && index.rank() != 2 {
        return Err(TensorUtilsError::InvalidInput(
            "t and index rank must be equal to 2".to_string(),
        ));
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

pub fn pad_replicate_last_dim(t: &Tensor, pad: (usize, usize)) -> Result<Tensor> {
    let (pad_l, pad_r) = pad;
    let last_dim = t.dim(D::Minus1)?;

    let mut pad_tensor = t.clone();
    if pad_l > 0 {
        let left = pad_tensor.narrow(D::Minus1, 0, 1)?.contiguous()?;
        let rank = left.rank();
        let mut shape = vec![1usize; rank - 1];
        shape.push(pad_l);
        let left_pad = left.repeat(shape)?;
        pad_tensor = Tensor::cat(&[&left_pad, &pad_tensor], D::Minus1)?;
    }
    if pad_r > 0 {
        let start_i = last_dim - 1;
        let right = pad_tensor.narrow(D::Minus1, start_i, 1)?.contiguous()?;
        let rank = right.rank();
        let mut shape = vec![1usize; rank - 1];
        shape.push(pad_r);
        let right_pad = right.repeat(shape)?;
        pad_tensor = Tensor::cat(&[&pad_tensor, &right_pad], D::Minus1)?;
    }
    Ok(pad_tensor)
}
