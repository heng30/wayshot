//! Validate batched inference with TWO DIFFERENT text inputs of unequal length.
//!
//! Right-pads both segments to max(S) with zero text-tokens + zero feat
//! patches, runs through model.inference at B=2, then VAE-decodes each
//! element using its own stop_step. Compares each segment's audio to a
//! serial baseline at B=1.
//!
//! If the audio is recognizable / similar to serial, the simplest possible
//! batching (right-pad + causal attention + accept pad K/V in cache) works
//! and we can ship it. If not, we need per-element key-padding masks.
//!
//! Run: cargo run --release --example parallel_varlen \
//!      --no-default-features --features vulkan -- /path/to/VoxCPM2

#![recursion_limit = "256"]

use std::env;
use std::time::Instant;

use burn::prelude::*;
use burn::tensor::{Int, TensorData};
use voxcpm_rs::voxcpm2::model::{AUDIO_START_TOKEN, VoxCpm2Model};
use voxcpm_rs::{GenerateOptions, Prompt, VoxCPM};

#[cfg(all(feature = "vulkan", not(feature = "wgpu")))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(feature = "wgpu", feature = "vulkan"))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(not(feature = "wgpu"), not(feature = "vulkan"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

/// Build single-batch inputs identical to wrapper.rs (Prompt::None case),
/// returning (text_token, text_mask, feat, feat_mask) all with B=1.
fn build_single_inputs(
    voxcpm: &VoxCPM<B>,
    text: &str,
    device: &<B as Backend>::Device,
) -> (
    Tensor<B, 2, Int>,
    Tensor<B, 2>,
    Tensor<B, 4>,
    Tensor<B, 2>,
) {
    let mut tokens = voxcpm.tokenizer.encode(text).expect("tokenize");
    tokens.push(AUDIO_START_TOKEN);
    let s = tokens.len();
    let p = voxcpm.model.patch_size();
    let d = voxcpm.model.latent_dim();

    let text_token: Tensor<B, 2, Int> =
        Tensor::from_data(TensorData::new(tokens, [1, s]), device);
    let text_mask: Tensor<B, 2> = Tensor::ones([1, s], device);
    let feat_mask: Tensor<B, 2> = Tensor::zeros([1, s], device);
    let feat: Tensor<B, 4> = Tensor::zeros([1, s, p, d], device);
    (text_token, text_mask, feat, feat_mask)
}

/// Right-pad single-batch inputs to the given max_s. Pads text-token=0,
/// text/feat masks=0, feat patches=0. Returns (padded_inputs, real_len).
fn pad_to(
    inp: (Tensor<B, 2, Int>, Tensor<B, 2>, Tensor<B, 4>, Tensor<B, 2>),
    max_s: usize,
    device: &<B as Backend>::Device,
) -> (
    Tensor<B, 2, Int>,
    Tensor<B, 2>,
    Tensor<B, 4>,
    Tensor<B, 2>,
    usize,
) {
    let (tt, tm, ft, fm) = inp;
    let s = tt.dims()[1];
    if s == max_s {
        return (tt, tm, ft, fm, s);
    }
    let pad = max_s - s;
    let p = ft.dims()[2];
    let d = ft.dims()[3];

    let pad_tt: Tensor<B, 2, Int> = Tensor::zeros([1, pad], device);
    let pad_tm: Tensor<B, 2> = Tensor::zeros([1, pad], device);
    let pad_fm: Tensor<B, 2> = Tensor::zeros([1, pad], device);
    let pad_ft: Tensor<B, 4> = Tensor::zeros([1, pad, p, d], device);

    let tt = Tensor::cat(vec![tt, pad_tt], 1);
    let tm = Tensor::cat(vec![tm, pad_tm], 1);
    let fm = Tensor::cat(vec![fm, pad_fm], 1);
    let ft = Tensor::cat(vec![ft, pad_ft], 1);
    (tt, tm, ft, fm, s)
}

