use recorder::Denoise;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    log::info!("Starting to read audio file...");
    log::info!("Press Ctrl-C to stop recording.");

    let stop_sig = Arc::new(AtomicBool::new(false));
    let stop_sig_clone = stop_sig.clone();
    ctrlc::set_handler(move || {
        log::info!("Ctrl-C received, stopping recording...");
        stop_sig_clone.store(true, Ordering::Relaxed);
    })?;

    // Use streaming processing with progress callback
    log::info!("Using streaming processing with progress tracking...");
    let denoiser = Denoise::new("target/input.wav", "target/output.wav")?;

    let now = std::time::Instant::now();
    denoiser.run(
        stop_sig,
        Some(|progress: f32| {
            log::debug!("Processing progress: {:.0}%", progress * 100.0);
        }),
    )?;

    log::info!(
        "Streaming denoising completed! Output file: target/output.wav. Spent: {:.2?}",
        now.elapsed()
    );

    Ok(())
}
