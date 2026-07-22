// Probe pure bf16 elementwise ops on the GPU backend.

use burn::prelude::*;
use burn::tensor::{TensorData, activation::sigmoid};

#[cfg(feature = "vulkan")]
type Gpu = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
type Gpu = burn::backend::Wgpu<half::bf16, i32>;

fn rd(t: Tensor<Gpu, 1>) -> Vec<f32> {
    t.into_data().convert::<f32>().into_vec().unwrap()
}

fn bf(v: f32, n: usize) -> TensorData {
    TensorData::new(vec![half::bf16::from_f32(v); n], [n])
}

fn main() {
    let device = Default::default();
    let n = 8;

    let a: Tensor<Gpu, 1> = Tensor::from_data(bf(10.0, n), &device);
    let b: Tensor<Gpu, 1> = Tensor::from_data(bf(2.0, n), &device);
    let c: Tensor<Gpu, 1> = Tensor::from_data(bf(0.5, n), &device);
    let d: Tensor<Gpu, 1> = Tensor::from_data(bf(1.0, n), &device);
    let z: Tensor<Gpu, 1> = Tensor::from_data(bf(0.0, n), &device);

    eprintln!("Pure bf16 binary ops:");
    eprintln!("  10.0 + 2.0 = {:?}  (expect 12.0)", &rd(a.clone() + b.clone())[..2]);
    eprintln!("  10.0 - 2.0 = {:?}  (expect 8.0)", &rd(a.clone() - b.clone())[..2]);
    eprintln!("  10.0 * 2.0 = {:?}  (expect 20.0)", &rd(a.clone() * b.clone())[..2]);
    eprintln!("  10.0 / 2.0 = {:?}  (expect 5.0)", &rd(a.clone() / b.clone())[..2]);
    eprintln!("  1.0 * 0.5  = {:?}  (expect 0.5)", &rd(d.clone() * c.clone())[..2]);
    eprintln!("  1.0 * 1.0  = {:?}  (expect 1.0)", &rd(d.clone() * d.clone())[..2]);
    eprintln!("  0.5 * 0.5  = {:?}  (expect 0.25)", &rd(c.clone() * c.clone())[..2]);
    eprintln!("  10.0 * 0.0 = {:?}  (expect 0.0)", &rd(a.clone() * z)[..2]);

    eprintln!("\nScalar ops:");
    eprintln!("  10.0 + scalar(2.0) = {:?}", &rd(a.clone().add_scalar(2.0))[..2]);
    eprintln!("  10.0 * scalar(2.0) = {:?}", &rd(a.clone().mul_scalar(2.0))[..2]);

    eprintln!("\nUnary ops on bf16:");
    eprintln!("  sigmoid(10.0)        = {:?}  (expect ~1.0)", &rd(sigmoid(a.clone()))[..2]);
    eprintln!("  sigmoid(0.0)         = {:?}  (expect 0.5)", &rd(sigmoid(Tensor::<Gpu, 1>::from_data(bf(0.0, n), &device)))[..2]);
    eprintln!("  sqrt(4.0)            = {:?}  (expect 2.0)", &rd(Tensor::<Gpu, 1>::from_data(bf(4.0, n), &device).sqrt())[..2]);
    eprintln!("  exp(0.0)             = {:?}  (expect 1.0)", &rd(Tensor::<Gpu, 1>::from_data(bf(0.0, n), &device).exp())[..2]);
    eprintln!("  exp(1.0)             = {:?}  (expect 2.718)", &rd(Tensor::<Gpu, 1>::from_data(bf(1.0, n), &device).exp())[..2]);

    eprintln!("\nbf16 fan-out: a*a*a*a vs (a*a)*(a*a):");
    let aa = a.clone() * a.clone();
    eprintln!("  10*10        = {:?}  (expect 100)", &rd(aa.clone())[..2]);
    let aaaa1 = a.clone() * a.clone() * a.clone() * a.clone();
    eprintln!("  10*10*10*10  = {:?}  (expect 10000)", &rd(aaaa1)[..2]);

    eprintln!("\nDifferent shapes: 1024 elements:");
    let big_a: Tensor<Gpu, 1> = Tensor::from_data(bf(10.0, 1024), &device);
    let big_b: Tensor<Gpu, 1> = Tensor::from_data(bf(2.0, 1024), &device);
    let big_p = big_a * big_b;
    let v = rd(big_p);
    eprintln!("  10.0 * 2.0 [1024] = {:?}  (expect 20.0)", &v[..4]);

    eprintln!("\n2D shape:");
    let m_a: Tensor<Gpu, 2> = Tensor::from_data(TensorData::new(vec![half::bf16::from_f32(10.0); 32], [4, 8]), &device);
    let m_b: Tensor<Gpu, 2> = Tensor::from_data(TensorData::new(vec![half::bf16::from_f32(2.0); 32], [4, 8]), &device);
    let m_p = m_a * m_b;
    let v: Vec<f32> = m_p.into_data().convert::<f32>().into_vec().unwrap();
    eprintln!("  10*2 [4,8] = {:?}  (expect 20)", &v[..4]);

    eprintln!("\nDirect from f32 data via from_data (this is what voxcpm-rs uses everywhere):");
    let weight_like: Tensor<Gpu, 1> = Tensor::from_data(TensorData::new(vec![10.0f32; n], [n]), &device);
    eprintln!("  from_data(f32 vec) dtype: {:?}", weight_like.dtype());
    let act_like: Tensor<Gpu, 1> = Tensor::from_data(TensorData::new(vec![2.0f32; n], [n]), &device);
    eprintln!("  from_data(f32 vec) dtype: {:?}", act_like.dtype());
    let p = weight_like * act_like;
    eprintln!("  product dtype: {:?}", p.dtype());
    eprintln!("  product head: {:?}", &rd(p)[..2]);
}
