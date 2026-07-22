//! Probe whether GPU throughput scales with batch size at decode time.
//!
//! This is the gating measurement for the parallel-segment generation
//! design. It loads the real VoxCPM2 model and measures wall-time per
//! decode step for batch sizes 1, 2, 4, 8 on:
//!   - the base LM (forward_step, the dominant cost)
//!   - the residual LM (also forward_step)
//!   - the DiT decoder (forward_cached, which already runs at batch=2 for CFG)
//!
//! Decision rule:
//!   - If batch=2 takes ~1.0-1.2× the time of batch=1: GPU is launch-bound,
//!     parallel-segment will give near-2× throughput. GREEN LIGHT.
//!   - If batch=2 takes ~1.4-1.7×: partial scaling, ~1.3-1.5× throughput.
//!     Yellow light, still worth doing for long inputs.
//!   - If batch=2 takes ~1.8-2.0×: GPU is already saturated. RED LIGHT,
//!     parallel-segment will not help.
//!
//! Run: cargo run --release --example batch_scaling \
//!      --no-default-features --features vulkan -- /path/to/VoxCPM2

#![recursion_limit = "256"]

use std::env;
use std::time::Instant;

use burn::prelude::*;
use burn::tensor::TensorData;
use voxcpm_rs::VoxCPM;

#[cfg(all(feature = "vulkan", not(feature = "wgpu")))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(feature = "wgpu", not(feature = "vulkan")))]
type B = burn::backend::Wgpu<f32, i32>;
#[cfg(all(feature = "wgpu", feature = "vulkan"))]
type B = burn::backend::Vulkan<half::bf16, i32>;
#[cfg(all(not(feature = "wgpu"), not(feature = "vulkan"), feature = "cpu"))]
type B = burn::backend::NdArray<f32>;

const REPS: usize = 30;
const WARMUP: usize = 5;

fn make_input<const D: usize, S: Into<Shape>>(shape: S, device: &<B as Backend>::Device) -> Tensor<B, D> {
    let shape: Shape = shape.into();
    let n: usize = shape.dims.iter().product();
    let data: Vec<f32> = (0..n).map(|i| ((i % 97) as f32) * 0.01 - 0.5).collect();
    Tensor::<B, D>::from_data(TensorData::new(data, shape), device)
}

fn sync(t: Tensor<B, 2>) {
    // Force GPU→CPU sync to make timing meaningful.
    let _ = t.slice([0..1, 0..1]).into_data();
}

fn bench_base_lm(model: &voxcpm_rs::voxcpm2::VoxCpm2Model<B>, batch: usize, ctx: usize, device: &<B as Backend>::Device) -> f64 {
    let lm_h = model.config.0.lm_config.hidden_size;
    let cfg = &model.config.0.lm_config;
    let mut cache = voxcpm_rs::minicpm4::StaticKvCache::new(
        cfg.num_hidden_layers,
        cfg.num_key_value_heads,
        cfg.head_dim(),
        batch,
        ctx + REPS + WARMUP + 16,
        device,
    );
    // Pre-fill cache up to ctx with a forward-step loop so we attend over a
    // realistic context length.
    for pos in 0..ctx {
        let x = make_input::<2, _>([batch, lm_h], device);
        let _ = model.base_lm.forward_step(x, pos, &mut cache);
    }
    // Warmup
    for _ in 0..WARMUP {
        let x = make_input::<2, _>([batch, lm_h], device);
        let pos = cache.step();
        let out = model.base_lm.forward_step(x, pos, &mut cache);
        sync(out);
    }
    // Measure
    let t0 = Instant::now();
    let mut last: Option<Tensor<B, 2>> = None;
    for _ in 0..REPS {
        let x = make_input::<2, _>([batch, lm_h], device);
        let pos = cache.step();
        last = Some(model.base_lm.forward_step(x, pos, &mut cache));
    }
    sync(last.unwrap());
    let dt = t0.elapsed().as_secs_f64() / REPS as f64 * 1000.0;
    dt
}

fn bench_res_lm(model: &voxcpm_rs::voxcpm2::VoxCpm2Model<B>, batch: usize, ctx: usize, device: &<B as Backend>::Device) -> f64 {
    let lm_h = model.config.0.lm_config.hidden_size;
    let cfg = model.config.0.residual_lm_config();
    let mut cache = voxcpm_rs::minicpm4::StaticKvCache::new(
        cfg.num_hidden_layers,
        cfg.num_key_value_heads,
        cfg.head_dim(),
        batch,
        ctx + REPS + WARMUP + 16,
        device,
    );
    for pos in 0..ctx {
        let x = make_input::<2, _>([batch, lm_h], device);
        let _ = model.residual_lm.forward_step(x, pos, &mut cache);
    }
    for _ in 0..WARMUP {
        let x = make_input::<2, _>([batch, lm_h], device);
        let pos = cache.step();
        let out = model.residual_lm.forward_step(x, pos, &mut cache);
        sync(out);
    }
    let t0 = Instant::now();
    let mut last: Option<Tensor<B, 2>> = None;
    for _ in 0..REPS {
        let x = make_input::<2, _>([batch, lm_h], device);
        let pos = cache.step();
        last = Some(model.residual_lm.forward_step(x, pos, &mut cache));
    }
    sync(last.unwrap());
    t0.elapsed().as_secs_f64() / REPS as f64 * 1000.0
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,wgpu_hal=error,wgpu_core=error,naga=error,cubecl_wgpu=warn"),
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

    let ctx = 256; // Realistic decode-time KV-cache occupancy.

    println!("=== base LM forward_step (28 layers, hidden=2048, ctx={}) ===", ctx);
    println!("{:>6} {:>10} {:>10} {:>12}", "batch", "ms/step", "ms/elem", "throughput×");
    let base_b1 = bench_base_lm(model, 1, ctx, &device);
    println!("{:>6} {:>10.2} {:>10.2} {:>12.2}", 1, base_b1, base_b1 / 1.0, 1.0);
    for &b in &[2usize, 4, 8] {
        let dt = bench_base_lm(model, b, ctx, &device);
        let throughput = (b as f64) * base_b1 / dt;
        println!("{:>6} {:>10.2} {:>10.2} {:>12.2}", b, dt, dt / b as f64, throughput);
    }

    println!("\n=== residual LM forward_step (8 layers, no_rope, hidden=2048, ctx={}) ===", ctx);
    println!("{:>6} {:>10} {:>10} {:>12}", "batch", "ms/step", "ms/elem", "throughput×");
    let res_b1 = bench_res_lm(model, 1, ctx, &device);
    println!("{:>6} {:>10.2} {:>10.2} {:>12.2}", 1, res_b1, res_b1 / 1.0, 1.0);
    for &b in &[2usize, 4, 8] {
        let dt = bench_res_lm(model, b, ctx, &device);
        let throughput = (b as f64) * res_b1 / dt;
        println!("{:>6} {:>10.2} {:>10.2} {:>12.2}", b, dt, dt / b as f64, throughput);
    }

    println!("\nLEGEND:");
    println!("  ms/step      = wall time for one forward_step at this batch size");
    println!("  ms/elem      = ms/step / batch (per-stream cost)");
    println!("  throughput×  = (batch * baseline_ms) / current_ms; ideal = batch size");
    println!("                 ~batch    => perfect scaling, GO");
    println!("                 ~1.0      => no scaling (saturated), STOP");
}
