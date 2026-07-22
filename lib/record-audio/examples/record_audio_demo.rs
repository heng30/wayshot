use record_audio::AudioRecorder;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Handle Ctrl+C to stop recording
    ctrlc::set_handler(move || {
        log::info!("Received interrupt signal, stopping recording...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Create audio recorder
    let mut recorder = AudioRecorder::new();

    // List available input devices
    log::info!("=== Available Input Devices ===");
    let devices = recorder.get_input_devices()?;
    for (i, device) in devices.iter().enumerate() {
        log::info!(
            "  [{}] {} ({} ch, {} Hz)",
            i,
            device.name,
            device.channels,
            device.sample_rate
        );
    }

    // Select device (use None for default, or specify device name)
    let selected_device: Option<&str> = None;
    log::info!("\n=== Starting Recording ===");
    if let Some(name) = selected_device {
        log::info!("Using device: {}", name);
    } else {
        log::info!("Using default input device");
    }

    // Start recording
    recorder.start_recording(selected_device)?;
    log::info!("Recording started... Press Ctrl+C to stop");

    // Monitor recording progress
    let start = std::time::Instant::now();
    let mut last_log = std::time::Instant::now();

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));

        // Log progress every second
        if last_log.elapsed() >= Duration::from_secs(1) {
            let duration = recorder.recorded_duration_secs();
            let sample_count = recorder.recorded_sample_count();
            log::info!("Recording: {:.1}s ({} samples)", duration, sample_count);
            last_log = std::time::Instant::now();
        }
    }

    // Stop recording
    log::info!("\n=== Stopping Recording ===");
    let recorded_audio = recorder.stop_recording()?;

    let elapsed = start.elapsed();
    log::info!("Recording duration: {:.2}s", elapsed.as_secs_f64());
    log::info!("Sample count: {}", recorded_audio.samples.len());
    log::info!("Channels: {}", recorded_audio.channels);
    log::info!("Sample rate: {} Hz", recorded_audio.sample_rate);
    log::info!("Duration: {:.2}s", recorded_audio.duration_secs());

    // Calculate audio level
    if let Some(db) = recorded_audio.rms_level_db() {
        log::info!("RMS level: {:.1} dB", db);
    }

    // Save to file
    let output_path = PathBuf::from("tmp/recorded_audio.wav");
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    recorded_audio.save_to_file(&output_path)?;
    log::info!("Saved to: {}", output_path.display());

    Ok(())
}
