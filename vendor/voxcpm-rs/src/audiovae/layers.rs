//! Primitive building blocks of the AudioVAE: causal convs, snake, noise, SR conditioning.

use burn::module::Param;
use burn::nn::conv::{Conv1d, Conv1dConfig, ConvTranspose1d, ConvTranspose1dConfig};
use burn::nn::{Embedding, EmbeddingConfig, PaddingConfig1d};
use burn::prelude::*;
use burn::tensor::{Int, TensorData};

// ---------------------------------------------------------------------------
// Causal Conv1d: left-pads by `padding*2 - output_padding` before a regular conv.
// ---------------------------------------------------------------------------

#[derive(Module, Debug)]
pub struct CausalConv1d<B: Backend> {
    pub conv: Conv1d<B>,
    pub left_pad: usize,
}

impl<B: Backend> CausalConv1d<B> {
    pub fn new(
        channels_in: usize,
        channels_out: usize,
        kernel_size: usize,
        stride: usize,
        dilation: usize,
        groups: usize,
        padding: usize,
        output_padding: usize,
        bias: bool,
        device: &B::Device,
    ) -> Self {
        let conv = Conv1dConfig::new(channels_in, channels_out, kernel_size)
            .with_stride(stride)
            .with_dilation(dilation)
            .with_groups(groups)
            .with_padding(PaddingConfig1d::Valid)
            .with_bias(bias)
            .init(device);
        Self {
            conv,
            left_pad: padding * 2 - output_padding,
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let x = if self.left_pad > 0 {
            x.pad((self.left_pad, 0, 0, 0), 0.0)
        } else {
            x
        };
        self.conv.forward(x)
    }
}

// ---------------------------------------------------------------------------
// Causal Transposed Conv1d: trims `padding*2 - output_padding` rightmost
// samples after a regular transposed conv.
// ---------------------------------------------------------------------------

#[derive(Module, Debug)]
pub struct CausalTransposeConv1d<B: Backend> {
    pub conv: ConvTranspose1d<B>,
    pub right_trim: usize,
}

impl<B: Backend> CausalTransposeConv1d<B> {
    pub fn new(
        channels_in: usize,
        channels_out: usize,
        kernel_size: usize,
        stride: usize,
        padding: usize,
        output_padding: usize,
        groups: usize,
        bias: bool,
        device: &B::Device,
    ) -> Self {
        let conv = ConvTranspose1dConfig::new([channels_in, channels_out], kernel_size)
            .with_stride(stride)
            .with_padding(0)
            .with_padding_out(0)
            .with_groups(groups)
            .with_bias(bias)
            .init(device);
        Self {
            conv,
            right_trim: padding * 2 - output_padding,
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let out = self.conv.forward(x);
        if self.right_trim > 0 {
            let t = out.dims()[2];
            out.narrow(2, 0, t - self.right_trim)
        } else {
            out
        }
    }
}

// ---------------------------------------------------------------------------
// Snake1d: x + (1/(alpha+eps)) * sin(alpha*x)^2
// ---------------------------------------------------------------------------

#[derive(Module, Debug)]
pub struct Snake1d<B: Backend> {
    pub alpha: Param<Tensor<B, 3>>, // [1, C, 1]
}

impl<B: Backend> Snake1d<B> {
    pub fn new(channels: usize, device: &B::Device) -> Self {
        Self {
            alpha: Param::from_tensor(Tensor::ones([1, channels, 1], device)),
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let alpha = self.alpha.val();
        let denom = alpha.clone().add_scalar(1e-9).recip();
        let sin_sq = (alpha * x.clone()).sin().powf_scalar(2.0);
        x + denom * sin_sq
    }
}

// ---------------------------------------------------------------------------
// Residual Unit: Snake → Conv(k=7, dil) → Snake → Conv(k=1)
// ---------------------------------------------------------------------------

#[derive(Module, Debug)]
pub struct CausalResidualUnit<B: Backend> {
    pub snake1: Snake1d<B>,
    pub conv1: CausalConv1d<B>,
    pub snake2: Snake1d<B>,
    pub conv2: CausalConv1d<B>,
}

impl<B: Backend> CausalResidualUnit<B> {
    pub fn new(dim: usize, dilation: usize, groups: usize, device: &B::Device) -> Self {
        let kernel = 7usize;
        let pad = ((kernel - 1) * dilation) / 2;
        Self {
            snake1: Snake1d::new(dim, device),
            conv1: CausalConv1d::new(dim, dim, kernel, 1, dilation, groups, pad, 0, true, device),
            snake2: Snake1d::new(dim, device),
            conv2: CausalConv1d::new(dim, dim, 1, 1, 1, 1, 0, 0, true, device),
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let y = self.snake1.forward(x.clone());
        let y = self.conv1.forward(y);
        let y = self.snake2.forward(y);
        let y = self.conv2.forward(y);
        x + y
    }
}

// ---------------------------------------------------------------------------
// Noise block: x + randn * conv1(x)
// ---------------------------------------------------------------------------

#[derive(Module, Debug)]
pub struct NoiseBlock<B: Backend> {
    pub linear: CausalConv1d<B>,
}

impl<B: Backend> NoiseBlock<B> {
    pub fn new(dim: usize, device: &B::Device) -> Self {
        Self {
            linear: CausalConv1d::new(dim, dim, 1, 1, 1, 1, 0, 0, false, device),
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let [b, _c, t] = x.dims();
        let noise = Tensor::<B, 3>::random(
            [b, 1, t],
            burn::tensor::Distribution::Normal(0.0, 1.0),
            &x.device(),
        )
        .clamp(-3.0, 3.0);
        let h = self.linear.forward(x.clone());
        x + noise * h
    }
}

// ---------------------------------------------------------------------------
// Sample-rate conditioning layer
// ---------------------------------------------------------------------------

#[derive(Module, Debug)]
#[module(skip = cond_type)]
pub struct SampleRateConditionLayer<B: Backend> {
    pub scale_embed: Embedding<B>,
    pub bias_embed: Embedding<B>,
    pub cond_type: String,
}

impl<B: Backend> SampleRateConditionLayer<B> {
    /// Only the `scale_bias` cond type is supported (matches shipped VoxCPM2 weights).
    pub fn new(input_dim: usize, sr_bin_buckets: usize, cond_type: &str, device: &B::Device) -> Self {
        assert!(
            cond_type == "scale_bias" || cond_type == "scale_bias_init",
            "Only scale_bias cond_type is supported, got: {cond_type}"
        );
        Self {
            scale_embed: EmbeddingConfig::new(sr_bin_buckets, input_dim).init(device),
            bias_embed: EmbeddingConfig::new(sr_bin_buckets, input_dim).init(device),
            cond_type: cond_type.to_string(),
        }
    }

    /// `x`: `[B, C, T]`. `sr_cond`: `[B]` (bucket indices).
    pub fn forward(&self, x: Tensor<B, 3>, sr_cond: Tensor<B, 1, Int>) -> Tensor<B, 3> {
        let sr2: Tensor<B, 2, Int> = sr_cond.unsqueeze_dim(1); // [B, 1]
        let scale = self.scale_embed.forward(sr2.clone()); // [B, 1, C]
        let bias = self.bias_embed.forward(sr2); // [B, 1, C]
        let scale = scale.swap_dims(1, 2); // [B, C, 1]
        let bias = bias.swap_dims(1, 2); // [B, C, 1]
        x * scale + bias
    }
}

/// Compute the sample-rate bucket index via `bucketize` on CPU (SR is a tiny
/// scalar tensor in practice).
pub fn sr_bucket<B: Backend>(sr: i32, boundaries: &[i32], device: &B::Device) -> Tensor<B, 1, Int> {
    let idx = boundaries.iter().position(|&b| sr < b).unwrap_or(boundaries.len()) as i32;
    Tensor::<B, 1, Int>::from_data(TensorData::new(vec![idx as i64], [1]), device)
}
