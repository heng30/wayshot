//! Streaming TTS smoke test against a local VoxCPM2 checkpoint.
//!
//! Generates audio chunk-by-chunk via [`VoxCPM::generate_stream`] and writes
//! the concatenated result to a WAV file once finished. Logs per-chunk
//! latency so you can see the streaming behavior in action.
//!
//! Run with:
//!   cargo run --release --example tts_stream --no-default-features --features cpu-blas -- \
//!       /home/nipah/dev/ai_space/VoxCPM2 "Hello, streaming world!" /tmp/stream.wav
//!
//!   cargo run --release --example tts_stream --no-default-features --features wgpu -- \
//!       /home/nipah/dev/ai_space/VoxCPM2 "Hello, streaming world!" /tmp/stream.wav

#![recursion_limit = "256"]

use std::env;
use std::time::Instant;

use voxcpm_rs::{audio, GenerateOptions, VoxCPM};

#[cfg(feature = "wgpu")]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

#[cfg(feature = "wgpu")]
fn backend_name() -> &'static str {
    "wgpu"
}
#[cfg(all(not(feature = "wgpu"), feature = "cpu"))]
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
        .unwrap_or_else(|| "Hello world, this is a streaming test.".to_string());
    let out = args.next().unwrap_or_else(|| "/tmp/stream.wav".to_string());

    let device = Default::default();
    eprintln!("backend: {}", backend_name());
    let t0 = Instant::now();
    eprintln!("loading model from {model_dir} ...");
    let model: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).expect("load model");
    eprintln!("loaded in {:.2?}", t0.elapsed());

    let timesteps = env::var("VOXCPM_TIMESTEPS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);
    let chunk_patches = env::var("VOXCPM_CHUNK_PATCHES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5);

    let opts = GenerateOptions::builder()
        .timesteps(timesteps)
        .cfg(2.0)
        .max_len(500)
        .chunk_patches(chunk_patches)
        .build();

    eprintln!(
        "streaming: {text:?}  (chunk_patches={chunk_patches}, timesteps={timesteps})"
    );
    let sr = model.sample_rate();
    let stream = model
        .generate_stream(&text, opts)
        .expect("init stream");

    let t1 = Instant::now();
    let mut last = t1;
    let mut all: Vec<f32> = Vec::new();
    for (i, chunk) in stream.enumerate() {
        let chunk = chunk.expect("chunk");
        let now = Instant::now();
        let chunk_sec = chunk.len() as f32 / sr as f32;
        eprintln!(
            "  chunk {i:3}: {:5} samples ({:.3}s audio) in {:.2?} (since-start {:.2?})",
            chunk.len(),
            chunk_sec,
            now.duration_since(last),
            now.duration_since(t1),
        );
        all.extend_from_slice(&chunk);
        last = now;
    }
    let elapsed = t1.elapsed();
    let audio_sec = all.len() as f32 / sr as f32;
    eprintln!(
        "done: {} samples @ {} Hz ({:.2}s of audio) in {:.2?} (RTF = {:.2})",
        all.len(),
        sr,
        audio_sec,
        elapsed,
        elapsed.as_secs_f32() / audio_sec
    );
    eprintln!("writing {out}");
    audio::write_wav(&out, &all, sr).expect("write wav");
    eprintln!("done");
}
