//! Run full inference path deterministically (via VOXCPM_Z_ZERO=1) and dump feat stats.
use burn::prelude::*;
use burn::tensor::{Int, TensorData};
use voxcpm_rs::VoxCPM;

#[cfg(feature = "wgpu")]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

fn main() {
    let model_dir = std::env::args().nth(1).unwrap_or_else(|| "/home/nipah/dev/ai_space/VoxCPM2".into());
    let device = Default::default();
    let vox: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).unwrap();
    let model = &vox.model;

    let tokens: Vec<i64> = vec![21045, 2809, 72, 101];
    let s = tokens.len();
    let text_token: Tensor<B, 2, Int> = Tensor::from_data(TensorData::new(tokens, [1, s]), &device);
    let text_mask: Tensor<B, 2> = Tensor::ones([1, s], &device);
    let feat_mask: Tensor<B, 2> = Tensor::zeros([1, s], &device);
    let p = model.patch_size();
    let d = model.latent_dim();
    let feat: Tensor<B, 4> = Tensor::zeros([1, s, p, d], &device);

    let (latent, _stop_steps) = model
        .inference(text_token, text_mask, feat, feat_mask, 5, 20, 10, 2.0, None)
        .expect("inference");
    let dims = latent.dims();
    eprintln!("latent shape: {:?}", dims);
    let data = latent.into_data();
    let sl = data.as_slice::<f32>().unwrap();
    let mut sum = 0f64; let mut abs_sum = 0f64; let mut abs_max = 0f32;
    for v in sl { sum += *v as f64; abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); }
    eprintln!("latent mean={} mean_abs={} abs_max={}", sum/sl.len() as f64, abs_sum/sl.len() as f64, abs_max);
    // shape [1, D, T*P]. We want latent[0, :8, 0], latent[0, :8, -1], latent[0, 0, :8]
    let tp = dims[2];
    let first: Vec<f32> = (0..8).map(|i| sl[i*tp]).collect();
    let last:  Vec<f32> = (0..8).map(|i| sl[i*tp + (tp-1)]).collect();
    let row0:  Vec<f32> = (0..8).map(|i| sl[i]).collect();
    eprintln!("latent[0,:8,0]: {:?}", first);
    eprintln!("latent[0,:8,-1]: {:?}", last);
    eprintln!("latent[0,0,:8]: {:?}", row0);
}
