//! Variable-length batch demo: 8 utterances of very different lengths
//! generated in a single batched forward pass, then written to disk so
//! you can listen and confirm each one is intact.
//!
//! Usage:
//!   cargo run --release --example batch_varlen \
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

// 8 items, deliberately spanning very short to very long.
const TEXTS: &[&str] = &[
    // 1) Tiny.
    "Hi.",
    // 2) Short greeting.
    "Hello there, friend.",
    // 3) Single medium sentence.
    "The quick brown fox jumps over the lazy dog.",
    // 4) Two short sentences.
    "Welcome to the show. Glad you could make it.",
    // 5) Medium, technical.
    "Speech synthesis can be made faster than realtime with batched decoding on a modern GPU.",
    // 6) Long, multi-clause.
    "Today we are going to talk about why running multiple sequences through a single forward pass amortizes the per-kernel launch overhead and the per-call weight bandwidth, which is the real bottleneck at small batch sizes on most modern accelerators.",
    // 7) Long, narrative.
    "Once upon a time there was a small program written in Rust that could speak in many voices. It learned to talk to its users one sentence at a time, and then one day it learned to talk to many of them at once, which made it much, much faster than it had ever been before.",
    // 8) Very long, dense.
    "When you batch eight independent utterances of wildly different lengths into a single generate call, the system right-pads every row to the longest one and then carries a per-element key padding mask through every attention layer so that no token ever attends to padding positions; the per-element stop head then decides independently for each row when to halt, and the decoder slices each row's latent with its own stop step before turning it into audio, so each output is exactly as long as it should be regardless of what its neighbours did.",
];

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
    let voxcpm: VoxCPM<B> = VoxCPM::from_local(&model_dir, &device).expect("load");
    let sr = voxcpm.sample_rate();
    eprintln!("loaded.\n");

    let outdir = "/tmp/voxbatching";
    std::fs::create_dir_all(outdir).expect("mkdir");

    // Warmup
    {
        let opts = GenerateOptions::builder().timesteps(10).max_len(20).min_len(2).build();
        eprintln!("warmup ...");
        let _ = voxcpm.generate("Hi.", opts).expect("warmup");
    }

    let opts = GenerateOptions::builder()
        .timesteps(10)
        .cfg(2.0)
        .max_len(500)
        .min_len(2)
        .build();

    eprintln!("\n=== BATCH (b=8, variable lengths) ===");
    for (i, t) in TEXTS.iter().enumerate() {
        eprintln!("  [{i}] ({} chars) {:?}", t.len(),
            if t.len() > 80 { format!("{}...", &t[..77]) } else { t.to_string() });
    }

    let t0 = Instant::now();
    let mut bb = voxcpm.batch();
    for &t in TEXTS {
        bb = bb.add(t, Prompt::None);
    }
    let outs = bb.run(opts).expect("batch run");
    let ms = t0.elapsed().as_secs_f64() * 1000.0;

    let mut total_samples = 0usize;
    eprintln!("\n--- per-item results ---");
    for (i, pcm) in outs.iter().enumerate() {
        let secs = pcm.len() as f64 / sr as f64;
        total_samples += pcm.len();
        let path = format!("{outdir}/batch_{i:02}.wav");
        voxcpm_rs::audio::write_wav(&path, pcm, sr).expect("wav");
        eprintln!("  [{i}] {} samples ({secs:.2}s) -> {path}", pcm.len());
    }

    let total_secs = total_samples as f64 / sr as f64;
    let rtf = (ms / 1000.0) / total_secs;
    eprintln!(
        "\nTOTAL: {ms:.0} ms wall, {total_samples} samples ({total_secs:.2}s audio), RTF={rtf:.3}"
    );
    eprintln!(
        "All 8 utterances generated in a SINGLE batched forward pass with variable lengths."
    );
    eprintln!("\nListen at: {outdir}/batch_*.wav");
}
