//! Batch-API benchmark: independent utterances, many at once.
//!
//! Compares per-item serial generation vs `VoxCPM::batch()` at b=2,4,8.
//!
//! Usage:
//!   cargo run --release --example bench_batch \
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

const TEXTS: &[&str] = &[
    "Hello world, this is the first independent utterance.",
    "Speech synthesis can be made faster than realtime with batched decoding.",
    "Modern GPUs love bigger work batches at decode time.",
    "The launch overhead per kernel is the real bottleneck on small shapes.",
    "With parallel batched inputs we get near linear speedup on the GPU.",
    "This is the sixth independent utterance in our batch test.",
    "And the seventh one to round out the workload.",
    "An eighth utterance to fill out the batch nicely.",
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

    let outdir = "examples_tmp";
    let _ = std::fs::create_dir_all(outdir);

    // Warmup
    {
        let opts = GenerateOptions::builder().timesteps(10).max_len(20).min_len(2).build();
        eprintln!("warmup ...");
        let _ = voxcpm.generate("Hi.", opts).expect("warmup");
    }

    let opts = || {
        GenerateOptions::builder()
            .timesteps(10)
            .cfg(2.0)
            .max_len(80)
            .min_len(2)
            .build()
    };

    // Per-item serial baseline
    eprintln!("\n=== SERIAL (one generate per item) ===");
    let t0 = Instant::now();
    let mut total_samples = 0usize;
    for (i, t) in TEXTS.iter().enumerate() {
        let pcm = voxcpm.generate(t, opts()).expect("serial");
        total_samples += pcm.len();
        if i == 0 {
            voxcpm_rs::audio::write_wav(format!("{outdir}/bench_batch_serial_0.wav"), &pcm, sr)
                .unwrap();
        }
    }
    let serial_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let serial_secs = total_samples as f64 / sr as f64;
    let serial_rtf = (serial_ms / 1000.0) / serial_secs;
    eprintln!(
        "  {serial_ms:.0} ms, total {total_samples} samples ({serial_secs:.2}s audio), RTF={serial_rtf:.3}"
    );

    // Batch runs
    for &n in &[2usize, 4, 8] {
        eprintln!("\n=== BATCH (b={n}) ===");
        let t0 = Instant::now();
        let mut total_samples = 0usize;
        let mut wrote_first = false;
        for chunk in TEXTS.chunks(n) {
            let mut bb = voxcpm.batch();
            for &t in chunk {
                bb = bb.add(t, Prompt::None);
            }
            let outs = bb.run(opts()).expect("batch");
            for pcm in &outs {
                total_samples += pcm.len();
            }
            if !wrote_first {
                voxcpm_rs::audio::write_wav(
                    format!("{outdir}/bench_batch_b{n}_0.wav"),
                    &outs[0],
                    sr,
                )
                .unwrap();
                wrote_first = true;
            }
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let secs = total_samples as f64 / sr as f64;
        let rtf = (ms / 1000.0) / secs;
        let speedup = serial_ms / ms;
        eprintln!(
            "  {ms:.0} ms, total {total_samples} samples ({secs:.2}s audio), RTF={rtf:.3}, speedup={speedup:.2}x"
        );
    }

    eprintln!("\nWAVs in {outdir}/bench_batch_*.wav");
}
