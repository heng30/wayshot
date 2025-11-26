use hound::WavReader;
use mp4m::audio_processor::AudioProcessorConfigBuilder;
use mp4m::{AudioProcessor, OutputDestination, sample_rate};
use rand::Rng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let input_file1 = "data/speaker.wav";
    let input_file2 = "data/input.wav";
    let output_file = "data/tmp/two-audio-mixed.wav";

    log::debug!("Reading WAV files: {}, {}", input_file1, input_file2);

    let mut reader1 = WavReader::open(input_file1)?;
    let mut reader2 = WavReader::open(input_file2)?;

    let spec1 = reader1.spec();
    let spec2 = reader2.spec();

    log::debug!("Audio specs - File 1: {:?}", spec1);
    log::debug!("Audio specs - File 2: {:?}", spec2);

    let all_samples1: Vec<f32> = reader1.samples::<f32>().collect::<Result<Vec<f32>, _>>()?;
    let all_samples2: Vec<i16> = reader2.samples::<i16>().collect::<Result<Vec<i16>, _>>()?;

    log::debug!("Total samples - File 1: {}", all_samples1.len());
    log::debug!("Total samples - File 2: {}", all_samples2.len());

    let max_samples = all_samples1.len().max(all_samples2.len());
    log::debug!("Max samples across files: {}", max_samples);

    let samples_per_ms = (spec1.sample_rate as f32 / 1000.0) as usize;
    let min_chunk_samples = (500.0 * samples_per_ms as f32) as usize;
    let max_chunk_samples = (1000.0 * samples_per_ms as f32) as usize;

    log::debug!("Samples per ms: {}", samples_per_ms);
    log::debug!("Min chunk samples (500ms): {}", min_chunk_samples);
    log::debug!("Max chunk samples (1000ms): {}", max_chunk_samples);

    let config = AudioProcessorConfigBuilder::default()
        .target_sample_rate(sample_rate::CD)
        .convert_to_mono(true)
        .output_destination(Some(OutputDestination::<f32>::File(output_file.into())))
        .build()?;

    let mut processor = AudioProcessor::new(config);
    let sender1 = processor.add_track(spec1);
    let sender2 = processor.add_track(spec2);

    let mut rng = rand::rng();
    let mut processed_samples = 0;

    while processed_samples < max_samples {
        let remaining_samples = max_samples - processed_samples;

        let chunk_size = if remaining_samples < min_chunk_samples {
            remaining_samples
        } else {
            rng.random_range(min_chunk_samples..=max_chunk_samples.min(remaining_samples))
        };

        let chunk1 = if processed_samples < all_samples1.len() {
            let end = (processed_samples + chunk_size).min(all_samples1.len());
            &all_samples1[processed_samples..end]
        } else {
            &[]
        };

        let chunk2 = if processed_samples < all_samples2.len() {
            let end = (processed_samples + chunk_size).min(all_samples2.len());
            &all_samples2[processed_samples..end]
        } else {
            &[]
        };

        log::debug!(
            "Processing chunk: {} samples ({} ms) - Tracks: {}/{}",
            chunk_size,
            chunk_size as f32 / samples_per_ms as f32,
            chunk1.len(),
            chunk2.len()
        );

        if !chunk1.is_empty() {
            sender1.send(chunk1.to_vec())?;
        }
        if !chunk2.is_empty() {
            sender2.send(chunk2.iter().map(|i| *i as f32).collect())?;
        }

        processor.process_samples()?;

        processed_samples += chunk_size;

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    processor.flush()?;

    log::debug!("Two-track audio mixing completed!");
    log::debug!("Mixed output saved to: {}", output_file);

    Ok(())
}
