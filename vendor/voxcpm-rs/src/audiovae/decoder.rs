//! Causal decoder used in AudioVAE v2.

use crate::audiovae::layers::{
    sr_bucket, CausalConv1d, CausalResidualUnit, CausalTransposeConv1d, NoiseBlock,
    SampleRateConditionLayer, Snake1d,
};
use burn::prelude::*;
use burn::tensor::activation::tanh;

#[derive(Module, Debug)]
pub struct CausalDecoderBlock<B: Backend> {
    pub snake: Snake1d<B>,
    pub up: CausalTransposeConv1d<B>,
    pub noise: Option<NoiseBlock<B>>,
    pub res1: CausalResidualUnit<B>,
    pub res2: CausalResidualUnit<B>,
    pub res3: CausalResidualUnit<B>,
    pub input_channels: usize,
}

impl<B: Backend> CausalDecoderBlock<B> {
    pub fn new(
        input_dim: usize,
        output_dim: usize,
        stride: usize,
        groups: usize,
        use_noise_block: bool,
        device: &B::Device,
    ) -> Self {
        let kernel = 2 * stride;
        let pad = (stride as f64 / 2.0).ceil() as usize;
        let out_pad = stride % 2;
        Self {
            snake: Snake1d::new(input_dim, device),
            up: CausalTransposeConv1d::new(input_dim, output_dim, kernel, stride, pad, out_pad, 1, true, device),
            noise: use_noise_block.then(|| NoiseBlock::new(output_dim, device)),
            res1: CausalResidualUnit::new(output_dim, 1, groups, device),
            res2: CausalResidualUnit::new(output_dim, 3, groups, device),
            res3: CausalResidualUnit::new(output_dim, 9, groups, device),
            input_channels: input_dim,
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let x = self.snake.forward(x);
        let x = self.up.forward(x);
        let x = match &self.noise {
            Some(n) => n.forward(x),
            None => x,
        };
        let x = self.res1.forward(x);
        let x = self.res2.forward(x);
        self.res3.forward(x)
    }
}

/// "First conv" path: in depthwise mode it is `Conv(in, in, k=7, groups=in)` followed by
/// a pointwise `Conv(in, channels, k=1)`; otherwise a single `Conv(in, channels, k=7)`.
#[derive(Module, Debug)]
pub struct DecoderFirstConv<B: Backend> {
    pub dw: Option<CausalConv1d<B>>,
    pub pw: CausalConv1d<B>,
}

impl<B: Backend> DecoderFirstConv<B> {
    pub fn new(input_channel: usize, channels: usize, depthwise: bool, device: &B::Device) -> Self {
        if depthwise {
            Self {
                dw: Some(CausalConv1d::new(
                    input_channel, input_channel, 7, 1, 1, input_channel, 3, 0, true, device,
                )),
                pw: CausalConv1d::new(input_channel, channels, 1, 1, 1, 1, 0, 0, true, device),
            }
        } else {
            Self {
                dw: None,
                pw: CausalConv1d::new(input_channel, channels, 7, 1, 1, 1, 3, 0, true, device),
            }
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let x = match &self.dw {
            Some(dw) => dw.forward(x),
            None => x,
        };
        self.pw.forward(x)
    }
}

#[derive(Module, Debug)]
#[module(skip = sr_bin_boundaries)]
pub struct CausalDecoder<B: Backend> {
    pub first: DecoderFirstConv<B>,
    pub blocks: Vec<CausalDecoderBlock<B>>,
    pub snake_out: Snake1d<B>,
    pub last: CausalConv1d<B>,
    pub sr_cond_layers: Vec<Option<SampleRateConditionLayer<B>>>,
    pub sr_bin_boundaries: Option<Vec<i32>>,
}

impl<B: Backend> CausalDecoder<B> {
    pub fn new(
        input_channel: usize,
        channels: usize,
        rates: &[usize],
        depthwise: bool,
        use_noise_block: bool,
        sr_bin_boundaries: Option<Vec<i32>>,
        cond_type: &str,
        device: &B::Device,
    ) -> Self {
        let first = DecoderFirstConv::new(input_channel, channels, depthwise, device);

        let mut blocks = Vec::with_capacity(rates.len());
        let mut out_dim = channels;
        for (i, &stride) in rates.iter().enumerate() {
            let in_dim = channels / 2usize.pow(i as u32);
            out_dim = channels / 2usize.pow((i + 1) as u32);
            let groups = if depthwise { out_dim } else { 1 };
            blocks.push(CausalDecoderBlock::new(in_dim, out_dim, stride, groups, use_noise_block, device));
        }

        let snake_out = Snake1d::new(out_dim, device);
        let last = CausalConv1d::new(out_dim, 1, 7, 1, 1, 1, 3, 0, true, device);

        // One SR-cond layer per decoder block (if enabled), else empty.
        let sr_cond_layers = match &sr_bin_boundaries {
            Some(b) => {
                let buckets = b.len() + 1;
                blocks
                    .iter()
                    .map(|blk| Some(SampleRateConditionLayer::new(blk.input_channels, buckets, cond_type, device)))
                    .collect()
            }
            None => vec![None; blocks.len()],
        };

        Self {
            first,
            blocks,
            snake_out,
            last,
            sr_cond_layers,
            sr_bin_boundaries,
        }
    }

    pub fn forward(&self, x: Tensor<B, 3>, target_sr_hz: i32) -> Tensor<B, 3> {
        let device = x.device();
        let sr_idx = self
            .sr_bin_boundaries
            .as_ref()
            .map(|b| sr_bucket::<B>(target_sr_hz, b, &device));

        let mut h = self.first.forward(x);
        for (blk, sr_layer) in self.blocks.iter().zip(self.sr_cond_layers.iter()) {
            if let (Some(sl), Some(idx)) = (sr_layer, sr_idx.as_ref()) {
                h = sl.forward(h, idx.clone());
            }
            h = blk.forward(h);
        }
        let h = self.snake_out.forward(h);
        let h = self.last.forward(h);
        tanh(h)
    }
}
