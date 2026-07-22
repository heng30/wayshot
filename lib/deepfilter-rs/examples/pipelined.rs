//! Example: Pipelined real-time denoising with separate producer/consumer threads
//!
//! Demonstrates how to run DeepFilter inference on a dedicated thread, decoupled
//! from the audio I/O thread via a bounded channel. This is the recommended pattern
//! for real-time voice applications where ONNX inference is too heavy for the
//! audio callback thread.
//!
//! Architecture:
//!   Audio thread  ──(channel)──►  DeepFilter thread
//!                                   (ONNX inference)
//!
//! Usage: cargo run --example pipelined -- input.wav output.wav [model_dir] [--mode split|combined|stateless]

use deepfilter_rs::{DeepFilterProcessor, SessionMode, HOP_SIZE, SAMPLE_RATE};
use std::path::Path;
use std::sync::mpsc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A job sent from the producer (audio) thread to the DeepFilter consumer thread.
struct DenoiseJob {
    /// Sequential frame index (for ordering / diagnostics).
    frame_idx: u64,
    /// 480 samples of audio to denoise.
    audio: Vec<f32>,
}

/// Result sent back from DeepFilter thread.
struct DenoiseResult {
    denoised: Vec<f32>,
}

fn parse_session_mode(args: &[String]) -> SessionMode {
    for (i, a) in args.iter().enumerate() {
        if a == "--mode" {
            if let Some(val) = args.get(i + 1) {
                return match val.as_str() {
                    "split" => SessionMode::SplitStreaming,
                    "combined" => SessionMode::CombinedStreaming,
                    "stateless" => SessionMode::Stateless,
                    other => {
                        eprintln!("Unknown mode '{}', using auto. Options: split, combined, stateless", other);
                        SessionMode::Auto
                    }
                };
            }
        }
    }
    SessionMode::Auto
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input.wav> <output.wav> [model_dir] [--mode split|combined|stateless]", args[0]);
        std::process::exit(1);
    }

    let session_mode = parse_session_mode(&args);
    let positional: Vec<&String> = args[1..].iter()
        .filter(|a| !a.starts_with('-'))
        .filter(|a| !["split", "combined", "stateless"].contains(&a.as_str()))
        .collect();
    let input_path = positional[0];
    let output_path = positional[1];
    let model_dir = if positional.len() > 2 {
        Path::new(positional[2]).to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("models/dfn3_ll")
    };

    // ── Load input audio ──────────────────────────────────────────────
    let mut reader = hound::WavReader::open(input_path)?;
    let spec = reader.spec();
    println!(
        "Input: {} Hz, {} ch, {:?}",
        spec.sample_rate, spec.channels, spec.sample_format
    );
    if spec.sample_rate != SAMPLE_RATE as u32 {
        eprintln!(
            "Warning: sample rate {} != expected {}. Resample first!",
            spec.sample_rate, SAMPLE_RATE
        );
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect(),
    };
    let mono: Vec<f32> = if spec.channels == 2 {
        samples.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect()
    } else {
        samples
    };

    let total_frames = mono.len() / HOP_SIZE;
    println!(
        "Processing {} frames ({:.2}s) with pipelined threads...",
        total_frames,
        mono.len() as f32 / SAMPLE_RATE as f32
    );

    // ── Channels ──────────────────────────────────────────────────────
    // Job channel is bounded to limit memory if producer outpaces consumer.
    // Result channel is unbounded because the main thread drains it only
    // after the producer loop finishes — a bounded result channel would
    // deadlock once both channels fill up.
    let (job_tx, job_rx) = mpsc::sync_channel::<DenoiseJob>(500);
    let (result_tx, result_rx) = mpsc::channel::<DenoiseResult>();

    let underrun_count = Arc::new(AtomicU64::new(0));
    let underrun_count_thread = Arc::clone(&underrun_count);

    // ── DeepFilter consumer thread ────────────────────────────────────
    let df_thread = std::thread::spawn(move || -> Vec<u64> {
        let mut proc = DeepFilterProcessor::with_mode(&model_dir, session_mode, Some(2))
            .expect("load DeepFilter model");
        proc.warmup().expect("DeepFilter warmup");
        let frame_budget = std::time::Duration::from_secs_f64(HOP_SIZE as f64 / SAMPLE_RATE as f64);
        eprintln!("DeepFilter ready: {} [{}] (budget: {:.2}ms/frame)",
                  proc.variant().name(), proc.inference_mode_name(),
                  frame_budget.as_secs_f64() * 1000.0);

        let mut denoised = vec![0.0f32; HOP_SIZE];
        let mut max_frame_time = std::time::Duration::ZERO;
        let mut total_frame_time = std::time::Duration::ZERO;
        let mut frame_count: u64 = 0;
        let mut latencies_us: Vec<u64> = Vec::new();

        while let Ok(job) = job_rx.recv() {
            denoised.fill(0.0);

            let t0 = std::time::Instant::now();
            proc.process_frame(&job.audio, &mut denoised)
                .expect("denoise frame");
            let dt = t0.elapsed();

            total_frame_time += dt;
            frame_count += 1;
            latencies_us.push(dt.as_micros() as u64);
            if dt > max_frame_time {
                max_frame_time = dt;
            }
            if dt > frame_budget {
                let n = underrun_count_thread.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!("UNDERRUN frame {}: {:.2}ms > {:.2}ms budget (total: {})",
                          job.frame_idx, dt.as_secs_f64() * 1000.0,
                          frame_budget.as_secs_f64() * 1000.0, n);
            }

            let _ = result_tx.send(DenoiseResult {
                denoised: denoised.clone(),
            });
        }

        if frame_count > 0 {
            let avg_ms = total_frame_time.as_secs_f64() * 1000.0 / frame_count as f64;
            eprintln!("Consumer stats: {} frames, avg: {:.2}ms, max: {:.2}ms",
                      frame_count, avg_ms, max_frame_time.as_secs_f64() * 1000.0);
        }

        latencies_us
    });

    // ── Producer: simulate audio I/O thread ───────────────────────────
    let start = std::time::Instant::now();
    for (i, chunk) in mono.chunks(HOP_SIZE).enumerate() {
        if chunk.len() < HOP_SIZE {
            break; // skip incomplete final frame
        }
        // In a real app, this is where you'd run any fast pre-processing
        // (e.g. gain, filtering) before sending to the DeepFilter thread:
        //
        //   dsp.process(&raw_mic, &mut processed);
        //   job_tx.send(DenoiseJob { audio: processed, ... });
        //
        let job = DenoiseJob {
            frame_idx: i as u64,
            audio: chunk.to_vec(),
        };
        job_tx.send(job)?;
    }
    drop(job_tx); // signal consumer to finish
    eprintln!("[DEBUG] producer done, job_tx dropped");

    // ── Collect results ───────────────────────────────────────────────
    let mut out_samples: Vec<f32> = Vec::with_capacity(mono.len());
    let mut collected = 0u64;
    while let Ok(result) = result_rx.recv() {
        out_samples.extend_from_slice(&result.denoised);
        collected += 1;
    }
    eprintln!("[DEBUG] collected {} results, waiting for df_thread join", collected);

    let latencies_us = df_thread.join().unwrap();
    eprintln!("[DEBUG] df_thread joined");

    let elapsed = start.elapsed();
    let rtf = elapsed.as_secs_f32() / (mono.len() as f32 / SAMPLE_RATE as f32);
    let underruns = underrun_count.load(Ordering::Relaxed);
    println!("Done in {:.2}s (RTF: {:.3}x realtime, underruns: {})",
             elapsed.as_secs_f32(), rtf, underruns);

    // ── Write output ──────────────────────────────────────────────────
    let out_spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output_path, out_spec)?;
    for &s in &out_samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    writer.finalize()?;

    println!("Saved to {}", output_path);

    // Write per-frame latency CSV
    let csv_path = format!("{}.csv", output_path);
    let mut csv = String::from("frame,latency_us,latency_ms\n");
    for (i, &us) in latencies_us.iter().enumerate() {
        csv.push_str(&format!("{},{},{:.3}\n", i, us, us as f64 / 1000.0));
    }
    std::fs::write(&csv_path, &csv)?;
    println!("Latency CSV written to {}", csv_path);

    Ok(())
}
