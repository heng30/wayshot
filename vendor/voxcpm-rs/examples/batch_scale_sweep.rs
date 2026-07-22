//! Batch-size scaling sweep with uniform-length utterances.
//!
//! Sweeps batch sizes 1, 2, 4, 8, 16, 32, 64 (configurable). Each run
//! generates the SAME equal-length sentence N times in one batch so that
//! no element dominates the per-step cost. We measure:
//!   - wall time per batch
//!   - RTF (wall / total audio)
//!   - throughput (audio seconds per wall second)
//!   - speedup vs b=1
//!
//! Usage:
//!   cargo run --release --example batch_scale_sweep \
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

// One medium sentence used as every batch element.
const TEXT: &str = "The quick brown fox jumps over the lazy dog every single morning.";

const BATCH_SIZES: &[usize] = &[1, 2, 4, 8, 16, 32, 64];

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

    // Warmup
    {
        let opts = GenerateOptions::builder().timesteps(10).max_len(20).min_len(2).build();
        eprintln!("warmup ...");
        let _ = voxcpm.generate("Hi.", opts).expect("warmup");
        eprintln!();
    }

    let mk_opts = || {
        GenerateOptions::builder()
            .timesteps(10)
            .cfg(2.0)
            .max_len(80)
            .min_len(2)
            .build()
    };

    eprintln!("text: {TEXT:?}");
    eprintln!("sweeping batch sizes: {BATCH_SIZES:?}\n");

    println!(
        "{:>5} | {:>10} | {:>10} | {:>8} | {:>14} | {:>10}",
        "B", "wall(ms)", "audio(s)", "RTF", "throughput(x)", "speedup"
    );
    println!("{:-<5}-+-{:-<10}-+-{:-<10}-+-{:-<8}-+-{:-<14}-+-{:-<10}", "", "", "", "", "", "");

    let mut baseline_ms_per_item: Option<f64> = None;

    for &b in BATCH_SIZES {
        // Build a batch of `b` copies of the same sentence.
        let mut bb = voxcpm.batch();
        for _ in 0..b {
            bb = bb.add(TEXT, Prompt::None);
        }
        let t0 = Instant::now();
        let outs = match bb.run(mk_opts()) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("  b={b}: ERROR {e:?}");
                break;
            }
        };
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let total_samples: usize = outs.iter().map(|p| p.len()).sum();
        let audio_s = total_samples as f64 / sr as f64;
        let rtf = (ms / 1000.0) / audio_s.max(1e-9);
        let throughput = audio_s / (ms / 1000.0); // audio-seconds per wall-second
        let ms_per_item = ms / b as f64;
        let speedup = baseline_ms_per_item.map(|base| base / ms_per_item).unwrap_or(1.0);
        if baseline_ms_per_item.is_none() {
            baseline_ms_per_item = Some(ms_per_item);
        }
        println!(
            "{b:>5} | {ms:>10.0} | {audio_s:>10.2} | {rtf:>8.3} | {throughput:>14.2} | {speedup:>9.2}x"
        );
    }

    eprintln!("\nLEGEND:");
    eprintln!("  RTF = wall_seconds / total_audio_seconds. <1.0 means faster than realtime.");
    eprintln!("  throughput = total_audio_seconds / wall_seconds. Higher = better.");
    eprintln!("  speedup = (wall_per_item at b=1) / (wall_per_item at b). Linear scaling = b.");
    eprintln!("  When speedup stops growing, you've saturated the GPU.");
}
