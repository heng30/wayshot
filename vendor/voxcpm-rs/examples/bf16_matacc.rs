use burn::prelude::*;
use burn::tensor::TensorData;

#[cfg(feature = "vulkan")]
type Gpu = burn::backend::Vulkan<half::bf16, i32>;

fn main() {
    let device = Default::default();
    // Test bf16 accumulation precision: K=512, A = mostly 1.0 with small +0.001 perturbations
    // CPU-precise expected: 512.0 + small adjustments. bf16 with bf16-accumulator would lose the small parts.
    let m=8; let k=512; let kn=8;
    let aa: Vec<half::bf16> = (0..m*k).map(|i| half::bf16::from_f32(1.0 + (i % 7) as f32 * 0.001)).collect();
    let bb: Vec<half::bf16> = vec![half::bf16::from_f32(1.0); k*kn];
    let A: Tensor<Gpu,2> = Tensor::from_data(TensorData::new(aa.clone(), [m,k]), &device);
    let B: Tensor<Gpu,2> = Tensor::from_data(TensorData::new(bb, [k,kn]), &device);
    let C = A.matmul(B);
    let cv: Vec<f32> = C.into_data().convert::<f32>().into_vec().unwrap();
    // CPU expected: for each row i, output value = sum over k of (1.0 + (i*k+j)%7 * 0.001)
    // = 512 + 0.001 * sum((i*k+j)%7 for j in 0..512)
    eprintln!("bf16 matmul K=512 with small perturbations:");
    eprintln!("  gpu first row: {:?}", &cv[..8]);
    let cpu: Vec<f32> = (0..m).map(|i| {
        (0..k).map(|j| 1.0 + ((i*k+j) % 7) as f32 * 0.001).sum::<f32>()
    }).collect();
    eprintln!("  cpu first row: {:?}", &cpu[..8]);

    // More sensitive: alternating positive/negative -> sums to zero in f32, lost in bf16
    // each row: [1, -1, 1, -1, ...] of length 512 -> sums to 0
    let aa: Vec<half::bf16> = (0..m*k).map(|i| half::bf16::from_f32(if i%2 == 0 {1.0} else {-1.0})).collect();
    let bb: Vec<half::bf16> = vec![half::bf16::from_f32(1.0); k*kn];
    let A: Tensor<Gpu,2> = Tensor::from_data(TensorData::new(aa, [m,k]), &device);
    let B: Tensor<Gpu,2> = Tensor::from_data(TensorData::new(bb, [k,kn]), &device);
    let C = A.matmul(B);
    let cv: Vec<f32> = C.into_data().convert::<f32>().into_vec().unwrap();
    eprintln!("matmul K=512, alternating ±1, expect 0: {:?}", &cv[..8]);

    // Conv-like: simulate what conv1d ic=32 -> oc=1 k=7 with small input does.
    // Total accumulation = 32*7 = 224 multiply-adds.
    use burn::nn::conv::{Conv1d, Conv1dConfig};
    use burn::nn::PaddingConfig1d;
    let cfg = Conv1dConfig::new(32, 1, 7).with_padding(PaddingConfig1d::Same).with_bias(false);
    let mut conv: Conv1d<Gpu> = cfg.init(&device);
    // Set weights = 1/224 each so ideal output = mean of input window ≈ input value
    let wv: Vec<half::bf16> = vec![half::bf16::from_f32(1.0/224.0); 32*7];
    let wt: Tensor<Gpu, 3> = Tensor::from_data(TensorData::new(wv, [1, 32, 7]), &device);
    conv.weight = burn::module::Param::from_tensor(wt);
    // input: ones
    let xv: Vec<half::bf16> = vec![half::bf16::from_f32(0.176); 1*32*30720];
    let x: Tensor<Gpu, 3> = Tensor::from_data(TensorData::new(xv, [1, 32, 30720]), &device);
    let y = conv.forward(x);
    let yv: Vec<f32> = y.into_data().convert::<f32>().into_vec().unwrap();
    let mx = yv.iter().fold(0f32, |a,b| a.max(b.abs()));
    let mn = yv.iter().map(|x|x.abs()).sum::<f32>()/yv.len() as f32;
    eprintln!("conv 32->1 k=7 with 1/224 weights, in=0.176 (expect ~0.176): absmax={mx} mean={mn} first8={:?}", &yv[..8]);
}
