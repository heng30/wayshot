//! End-to-end RTF benchmark: serial vs parallel-segment generation.
//!
//! Drives `VoxCPM::generate` with `parallel_segments` set to several values
//! and measures wall-time vs audio-duration vs serial baseline.
//!
//! Usage:
//!   cargo run --release --example bench_parallel \
//!       --no-default-features --features vulkan -- /path/to/VoxCPM2

#![recursion_limit = "256"]

use std::env;
use std::time::Instant;

use voxcpm_rs::{GenerateOptions, Prompt, VoxCPM};

#[cfg(all(feature = "vulkan", not(feature = "wgpu")))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(feature = "wgpu", feature = "vulkan"))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(not(feature = "wgpu"), not(feature = "vulkan"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

const SAMPLE_TEXT: &str = "The quick brown fox jumps over the lazy dog. \
Hello world, this is a longer sentence than the first. \
Speech synthesis can be made faster than realtime with batched decoding. \
Modern GPUs love bigger work batches at decode time. \
The launch overhead per kernel is the real bottleneck on small shapes. \
With parallel segments and a self-seeded voice we get near linear speedup. \
This is the seventh sentence in our test paragraph. \
And the eighth one to round out the batch. \
A ninth sentence for good measure. \
And finally a tenth one to push the parallel batch.";

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
    let sr = voxcpm.sample_rate();
    eprintln!("loaded.\n");

    let segments = voxcpm_rs::voxcpm2::split_sentences(SAMPLE_TEXT);
    eprintln!("text -> {} segments", segments.len());
    for (i, s) in segments.iter().enumerate() {
        eprintln!("  [{i}] {s:?}");
    }
    eprintln!();

    let outdir = "examples_tmp";
    let _ = std::fs::create_dir_all(outdir);

    // Warmup with a tiny generate to compile shaders.
    {
        let opts = GenerateOptions::builder()
            .timesteps(10)
            .max_len(20)
            .min_len(2)
            .build();
        eprintln!("warmup ...");
        let _ = voxcpm.generate("Hi.", opts).expect("warmup");
    }

    let base_opts = || {
        GenerateOptions::builder()
            .timesteps(10)
            .cfg(2.0)
            .max_len(80)
            .min_len(2)
            .prompt(Prompt::None)
            .build()
    };

    // ---- Serial baseline: whole paragraph as one input ----
    eprintln!("\n=== SERIAL (whole paragraph, no parallel_segments) ===");
    let opts = base_opts();
    let t0 = Instant::now();
    let serial_pcm = voxcpm.generate(SAMPLE_TEXT, opts).expect("serial");
    let serial_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let serial_secs = serial_pcm.len() as f64 / sr as f64;
    let serial_rtf = (serial_ms / 1000.0) / serial_secs;
    eprintln!("  {serial_ms:.0} ms, {} samples ({serial_secs:.2}s audio), RTF={serial_rtf:.3}",
        serial_pcm.len());
    voxcpm_rs::audio::write_wav(format!("{outdir}/parallel_serial.wav"), &serial_pcm, sr).unwrap();

    // ---- Per-sentence serial baseline (apples-to-apples for parallel runs) ----
    eprintln!("\n=== PER-SENTENCE SERIAL (each sentence as separate generate) ===");
    let t0 = Instant::now();
    let mut per_sent_pcm: Vec<f32> = Vec::new();
    for s in &segments {
        let opts = base_opts();
        let pcm = voxcpm.generate(s, opts).expect("per-sentence");
        per_sent_pcm.extend_from_slice(&pcm);
    }
    let per_sent_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let per_sent_secs = per_sent_pcm.len() as f64 / sr as f64;
    let per_sent_rtf = (per_sent_ms / 1000.0) / per_sent_secs;
    eprintln!("  {per_sent_ms:.0} ms, {} samples ({per_sent_secs:.2}s audio), RTF={per_sent_rtf:.3}",
        per_sent_pcm.len());
    voxcpm_rs::audio::write_wav(format!("{outdir}/parallel_per_sent.wav"), &per_sent_pcm, sr).unwrap();

    // ---- Parallel runs ----
    for &n in &[2usize, 4, 8] {
        eprintln!("\n=== PARALLEL (parallel_segments={n}) ===");
        let opts = GenerateOptions::builder()
            .timesteps(10)
            .cfg(2.0)
            .max_len(80)
            .min_len(2)
            .prompt(Prompt::None)
            .parallel_segments(n)
            .build();
        let t0 = Instant::now();
        let pcm = voxcpm.generate(SAMPLE_TEXT, opts).expect("parallel");
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let secs = pcm.len() as f64 / sr as f64;
        let rtf = (ms / 1000.0) / secs;
        let speedup = per_sent_ms / ms;
        eprintln!("  {ms:.0} ms, {} samples ({secs:.2}s audio), RTF={rtf:.3}, speedup_vs_per_sent={speedup:.2}x",
            pcm.len());
        voxcpm_rs::audio::write_wav(format!("{outdir}/parallel_n{n}.wav"), &pcm, sr).unwrap();
    }

    eprintln!("\nWAVs saved in {outdir}/parallel_*.wav");
    eprintln!("\nLEGEND:");
    eprintln!("  RTF = wall_time / audio_duration. RTF<1.0 means faster than realtime.");
    eprintln!("  speedup = serial_ms / parallel_ms.");
}
