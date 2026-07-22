//! Static KV cache used during streaming decode. Lives outside the [`Module`]
//! tree so derives don't need to traverse it.

use burn::prelude::*;

pub type LayerKv<B> = (Tensor<B, 4>, Tensor<B, 4>);

#[derive(Debug)]
pub struct StaticKvCache<B: Backend> {
    /// `Option` lets us `take()` the inner tensors during a step so the
    /// reference count drops to 1, allowing `slice_assign` to recycle the
    /// underlying buffer instead of copying it.
    pub layers: Vec<Option<LayerKv<B>>>,
    pub current_length: usize,
    pub max_length: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
}

impl<B: Backend> StaticKvCache<B> {
    pub fn new(
        num_layers: usize,
        num_kv_heads: usize,
        head_dim: usize,
        batch_size: usize,
        max_length: usize,
        device: &B::Device,
    ) -> Self {
        let layers = (0..num_layers)
            .map(|_| {
                Some((
                    Tensor::zeros([batch_size, num_kv_heads, max_length, head_dim], device),
                    Tensor::zeros([batch_size, num_kv_heads, max_length, head_dim], device),
                ))
            })
            .collect();
        Self {
            layers,
            current_length: 0,
            max_length,
            num_kv_heads,
            head_dim,
        }
    }

    /// Seed from a prefill pass' `(K, V)` tensors `[B, Hkv, S, D]`.
    pub fn fill(&mut self, prefill: Vec<LayerKv<B>>) {
        assert_eq!(prefill.len(), self.layers.len());
        let s = prefill[0].0.dims()[2];
        self.current_length = s;
        for (dst, src) in self.layers.iter_mut().zip(prefill) {
            let (dst_k, dst_v) = dst.take().expect("cache layer present");
            let [b, h, _, d] = dst_k.dims();
            let new_k = dst_k.slice_assign([0..b, 0..h, 0..s, 0..d], src.0);
            let new_v = dst_v.slice_assign([0..b, 0..h, 0..s, 0..d], src.1);
            *dst = Some((new_k, new_v));
        }
    }

    pub fn step(&mut self) -> usize {
        assert!(self.current_length < self.max_length, "KV cache is full");
        let pos = self.current_length;
        self.current_length += 1;
        pos
    }

    pub fn reset(&mut self) {
        self.current_length = 0;
    }

    pub fn layer_mut(&mut self, idx: usize) -> &mut Option<LayerKv<B>> {
        &mut self.layers[idx]
    }
}
