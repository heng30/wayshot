//! Validate that VoxCpm2Model::inference accepts B>1 inputs and produces
//! correct per-element latents when given identical replicated inputs.
//!
//! If this passes, the entire batched-decode mechanism is already supported
//! by the model and we just need a public API + per-element stop + variable
//! length text padding to get parallel-segment generation.
//!
//! Decision rule:
//!   - All N batch outputs match the serial baseline (max-abs diff < 1e-3): GO.
//!   - Wall-time of batch=N ≈ 1× serial: huge throughput win.
//!
//! Run: cargo run --release --example parallel_validate \
//!      --no-default-features --features vulkan -- /path/to/VoxCPM2

#![recursion_limit = "256"]

use std::env;
use std::time::Instant;

use burn::prelude::*;
use voxcpm_rs::voxcpm2::model::VoxCpm2Model;
use voxcpm_rs::{GenerateOptions, Prompt, VoxCPM};

#[cfg(all(feature = "vulkan", not(feature = "wgpu")))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(feature = "wgpu", feature = "vulkan"))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(not(feature = "wgpu"), not(feature = "vulkan"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

fn replicate_batch<const D: usize>(t: Tensor<B, D>, n: usize) -> Tensor<B, D> {
    // Tile along dim 0. Original is [1, ...] -> [N, ...].
    let mut shape = t.dims().to_vec();
    let _b0 = shape[0];
    shape[0] = n;
    let mut out = vec![t.clone(); n];
    Tensor::cat(out.drain(..).collect(), 0)
}

fn replicate_batch_int(t: Tensor<B, 2, burn::tensor::Int>, n: usize) -> Tensor<B, 2, burn::tensor::Int> {
    let mut out = vec![t.clone(); n];
    Tensor::cat(out.drain(..).collect(), 0)
}

fn samples_from_latent(model: &VoxCpm2Model<B>, latent: Tensor<B, 3>) -> Vec<f32> {
    // latent: [1, D, T*P] -> waveform [1, 1, T_out] -> [T_out]
    let wav = model.audio_vae.decode(latent);
    let wav = wav.squeeze_dim::<2>(1).squeeze_dim::<1>(0);
    wav.into_data().convert::<f32>().into_vec::<f32>().expect("vae output")
}

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut m = 0.0f32;
    for i in 0..n {
        let d = (a[i] - b[i]).abs();
        if d > m {
            m = d;
        }
    }
    m
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
    eprintln!("loaded.\n");

    let text = "The quick brown fox jumps over the lazy dog.";
    let opts = GenerateOptions::builder()
        .timesteps(10)
        .cfg(2.0)
        .max_len(80)
        .min_len(2)
        .prompt(Prompt::None)
        .build();

    // ---- Serial baseline: drive inference manually so we can keep the latent
    // (instead of going through generate(), which decodes to PCM internally).
    eprintln!("=== Serial baseline (B=1) ===");
    let mut serial_samples: Option<Vec<f32>> = None;
    let mut serial_ms: f64 = 0.0;
    {
        // Build inference inputs by replaying what generate() does internally.
        // We can't access build_inference_inputs (private), so just call generate
        // for the audio, then re-run with manual inference for the latent.
        let t0 = Instant::now();
        let wav = voxcpm.generate(text, opts.clone()).expect("serial generate");
        serial_ms = t0.elapsed().as_secs_f64() * 1000.0;
        serial_samples = Some(wav);
    }
    eprintln!("serial: {:.0} ms, {} samples\n", serial_ms, serial_samples.as_ref().unwrap().len());

    // ---- Batched: replicate same input N times and call inference at B=N.
    // We need to construct the inputs ourselves since build_inference_inputs is private.
    // Easiest: do the same path as wrapper.rs build_inference_inputs.
    let model = &voxcpm.model;

    // Tokenize the same way wrapper does.
    let mut text_tokens = voxcpm.tokenizer.encode(text).expect("tokenize");
    text_tokens.push(voxcpm_rs::voxcpm2::model::AUDIO_START_TOKEN);
    let text_len = text_tokens.len();
    let p = model.patch_size();
    let d = model.latent_dim();

    // Single-batch inputs first (matches wrapper exactly for Prompt::None).
    let text_token: Tensor<B, 2, burn::tensor::Int> = Tensor::from_data(
        burn::tensor::TensorData::new(text_tokens.clone(), [1, text_len]),
        &device,
    );
    let text_mask: Tensor<B, 2> = Tensor::ones([1, text_len], &device);
    let feat_mask: Tensor<B, 2> = Tensor::zeros([1, text_len], &device);
    let feat: Tensor<B, 4> = Tensor::zeros([1, text_len, p, d], &device);

    // Re-run serial via inference() to get the latent (for direct comparison).
    eprintln!("=== Re-running serial via model.inference (B=1) for latent capture ===");
    let t0 = Instant::now();
    let (serial_latent, serial_stops) = model
        .inference(
            text_token.clone(),
            text_mask.clone(),
            feat.clone(),
            feat_mask.clone(),
            opts.min_len,
            opts.max_len,
            opts.inference_timesteps,
            opts.cfg_value as f64,
            None,
        )
        .expect("serial inference");
    let serial_inf_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let serial_samples_b1 = samples_from_latent(model, serial_latent.clone());
    let serial_latent_dims = serial_latent.dims();
    eprintln!(
        "serial inference: {:.0} ms, latent {:?}, stops={:?}, {} samples",
        serial_inf_ms, serial_latent_dims, serial_stops, serial_samples_b1.len()
    );

    for &n in &[2usize, 4, 8] {
        eprintln!("\n=== Batched (B={n}) ===");
        let bt = replicate_batch_int(text_token.clone(), n);
        let btm = replicate_batch(text_mask.clone(), n);
        let bf = replicate_batch(feat.clone(), n);
        let bfm = replicate_batch(feat_mask.clone(), n);

        let t0 = Instant::now();
        let (batched_latent, batched_stops) = match model.inference(
            bt, btm, bf, bfm,
            opts.min_len,
            opts.max_len,
            opts.inference_timesteps,
            opts.cfg_value as f64,
            None,
        ) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("FAILED at B={n}: {e:?}");
                continue;
            }
        };
        let batched_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let bd = batched_latent.dims();
        eprintln!("batched: {:.0} ms (vs {:.0} serial × {} = {:.0} expected serial-equiv), latent {:?}, stops={:?}",
            batched_ms, serial_inf_ms, n, serial_inf_ms * n as f64, bd, batched_stops);
        let speedup = (serial_inf_ms * n as f64) / batched_ms;
        eprintln!("THROUGHPUT speedup: {:.2}× (ideal = {n})", speedup);

        // Compare each batch element to serial.
        let mut max_diff_overall = 0.0f32;
        for i in 0..n {
            let lat_i = batched_latent.clone().slice([i..i + 1, 0..bd[1], 0..bd[2]]);
            let samples_i = samples_from_latent(model, lat_i);
            let n_compare = samples_i.len().min(serial_samples_b1.len());
            let diff = max_abs_diff(&samples_i[..n_compare], &serial_samples_b1[..n_compare]);
            if diff > max_diff_overall { max_diff_overall = diff; }
            eprintln!(
                "  elem {i}: {} samples (serial={}), max-abs-diff over first {n_compare} = {:.6}",
                samples_i.len(), serial_samples_b1.len(), diff
            );
        }
        eprintln!("  >>> overall max diff: {:.6}", max_diff_overall);
    }
}
