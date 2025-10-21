use hound::WavReader;
use recorder::{DenoiseError, RealTimeDenoise};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    log::info!("Starting real-time denoise demo...");
    log::info!("Press Ctrl-C to stop demo.");

    let stop_sig = Arc::new(AtomicBool::new(false));
    let stop_sig_clone = stop_sig.clone();
    ctrlc::set_handler(move || {
        log::info!("Ctrl-C received, stopping demo...");
        stop_sig_clone.store(true, Ordering::Relaxed);
    })?;

    // Read input file to get audio specification
    let reader = WavReader::open("target/input.wav")?;
    let spec = reader.spec();

    log::info!("Audio format:");
    log::info!("  Sample rate: {} Hz", spec.sample_rate);
    log::info!("  Channels: {}", spec.channels);
    log::info!("  Bits per sample: {}", spec.bits_per_sample);
    log::info!("  Sample format: {:?}", spec.sample_format);

    // Create real-time denoiser
    let model = RealTimeDenoise::model();
    let mut denoiser = RealTimeDenoise::new(&model, spec)?;

    log::info!("Real-time denoiser created successfully");

    // Process audio in chunks to simulate real-time processing
    let mut total_processed = 0;
    let mut total_output = 0;

    // Read samples from input file
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => match spec.bits_per_sample {
            16 => reader
                .into_samples::<i16>()
                .map(|s| s.map(|v| v as f32))
                .collect::<Result<Vec<_>, _>>()?,
            24 | 32 => reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32))
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(Box::new(DenoiseError::UnsupportedBitDepth(
                    spec.bits_per_sample,
                )));
            }
        },
    };

    log::info!("Total samples to process: {}", samples.len());

    // Process audio in chunks
    let chunk_size = 1024; // Process 1024 samples at a time
    let mut processed_samples = Vec::new();

    let now = std::time::Instant::now();

    for chunk in samples.chunks(chunk_size) {
        if stop_sig.load(Ordering::Relaxed) {
            log::info!("Stopping demo...");
            break;
        }

        total_processed += chunk.len();

        // Process the chunk
        if let Some(denoised) = denoiser.process_frame(chunk)? {
            total_output += denoised.len();
            processed_samples.extend(denoised);

            log::debug!(
                "Processed: {} samples, Output: {} samples, Buffer: {} samples",
                total_processed,
                total_output,
                denoiser.buffered_samples()
            );
        } else {
            log::debug!(
                "Buffered: {} samples (waiting for full frame)",
                denoiser.buffered_samples()
            );
        }
    }

    // Process any remaining buffered samples
    if let Some(remained) = denoiser.flush() {
        log::info!("Processing remaining buffered samples: {}", remained.len());

        total_output += remained.len();
        processed_samples.extend(remained);
    }

    let elapsed = now.elapsed();

    log::info!("Real-time denoising completed!");
    log::info!(
        "Input samples: {}, Output samples: {}, Buffer remaining: {}",
        total_processed,
        total_output,
        denoiser.buffered_samples()
    );
    log::info!(
        "Processing time: {:.2?}, Throughput: {:.2} samples/sec",
        elapsed,
        total_processed as f64 / elapsed.as_secs_f64()
    );

    // Save processed audio to file (optional)
    if !processed_samples.is_empty() {
        log::info!("Saving processed audio to target/output_real_time.wav...");

        let mut writer = hound::WavWriter::create("target/output_real_time.wav", spec)?;
        for sample in processed_samples {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;

        log::info!("Output file saved: target/output_real_time.wav");
    }

    Ok(())
}

