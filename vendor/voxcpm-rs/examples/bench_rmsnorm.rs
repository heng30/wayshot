//! Microbenchmark: how much of our wall-clock is the unfused RMSNorm chain
//! actually costing us? Used to decide whether to invest in a fused cubecl
//! kernel for it.

use burn::module::Param;
use burn::prelude::*;

#[cfg(feature = "wgpu")]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

struct Norm<BB: Backend> {
    w: Param<Tensor<BB, 1>>,
    eps: f64,
}

impl<BB: Backend> Norm<BB> {
    fn new(d: usize, device: &BB::Device) -> Self {
        Self {
            w: Param::from_tensor(Tensor::ones([d], device)),
            eps: 1e-5,
        }
    }
    fn forward(&self, x: Tensor<BB, 3>) -> Tensor<BB, 3> {
        let v = x.clone().powf_scalar(2.0).mean_dim(2);
        let x = x * v.add_scalar(self.eps).powf_scalar(-0.5);
        x * self.w.val().unsqueeze()
    }
}

fn sync<BB: Backend>(t: &Tensor<BB, 3>) {
    // Force GPU→CPU sync by reading one scalar.
    let _ = t.clone().slice([0..1, 0..1, 0..1]).into_data();
}

fn main() {
    let device = Default::default();
    // Representative shape for a mid-decode tensor in one of the LMs / DiT.
    // hidden_size in voxcpm2 ≈ 896 for the LMs, 768 for DiT decoder.
    let cases: &[(usize, usize, usize, &str)] = &[
        (1, 32, 896, "lm-step (B=1, T=32, H=896)"),
        (1, 16, 768, "dit-mid (B=1, seq=16, H=768)"),
        (2, 16, 768, "dit-cfg (B=2, seq=16, H=768)"),
    ];

    for &(b, t, h, label) in cases {
        let norm = Norm::<B>::new(h, &device);
        let x = Tensor::<B, 3>::random([b, t, h], burn::tensor::Distribution::Normal(0.0, 1.0), &device);

        // warmup
        for _ in 0..30 {
            let y = norm.forward(x.clone());
            sync(&y);
        }
        let n = 1000;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            let y = norm.forward(x.clone());
            sync(&y);
        }
        let elapsed = t0.elapsed();
        println!(
            "{label}: {n} calls in {elapsed:.2?} → {:.3} ms/call",
            elapsed.as_secs_f64() * 1000.0 / n as f64
        );
    }
}
