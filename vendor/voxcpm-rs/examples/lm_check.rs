//! Debug: run text-only prefill through base_lm, print enc_out stats.
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
    let text: Tensor<B, 2, Int> = Tensor::from_data(TensorData::new(tokens, [1, s]), &device);

    // embed then scale
    let text_embed = model.base_lm.embed(text);
    let [_, _, h] = text_embed.dims();
    let data = text_embed.clone().into_data();
    let sl = data.as_slice::<f32>().unwrap();
    let mut abs_sum = 0f64; let mut abs_max = 0f32;
    for v in sl { abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); }
    eprintln!("text_embed shape: [1,{s},{h}] mean_abs={} abs_max={}", abs_sum / sl.len() as f64, abs_max);
    eprintln!("text_embed[0,0,:8]: {:?}", &sl[..8]);
    eprintln!("text_embed[0,-1,:8]: {:?}", &sl[(s-1)*h..(s-1)*h+8]);

    let (enc_out, _kv) = model.base_lm.forward(text_embed, true);
    let data = enc_out.clone().into_data();
    let sl = data.as_slice::<f32>().unwrap();
    let mut abs_sum = 0f64; let mut abs_max = 0f32; let mut sum = 0f64;
    for v in sl { abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); sum += *v as f64; }
    eprintln!("enc_out mean={} mean_abs={} abs_max={}", sum / sl.len() as f64, abs_sum / sl.len() as f64, abs_max);
    eprintln!("enc_out[0,-1,:8]: {:?}", &sl[(s-1)*h..(s-1)*h+8]);
    eprintln!("enc_out[0,-1,-8:]: {:?}", &sl[s*h-8..s*h]);

    // Residual LM prefill: residual_input = fusion_concat_proj(cat(enc_out, zeros))
    let feat_embed_zero: Tensor<B, 3> = Tensor::zeros([1, s, h], &device);
    let residual_input = model.fusion_concat_proj.forward(Tensor::cat(vec![enc_out, feat_embed_zero], 2));
    let data = residual_input.clone().into_data();
    let sl = data.as_slice::<f32>().unwrap();
    let mut abs_sum = 0f64; let mut abs_max = 0f32;
    for v in sl { abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); }
    eprintln!("residual_input mean_abs={} abs_max={}", abs_sum / sl.len() as f64, abs_max);
    eprintln!("residual_input[0,-1,:8]: {:?}", &sl[(s-1)*h..(s-1)*h+8]);

    let (res_out, _kv) = model.residual_lm.forward(residual_input, true);
    let data = res_out.into_data();
    let sl = data.as_slice::<f32>().unwrap();
    let mut abs_sum = 0f64; let mut abs_max = 0f32; let mut sum = 0f64;
    for v in sl { abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); sum += *v as f64; }
    eprintln!("residual_out mean={} mean_abs={} abs_max={}", sum/sl.len() as f64, abs_sum/sl.len() as f64, abs_max);
    eprintln!("residual_out[0,-1,:8]: {:?}", &sl[(s-1)*h..(s-1)*h+8]);
    eprintln!("residual_out[0,-1,-8:]: {:?}", &sl[s*h-8..s*h]);

    // stop_head check
    let tt: Tensor<B, 2, Int> = Tensor::from_data(TensorData::new(vec![21045i64,2809,72,101], [1usize,4]), &device);
    let te = model.base_lm.embed(tt);
    let (enc_out2, _) = model.base_lm.forward(te, true);
    let lm_hidden: Tensor<B, 2> = enc_out2.narrow(1, s-1, 1).squeeze_dim::<2>(1);
    let stop_logits = model.stop_head.forward(burn::tensor::activation::silu(model.stop_proj.forward(lm_hidden.clone())));
    let d = stop_logits.into_data();
    let sl = d.as_slice::<f32>().unwrap();
    eprintln!("stop_logits: {:?}", sl);

    // ===== DiT direct test =====
    // dit_hidden: cat(lm_to_dit_proj(lm_hidden), res_to_dit_proj(residual_hidden))
    let res_out_tensor = {
        let tt2: Tensor<B, 2, Int> = Tensor::from_data(TensorData::new(vec![21045i64,2809,72,101], [1usize,4]), &device);
        let te2 = model.base_lm.embed(tt2);
        let (enc2, _) = model.base_lm.forward(te2, true);
        let [_, ss, hh] = enc2.dims();
        let feat_zero = Tensor::<B, 3>::zeros([1, ss, hh], &device);
        let ri = model.fusion_concat_proj.forward(Tensor::cat(vec![enc2, feat_zero], 2));
        let (ro, _) = model.residual_lm.forward(ri, true);
        ro
    };
    let residual_hidden: Tensor<B, 2> = res_out_tensor.narrow(1, s-1, 1).squeeze_dim::<2>(1);
    let h1 = model.lm_to_dit_proj.forward(lm_hidden.clone());
    let h2 = model.res_to_dit_proj.forward(residual_hidden);
    let dit_hidden: Tensor<B, 2> = Tensor::cat(vec![h1, h2], 1);
    let d = dit_hidden.clone().into_data();
    let sl = d.as_slice::<f32>().unwrap();
    eprintln!("dit_hidden[0,:8]: {:?}", &sl[..8]);
    let mut abs_sum = 0f64; let mut abs_max = 0f32;
    for v in sl { abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); }
    eprintln!("dit_hidden mean_abs={} abs_max={}", abs_sum/sl.len() as f64, abs_max);

    // Test DiT estimator directly
    let x_in: Tensor<B,3> = Tensor::ones([1, 64, 4], &device).mul_scalar(0.5);
    let cond_in: Tensor<B,3> = Tensor::zeros([1, 64, 1], &device);
    let t_in: Tensor<B,1> = Tensor::from_data(TensorData::new(vec![0.5f32], [1]), &device);
    let dt_in: Tensor<B,1> = Tensor::zeros([1], &device);
    let dit_out = model.feat_decoder.estimator.forward(x_in, dit_hidden, t_in, cond_in, dt_in);
    let dims = dit_out.dims();
    eprintln!("dit_out shape: {:?}", dims);
    let d = dit_out.into_data();
    let sl = d.as_slice::<f32>().unwrap();
    // shape [1, 64, 4] row-major: index [0,i,0]=i*4+0; [0,i,-1]=i*4+3
    let first: Vec<f32> = (0..8).map(|i| sl[i*4]).collect();
    let last: Vec<f32> = (0..8).map(|i| sl[i*4+3]).collect();
    eprintln!("dit_out2[0,:8,0]: {:?}", first);
    eprintln!("dit_out2[0,:8,-1]: {:?}", last);
    let mut abs_sum = 0f64; let mut abs_max = 0f32; let mut sum = 0f64;
    for v in sl { sum += *v as f64; abs_sum += v.abs() as f64; abs_max = abs_max.max(v.abs()); }
    eprintln!("dit_out2 mean={} mean_abs={} abs_max={}", sum/sl.len() as f64, abs_sum/sl.len() as f64, abs_max);
}
