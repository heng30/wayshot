//! End-to-end TTS smoke test against a local VoxCPM2 checkpoint.
//!
//! Run with:
//!   # Pure-Rust CPU + SIMD elementwise (matmul still single-threaded):
//!   cargo run --release --example tts --no-default-features --features cpu -- \
//!       /home/nipah/dev/ai_space/VoxCPM2 "Hello world" /tmp/out.wav
//!
//!   # CPU + multi-threaded BLAS (vendored OpenBLAS, recommended for CPU):
//!   cargo run --release --example tts --no-default-features --features cpu-blas -- \
//!       /home/nipah/dev/ai_space/VoxCPM2 "Hello world" /tmp/out.wav
//!
//!   # GPU via Vulkan (AMD / NVIDIA):
//!   cargo run --release --example tts --no-default-features --features wgpu -- \
//!       /home/nipah/dev/ai_space/VoxCPM2 "Hello world" /tmp/out.wav

#![recursion_limit = "256"]

use std::env;
use std::time::Instant;

use voxcpm_rs::{audio, GenerateOptions, VoxCPM};

// NOTE on Vulkan + bf16:
//   The `vulkan` feature runs the model in native bf16 end-to-end via
//   cubecl-spirv. This requires the patched `burn-cubecl` and `cubecl-spirv`
//   crates from this repo (see `patches/README.md` and the workspace
//   `[patch.crates-io]` block) — without them, bf16 conv accumulators
//   collapse and several bf16 ops emit broken SPIR-V on mesa/radv.
#[cfg(all(feature = "vulkan", not(feature = "wgpu")))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(feature = "wgpu", feature = "vulkan"))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(not(feature = "wgpu"), not(feature = "vulkan"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

#[cfg(feature = "vulkan")]
fn backend_name() -> &'static str {
    "vulkan (bf16, SPIR-V)"
}
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
fn backend_name() -> &'static str {
    "wgpu"
}
#[cfg(all(not(feature = "wgpu"), not(feature = "vulkan"), feature = "cpu"))]
fn backend_name() -> &'static str {
    "ndarray"
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(
            "info,wgpu_hal=error,wgpu_core=error,naga=error,cubecl_wgpu=warn",
        ),
    )
    .init();
    let mut args = env::args().skip(1);
    let model_dir = args
        .next()
        .unwrap_or_else(|| "/home/nipah/dev/ai_space/VoxCPM2".to_string());
    let text = args
        .next()
        .unwrap_or_else(|| "Hello world, this is a test.".to_string());
    let out = args.next().unwrap_or_else(|| "/tmp/out.wav".to_string());

    let device = Default::default();
    eprintln!("backend: {}", backend_name());
    let t0 = Instant::now();
    eprintln!("loading model from {model_dir} ...");
    let model: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).expect("load model");
    eprintln!("loaded in {:.2?}", t0.elapsed());
    eprintln!("synthesizing: {text:?}");
    let timesteps = env::var("VOXCPM_TIMESTEPS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);
    let opts = GenerateOptions::builder()
        .timesteps(timesteps)
        .cfg(2.0)
        .max_len(500)
        .build();
    let t1 = Instant::now();
    let wav = model.generate(&text, opts).expect("generate");
    let elapsed = t1.elapsed();
    let sr = model.sample_rate();
    let audio_sec = wav.len() as f32 / sr as f32;
    eprintln!(
        "got {} samples @ {} Hz ({:.2}s of audio) in {:.2?} (RTF = {:.2})",
        wav.len(),
        sr,
        audio_sec,
        elapsed,
        elapsed.as_secs_f32() / audio_sec
    );
    eprintln!("writing {out}");
    audio::write_wav(&out, &wav, sr).expect("write wav");
    eprintln!("done");
}
