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

    let samples1_per_ms = (spec1.sample_rate as f32 * spec1.channels as f32 / 1000.0) as usize;
    let samples2_per_ms = (spec2.sample_rate as f32 * spec2.channels as f32 / 1000.0) as usize;
    let min_chunk_ms = 10;
    let max_chunk_ms = 10;

    log::debug!("Samples1 per ms: {}", samples1_per_ms);
    log::debug!("Samples2 per ms: {}", samples2_per_ms);
    log::debug!("Min chunk ms: {}", min_chunk_ms);
    log::debug!("Max chunk ms: {}", max_chunk_ms);

    let config = AudioProcessorConfigBuilder::default()
        .target_sample_rate(sample_rate::CD)
        .convert_to_mono(true)
        .output_destination(Some(OutputDestination::<f32>::File(output_file.into())))
        .build()?;

    let mut processor = AudioProcessor::new(config);
    let sender1 = processor.add_track(spec1);
    let sender2 = processor.add_track(spec2);

    let mut rng = rand::rng();
    let mut processed_samples1 = 0;
    let mut processed_samples2 = 0;

    while processed_samples1 < all_samples1.len() || processed_samples2 < all_samples2.len() {
        let chunk_ms = rng.random_range(min_chunk_ms..=max_chunk_ms);

        // Calculate chunk size for each track based on their sample rates
        let chunk1_size = chunk_ms * samples1_per_ms;
        let chunk2_size = chunk_ms * samples2_per_ms;

        let chunk1 = if processed_samples1 < all_samples1.len() {
            let end = (processed_samples1 + chunk1_size).min(all_samples1.len());
            &all_samples1[processed_samples1..end]
        } else {
            &[]
        };

        let chunk2 = if processed_samples2 < all_samples2.len() {
            let end = (processed_samples2 + chunk2_size).min(all_samples2.len());
            &all_samples2[processed_samples2..end]
        } else {
            &[]
        };

        log::debug!(
            "Processing chunk: {} ms - Track1: {} samples, Track2: {} samples",
            chunk_ms,
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

        processed_samples1 += chunk1_size;
        processed_samples2 += chunk2_size;
    }

    processor.flush()?;

    log::debug!("Two-track audio mixing completed!");
    log::debug!("Mixed output saved to: {}", output_file);

    Ok(())
}
