use audio_utils::audio::load_audio_file;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().collect();
    let audio_path = if args.len() > 1 {
        args[1].clone()
    } else {
        "data/test.mp4".to_string()
    };

    log::debug!("Loading audio file: {}", audio_path);
    log::debug!("{}", "=".repeat(60));

    let audio_config = load_audio_file(&audio_path)?;

    log::debug!("📊 Audio Information:");
    log::debug!("  Sample Rate:   {} Hz", audio_config.sample_rate);
    log::debug!("  Channels:      {}", audio_config.channel);
    log::debug!(
        "  Duration:      {:.2} seconds",
        audio_config.duration.as_secs_f64()
    );
    log::debug!(
        "  Total Samples: {}",
        audio_config.samples.len() / audio_config.channel as usize
    );

    if !audio_config.samples.is_empty() {
        let min_sample = audio_config
            .samples
            .iter()
            .cloned()
            .reduce(f32::min)
            .unwrap();
        let max_sample = audio_config
            .samples
            .iter()
            .cloned()
            .reduce(f32::max)
            .unwrap();
        let avg_sample: f32 =
            audio_config.samples.iter().sum::<f32>() / audio_config.samples.len() as f32;

        log::debug!("\n📈 Sample Statistics:");
        log::debug!("  Min Value:  {:.6}", min_sample);
        log::debug!("  Max Value:  {:.6}", max_sample);
        log::debug!("  Avg Value:  {:.6}", avg_sample);
        log::debug!(
            "  Peak Amplitude: {:.6} dB",
            20.0 * max_sample.abs().log10()
        );
    }

    // Print first few samples for debugging
    if audio_config.samples.len() > 10 {
        let preview_len = 10.min(audio_config.samples.len() / audio_config.channel as usize);
        log::debug!("\n🔍 First {} samples (per channel):", preview_len);
        for i in 0..audio_config.channel as usize {
            log::debug!("  Channel {}: ", i);
            for j in 0..preview_len {
                let idx = j * audio_config.channel as usize + i;
                if idx < audio_config.samples.len() {
                    log::debug!("{:.4} ", audio_config.samples[idx]);
                }
            }
        }
    }

    log::debug!("✅ Audio loaded successfully!");
    Ok(())
}