fn samples_from_latent(model: &VoxCpm2Model<B>, latent: Tensor<B, 3>) -> Vec<f32> {
    let wav = model.audio_vae.decode(latent);
    let wav = wav.squeeze_dim::<2>(1).squeeze_dim::<1>(0);
    wav.into_data()
        .convert::<f32>()
        .into_vec::<f32>()
        .expect("vae output")
}

fn rms(x: &[f32]) -> f32 {
    let n = x.len().max(1) as f32;
    (x.iter().map(|v| v * v).sum::<f32>() / n).sqrt()
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("warn,wgpu_hal=error,wgpu_core=error,naga=error,cubecl_wgpu=warn"),
    )
    .init();

    let model_dir = env::args()
        .nth(1)
        .unwrap_or_else(|| "/home/nipah/dev/ai_space/VoxCPM2".to_string());
    let device = Default::default();

    eprintln!("loading model from {model_dir} ...");
    let voxcpm: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).expect("load model");
    let model = &voxcpm.model;
    eprintln!("loaded.\n");

    // Two SHORT but different sentences. Different lengths.
    let text_a = "The quick brown fox.";
    let text_b = "Hello world, this is a longer sentence than the first one.";

    let opts = GenerateOptions::builder()
        .timesteps(10)
        .cfg(2.0)
        .max_len(80)
        .min_len(2)
        .prompt(Prompt::None)
        .build();

    // ---- Serial baselines (B=1, one per text). ----
    eprintln!("=== Serial baselines ===");
    let (tt_a, tm_a, ft_a, fm_a) = build_single_inputs(&voxcpm, text_a, &device);
    let (tt_b, tm_b, ft_b, fm_b) = build_single_inputs(&voxcpm, text_b, &device);
    let s_a = tt_a.dims()[1];
    let s_b = tt_b.dims()[1];
    eprintln!("text A ({s_a} tokens): {text_a:?}");
    eprintln!("text B ({s_b} tokens): {text_b:?}");

    let t0 = Instant::now();
    let (lat_a, stops_a) = model
        .inference(
            tt_a.clone(), tm_a.clone(), ft_a.clone(), fm_a.clone(),
            opts.min_len, opts.max_len, opts.inference_timesteps, opts.cfg_value as f64, None,
        )
        .expect("serial A");
    let ms_a = t0.elapsed().as_secs_f64() * 1000.0;
    let samples_a = samples_from_latent(model, lat_a.clone());
    eprintln!("  serial A: {ms_a:.0}ms, stops={stops_a:?}, {} samples (RMS={:.3})", samples_a.len(), rms(&samples_a));

    let t0 = Instant::now();
    let (lat_b, stops_b) = model
        .inference(
            tt_b.clone(), tm_b.clone(), ft_b.clone(), fm_b.clone(),
            opts.min_len, opts.max_len, opts.inference_timesteps, opts.cfg_value as f64, None,
        )
        .expect("serial B");
    let ms_b = t0.elapsed().as_secs_f64() * 1000.0;
    let samples_b = samples_from_latent(model, lat_b.clone());
    eprintln!("  serial B: {ms_b:.0}ms, stops={stops_b:?}, {} samples (RMS={:.3})", samples_b.len(), rms(&samples_b));

    let serial_total_ms = ms_a + ms_b;
    eprintln!("  serial total: {:.0}ms", serial_total_ms);

    // ---- Batched (B=2): pad both to max(S_a, S_b) and run. ----
    eprintln!("\n=== Batched B=2 (variable text, right-pad with zeros) ===");
    let max_s = s_a.max(s_b);
    eprintln!("padding both to S={max_s}");
    let (tt_ap, tm_ap, ft_ap, fm_ap, _) = pad_to((tt_a, tm_a, ft_a, fm_a), max_s, &device);
    let (tt_bp, tm_bp, ft_bp, fm_bp, _) = pad_to((tt_b, tm_b, ft_b, fm_b), max_s, &device);

    let bt = Tensor::cat(vec![tt_ap, tt_bp], 0);
    let btm = Tensor::cat(vec![tm_ap, tm_bp], 0);
    let bf = Tensor::cat(vec![ft_ap, ft_bp], 0);
    let bfm = Tensor::cat(vec![fm_ap, fm_bp], 0);

    let t0 = Instant::now();
    let (batched_latent, batched_stops) = model
        .inference_with_lengths(
            bt, btm, bf, bfm,
            opts.min_len, opts.max_len, opts.inference_timesteps, opts.cfg_value as f64, None,
            Some(vec![s_a, s_b]),
        )
        .expect("batched");
    let batched_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let bd = batched_latent.dims();
    eprintln!("batched: {batched_ms:.0}ms, latent {bd:?}, stops={batched_stops:?}");
    eprintln!("speedup vs serial sum: {:.2}× (ideal=2)", serial_total_ms / batched_ms);

    // Decode each element using its own stop count.
    for i in 0..2 {
        let stop_i = batched_stops[i];
        let pat = stop_i * voxcpm.model.patch_size();
        let lat_i = batched_latent.clone().slice([i..i + 1, 0..bd[1], 0..pat]);
        let samples = samples_from_latent(model, lat_i);
        let label = if i == 0 { "A" } else { "B" };
        let ref_samples = if i == 0 { &samples_a } else { &samples_b };
        let ref_label = if i == 0 { "A" } else { "B" };
        eprintln!(
            "  elem {i} (text {label}): {} samples (serial {ref_label}={}, RMS={:.3})",
            samples.len(),
            ref_samples.len(),
            rms(&samples),
        );
    }
    eprintln!("\nNOTE: audio is non-deterministic (diffusion noise), so element vs serial");
    eprintln!("comparison is by RMS / qualitative listening, not bit-exact diff.");
    eprintln!("If RMS is in the same ballpark (~0.05-0.2) and number of samples is reasonable,");
    eprintln!("the batched outputs are valid. Listen to them in the next step.");

    // Save WAVs for listening.
    let sr = voxcpm.sample_rate();
    let outdir = "examples_tmp";
    let _ = std::fs::create_dir_all(outdir);
    let save = |path: &str, s: &[f32]| {
        voxcpm_rs::audio::write_wav(path, s, sr).unwrap();
    };
    save(&format!("{outdir}/varlen_serial_A.wav"), &samples_a);
    save(&format!("{outdir}/varlen_serial_B.wav"), &samples_b);

    // Re-run batched, save batched outputs.
    let (tt_a, tm_a, ft_a, fm_a) = build_single_inputs(&voxcpm, text_a, &device);
    let (tt_b, tm_b, ft_b, fm_b) = build_single_inputs(&voxcpm, text_b, &device);
    let (tt_ap, tm_ap, ft_ap, fm_ap, _) = pad_to((tt_a, tm_a, ft_a, fm_a), max_s, &device);
    let (tt_bp, tm_bp, ft_bp, fm_bp, _) = pad_to((tt_b, tm_b, ft_b, fm_b), max_s, &device);
    let bt = Tensor::cat(vec![tt_ap, tt_bp], 0);
    let btm = Tensor::cat(vec![tm_ap, tm_bp], 0);
    let bf = Tensor::cat(vec![ft_ap, ft_bp], 0);
    let bfm = Tensor::cat(vec![fm_ap, fm_bp], 0);
    let (batched_latent, batched_stops) = model
        .inference_with_lengths(
            bt, btm, bf, bfm,
            opts.min_len, opts.max_len, opts.inference_timesteps, opts.cfg_value as f64, None,
            Some(vec![s_a, s_b]),
        )
        .expect("batched 2");
    let bd = batched_latent.dims();
    for i in 0..2 {
        let stop_i = batched_stops[i];
        let pat = stop_i * voxcpm.model.patch_size();
        let lat_i = batched_latent.clone().slice([i..i + 1, 0..bd[1], 0..pat]);
        let samples = samples_from_latent(model, lat_i);
        let label = if i == 0 { "A" } else { "B" };
        save(&format!("{outdir}/varlen_batched_{label}.wav"), &samples);
    }
    eprintln!("\nSaved WAVs to {outdir}/varlen_*.wav for listening.");
}
