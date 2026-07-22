//! Example: Process an audio file with DeepFilterNet
//!
//! Usage: cargo run --example process_file -- input.wav output.wav [model_dir/] [-D] [--mode split|combined|stateless]
//!
//! The -D flag compensates algorithmic delay (STFT + model lookahead) by trimming
//! the first N samples from the output. This matches the Tract CLI's -D behavior
//! and Python's pad=True mode.
//!
//! The --mode flag selects the inference mode:
//!   split     — 4 separate ONNX sessions (default if split files exist)
//!   combined  — single combined_streaming.onnx
//!   stateless — single combined.onnx with 40-frame window warm-up
//!
//! Lookahead is determined by the model variant:
//!   LL models (dfn3_ll, dfn2_ll):   lookahead=0, 10ms delay
//!   Standard models (dfn3_h0, dfn3): lookahead=2, 30ms delay, best quality

use deepfilter_rs::{DeepFilterStream, SessionMode, SAMPLE_RATE, HOP_SIZE};
use std::path::Path;

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
        eprintln!("Usage: {} <input.wav> <output.wav> [model_dir] [-D] [--mode split|combined|stateless]", args[0]);
        std::process::exit(1);
    }

    let compensate_delay = args.iter().any(|a| a == "-D");
    let session_mode = parse_session_mode(&args);
    let positional: Vec<&String> = args[1..].iter()
        .filter(|a| !a.starts_with('-'))
        .filter(|a| !["split", "combined", "stateless"].contains(&a.as_str()))
        .collect();
    if positional.len() < 2 {
        eprintln!("Usage: {} <input.wav> <output.wav> [model_dir] [-D] [--mode split|combined|stateless]", args[0]);
        std::process::exit(1);
    }

    let input_path = positional[0];
    let output_path = positional[1];
    let model_dir = if positional.len() > 2 {
        Path::new(positional[2]).to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("models/dfn3_ll")
    };

    // Create processor
    println!("Loading model from {:?}...", model_dir);
    let mut stream = DeepFilterStream::with_mode(&model_dir, session_mode, Some(2))?;
    let variant = stream.variant();
    println!("Using model variant: {}", variant.name());
    println!("Inference mode: {}", stream.inference_mode_name());
    println!("Lookahead: {} frames ({}ms delay)",
             stream.lookahead(), stream.latency_ms() as u32);

    let delay = stream.delay_samples();
    println!("Algorithmic delay: {} samples ({:.1}ms){}",
             delay, stream.latency_ms(),
             if compensate_delay { " [compensating]" } else { "" });

    // Warm up to avoid cold-start latency affecting timing
    stream.warmup()?;

    // Read input audio
    let mut reader = hound::WavReader::open(input_path)?;
    let spec = reader.spec();

    println!("Input: {} Hz, {} channels, {:?}",
             spec.sample_rate, spec.channels, spec.sample_format);

    if spec.sample_rate != SAMPLE_RATE as u32 {
        eprintln!("Warning: Input sample rate {} != expected {}. Resample first!",
                  spec.sample_rate, SAMPLE_RATE);
    }

    // Read samples
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => reader
                    .samples::<i16>()
                    .map(|s| s.unwrap() as f32 / 32768.0)
                    .collect(),
                24 | 32 => reader
                    .samples::<i32>()
                    .map(|s| s.unwrap() as f32 / 2147483648.0)
                    .collect(),
                _ => {
                    eprintln!("Unsupported bits per sample: {}", spec.bits_per_sample);
                    std::process::exit(1);
                }
            }
        }
    };

    // Convert to mono if stereo
    let mono: Vec<f32> = if spec.channels == 2 {
        samples.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect()
    } else if spec.channels == 1 {
        samples
    } else {
        // Take first channel for multi-channel
        samples.chunks(spec.channels as usize).map(|c| c[0]).collect()
    };

    println!("Processing {} samples ({:.2}s)...", mono.len(),
             mono.len() as f32 / SAMPLE_RATE as f32);

    // Process using streaming API, one HOP_SIZE chunk at a time for latency measurement
    let start = std::time::Instant::now();
    let mut output: Vec<f32> = Vec::with_capacity(mono.len());
    let mut latencies_us: Vec<u64> = Vec::new();

    for chunk in mono.chunks(HOP_SIZE) {
        let t0 = std::time::Instant::now();
        let out = stream.process(chunk)?;
        let dt = t0.elapsed();
        latencies_us.push(dt.as_micros() as u64);
        output.extend(out);
    }
    output.extend(stream.flush()?);

    let elapsed = start.elapsed();
    let rtf = elapsed.as_secs_f32() / (mono.len() as f32 / SAMPLE_RATE as f32);
    println!("Done in {:.2}s (RTF: {:.3}x realtime)", elapsed.as_secs_f32(), rtf);

    // Compensate delay by trimming the start (same as Tract's -D flag)
    if compensate_delay && delay < output.len() {
        output.drain(..delay);
        println!("Delay compensated: trimmed first {} samples", delay);
    }

    // Write output
    let out_spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(output_path, out_spec)?;
    for sample in &output {
        let s = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(s)?;
    }
    writer.finalize()?;

    println!("Saved {} samples to {}", output.len(), output_path);

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
