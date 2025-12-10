use hound::WavReader;
use mp4m::audio_processor::AudioProcessorConfigBuilder;
use mp4m::{AudioProcessor, OutputDestination, sample_rate};
use rand::Rng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let input_file = "data/speaker.wav";
    let output_file = "data/tmp/one-audio-mixed.wav";

    log::debug!("Reading WAV file: {}", input_file);

    let mut reader = WavReader::open(input_file)?;
    let spec = reader.spec();

    log::debug!("Audio specs: {:?}", spec);
    log::debug!("Sample rate: {} Hz", spec.sample_rate);
    log::debug!("Channels: {}", spec.channels);
    log::debug!("Bits per sample: {}", spec.bits_per_sample);

    let all_samples: Vec<f32> = reader.samples::<f32>().collect::<Result<Vec<f32>, _>>()?;
    log::debug!("Total samples: {}", all_samples.len());

    let samples_per_ms = (spec.sample_rate as f32 / 1000.0) as usize;
    let min_chunk_samples = (10.0 * samples_per_ms as f32) as usize;
    let max_chunk_samples = (20.0 * samples_per_ms as f32) as usize;

    log::debug!("Samples per ms: {}", samples_per_ms);
    log::debug!("Min chunk samples (500ms): {}", min_chunk_samples);
    log::debug!("Max chunk samples (1000ms): {}", max_chunk_samples);

    let config = AudioProcessorConfigBuilder::default()
        .target_sample_rate(sample_rate::PROFESSIONAL)
        .convert_to_mono(false)
        .output_destination(Some(OutputDestination::<f32>::File(output_file.into())))
        .build()?;

    let mut processor = AudioProcessor::new(config);
    let sender = processor.add_track(spec);

    let mut rng = rand::rng();
    let mut processed_samples = 0;

    while processed_samples < all_samples.len() {
        let remaining_samples = all_samples.len() - processed_samples;

        let chunk_size = if remaining_samples < min_chunk_samples {
            remaining_samples
        } else {
            rng.random_range(min_chunk_samples..=max_chunk_samples.min(remaining_samples))
        };

        let chunk = &all_samples[processed_samples..processed_samples + chunk_size];

        log::debug!(
            "Processing chunk: {} samples ({} ms)",
            chunk_size,
            chunk_size as f32 / samples_per_ms as f32
        );

        sender.send(chunk.to_vec())?;
        processor.process_samples()?;

        processed_samples += chunk_size;
    }

    processor.flush()?;

    log::debug!("Audio processing completed!");
    log::debug!("Output saved to: {}", output_file);

    Ok(())
}
