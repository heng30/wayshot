//! Example: Simulated real-time streaming with model selection
//!
//! Usage: cargo run --example realtime -- input.wav output.wav [model_dir] [--mode split|combined|stateless]

use deepfilter_rs::{DeepFilterProcessor, SessionMode, SAMPLE_RATE, HOP_SIZE, FFT_SIZE};
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

    println!("Loading model from {:?}...", model_dir);
    let mut processor = DeepFilterProcessor::with_mode(&model_dir, session_mode, Some(2))?;
    processor.warmup()?;
    let variant = processor.variant();
    println!("Using model variant: {}", variant.name());
    println!("Inference mode: {}", processor.inference_mode_name());

    // Read input audio
    let mut reader = hound::WavReader::open(input_path)?;
    let spec = reader.spec();
    println!("Input: {} Hz, {} channels, {:?}",
             spec.sample_rate, spec.channels, spec.sample_format);

    if spec.sample_rate != SAMPLE_RATE as u32 {
        eprintln!("Warning: Input sample rate {} != expected {}. Resample first!",
                  spec.sample_rate, SAMPLE_RATE);
    }

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
        samples.chunks(spec.channels as usize).map(|c| c[0]).collect()
    };

    let frame_duration = std::time::Duration::from_secs_f64(HOP_SIZE as f64 / SAMPLE_RATE as f64);
    println!("Processing {} samples ({:.2}s) with streaming... (budget: {:.2}ms/frame)",
             mono.len(), mono.len() as f32 / SAMPLE_RATE as f32,
             frame_duration.as_secs_f64() * 1000.0);

    // Simulate audio callback with small chunks
    let chunk_size = HOP_SIZE / 3;
    let mut out_samples: Vec<f32> = Vec::with_capacity(mono.len() + FFT_SIZE);
    let mut input_buf: Vec<f32> = Vec::with_capacity(HOP_SIZE);

    let mut frame_count: u64 = 0;
    let mut underrun_count: u64 = 0;
    let mut max_frame_time = std::time::Duration::ZERO;
    let mut total_frame_time = std::time::Duration::ZERO;
    let mut latencies_us: Vec<u64> = Vec::new();

    let start = std::time::Instant::now();

    for chunk in mono.chunks(chunk_size) {
        input_buf.extend_from_slice(chunk);
        while input_buf.len() >= HOP_SIZE {
            let frame_in: Vec<f32> = input_buf.drain(..HOP_SIZE).collect();
            let mut frame_out = vec![0.0f32; HOP_SIZE];

            let t0 = std::time::Instant::now();
            processor.process_frame(&frame_in, &mut frame_out)?;
            let dt = t0.elapsed();

            total_frame_time += dt;
            frame_count += 1;
            if dt > max_frame_time {
                max_frame_time = dt;
            }
            latencies_us.push(dt.as_micros() as u64);
            if dt > frame_duration {
                underrun_count += 1;
                eprintln!("UNDERRUN frame {}: {:.2}ms > {:.2}ms budget",
                          frame_count, dt.as_secs_f64() * 1000.0,
                          frame_duration.as_secs_f64() * 1000.0);
            }

            out_samples.extend_from_slice(&frame_out);
        }
    }

    // Flush remainder
    if !input_buf.is_empty() {
        input_buf.resize(HOP_SIZE, 0.0);
        let frame_in: Vec<f32> = input_buf.drain(..).collect();
        let mut frame_out = vec![0.0f32; HOP_SIZE];
        processor.process_frame(&frame_in, &mut frame_out)?;
        out_samples.extend_from_slice(&frame_out);
    }

    let elapsed = start.elapsed();
    let rtf = elapsed.as_secs_f32() / (mono.len() as f32 / SAMPLE_RATE as f32);
    let avg_ms = if frame_count > 0 {
        total_frame_time.as_secs_f64() * 1000.0 / frame_count as f64
    } else { 0.0 };
    println!("Done in {:.2}s (RTF: {:.3}x realtime)", elapsed.as_secs_f32(), rtf);
    println!("Frames: {}, avg: {:.2}ms, max: {:.2}ms, underruns: {}",
             frame_count, avg_ms, max_frame_time.as_secs_f64() * 1000.0, underrun_count);

    // Write output
    let out_spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output_path, out_spec)?;
    for sample in &out_samples {
        let s = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(s)?;
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
