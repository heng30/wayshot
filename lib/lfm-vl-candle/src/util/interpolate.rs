//! Bilinear interpolation for positional embedding resizing.

use crate::error::{LfmError, Result};
use candle_core::{DType, Tensor};

fn compute_scale(input_size: usize, output_size: usize, align_corners: bool) -> f32 {
    if align_corners && output_size > 1 {
        (input_size - 1) as f32 / (output_size - 1) as f32
    } else {
        input_size as f32 / output_size as f32
    }
}

fn compute_1d_coords(
    input_size: usize,
    output_size: usize,
    align_corner: Option<bool>,
) -> Result<Vec<f32>> {
    if input_size == 0 || output_size == 0 {
        return Err(LfmError::ImageProcessing("input_size and output_size must be > 0".into()));
    }
    if input_size == 1 {
        return Ok(vec![0f32; output_size]);
    }
    let align_corners = align_corner.unwrap_or(false);
    let scale = compute_scale(input_size, output_size, align_corners);
    if align_corners {
        Ok((0..output_size).map(|i| i as f32 * scale).collect())
    } else {
        Ok((0..output_size)
            .map(|i| {
                let coord = (i as f32 + 0.5) * scale - 0.5;
                coord.clamp(0.0, (input_size - 1) as f32)
            })
            .collect())
    }
}

/// Bilinear interpolation for 4-D tensors `[B, C, H, W]`.
pub fn interpolate_bilinear(
    input: &Tensor,
    target_size: (usize, usize),
    align_corner: Option<bool>,
    antialias: Option<bool>,
) -> Result<Tensor> {
    if input.rank() != 4 {
        return Err(LfmError::ImageProcessing("Input rank must be 4 [b, c, h, w]".into()));
    }
    let (_, _, input_height, input_width) = input.dims4()?;
    let (target_height, target_width) = target_size;
    if input_height == target_height && input_width == target_width {
        return Ok(input.clone());
    }
    let output = if antialias.unwrap_or(false)
        && (target_height < input_height || target_width < input_width)
    {
        interpolate_bilinear_antialias(input, target_size)?
    } else {
        interpolate_bilinear_standard(input, target_size, align_corner)?
    };
    let output = output.to_dtype(input.dtype())?.to_device(input.device())?;
    Ok(output)
}

fn interpolate_bilinear_standard(
    input: &Tensor,
    target_size: (usize, usize),
    align_corner: Option<bool>,
) -> Result<Tensor> {
    let (bs, channels, input_height, input_width) = input.dims4()?;
    let (target_height, target_width) = target_size;
    let coords_h = compute_1d_coords(input_height, target_height, align_corner)?;
    let coords_w = compute_1d_coords(input_width, target_width, align_corner)?;
    let dim0 = bs * channels;
    let input_3dim = input.reshape((dim0, input_height, input_width))?;
    let input_data = input_3dim.to_dtype(DType::F32)?.to_vec3::<f32>()?;
    let mut output_data = vec![vec![vec![0.0f32; target_width]; target_height]; dim0];
    for c in 0..dim0 {
        for (i, &coord_h) in coords_h.iter().enumerate() {
            let coord_h = coord_h.clamp(0.0, (input_height - 1) as f32);
            let y0 = coord_h.floor() as usize;
            let y1 = (y0 + 1).min(input_height - 1);
            let dy = coord_h - y0 as f32;
            for (j, &coord_w) in coords_w.iter().enumerate() {
                let coord_w = coord_w.clamp(0.0, (input_width - 1) as f32);
                let x0 = coord_w.floor() as usize;
                let x1 = (x0 + 1).min(input_width - 1);
                let dx = coord_w - x0 as f32;
                let q00 = input_data[c][y0][x0];
                let q01 = input_data[c][y0][x1];
                let q10 = input_data[c][y1][x0];
                let q11 = input_data[c][y1][x1];
                output_data[c][i][j] = q00 * (1.0 - dx) * (1.0 - dy)
                    + q01 * dx * (1.0 - dy)
                    + q10 * (1.0 - dx) * dy
                    + q11 * dx * dy;
            }
        }
    }
    let output = Tensor::new(output_data, input.device())?
        .reshape((bs, channels, target_height, target_width))?
        .to_dtype(input.dtype())?
        .contiguous()?;
    Ok(output)
}

fn antialias_filter(x: f32) -> f32 {
    let x = x.abs();
    if x < 1.0 { 1.0 - x } else { 0.0 }
}

fn interpolate_bilinear_antialias(
    input: &Tensor,
    target_size: (usize, usize),
) -> Result<Tensor> {
    let (bs, channels, input_height, input_width) = input.dims4()?;
    let (target_height, target_width) = target_size;
    let scale_h = input_height as f32 / target_height as f32;
    let scale_w = input_width as f32 / target_width as f32;
    let dim0 = bs * channels;
    let input_3dim = input.reshape((dim0, input_height, input_width))?;
    let input_data = input_3dim.to_dtype(DType::F32)?.to_vec3::<f32>()?;
    let mut output_data = vec![vec![vec![0.0f32; target_width]; target_height]; dim0];
    let support_size = scale_h.max(scale_w);
    for c in 0..dim0 {
        for out_y in 0..target_height {
            let center_y = (out_y as f32 + 0.5) * scale_h - 0.5;
            let start_y = (center_y - support_size).max(0.0) as usize;
            let end_y = (center_y + support_size).min(input_height as f32 - 1.0) as usize;
            for out_x in 0..target_width {
                let center_x = (out_x as f32 + 0.5) * scale_w - 0.5;
                let start_x = (center_x - support_size).max(0.0) as usize;
                let end_x = (center_x + support_size).min(input_width as f32 - 1.0) as usize;
                let mut total_weight = 0.0;
                let mut weighted_sum = 0.0;
                for src_y in start_y..=end_y {
                    for src_x in start_x..=end_x {
                        let dist_x = (src_x as f32 - center_x).abs();
                        let dist_y = (src_y as f32 - center_y).abs();
                        let weight_x = antialias_filter(dist_x / scale_w);
                        let weight_y = antialias_filter(dist_y / scale_h);
                        let weight = weight_x * weight_y;
                        weighted_sum += input_data[c][src_y][src_x] * weight;
                        total_weight += weight;
                    }
                }
                output_data[c][out_y][out_x] = if total_weight > 0.0 {
                    weighted_sum / total_weight
                } else {
                    let y = center_y.round().clamp(0.0, (input_height - 1) as f32) as usize;
                    let x = center_x.round().clamp(0.0, (input_width - 1) as f32) as usize;
                    input_data[c][y][x]
                };
            }
        }
    }
    let output = Tensor::new(output_data, input.device())?
        .reshape((bs, channels, target_height, target_width))?
        .to_dtype(input.dtype())?
        .contiguous()?;
    Ok(output)
}
