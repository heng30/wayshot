use hound::WavReader;
use mp4m::{AudioProcessor, AudioProcessorConfigBuilder, OutputDestination, sample_rate};
use rand::Rng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let input_file1 = "data/speaker-mono.wav";
    let input_file2 = "data/input.wav";
    let output_file = "data/tmp/diff-audio-mixed.wav";

    log::debug!("Reading WAV files: {}, {}", input_file1, input_file2);

    let mut reader1 = WavReader::open(input_file1)?;
    let mut reader2 = WavReader::open(input_file2)?;

    let spec1 = reader1.spec();
    let spec2 = reader2.spec();

    log::debug!("Audio specs - File 1: {:?}", spec1);
    log::debug!("Audio specs - File 2: {:?}", spec2);

    let all_samples1: Vec<f32> = match spec1.sample_format {
        hound::SampleFormat::Float => reader1.samples::<f32>().collect::<Result<Vec<f32>, _>>()?,
        hound::SampleFormat::Int => reader1
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32))
            .collect::<Result<Vec<f32>, _>>()?,
    };
    let all_samples2: Vec<f32> = match spec2.sample_format {
        hound::SampleFormat::Float => reader2.samples::<f32>().collect::<Result<Vec<f32>, _>>()?,
        hound::SampleFormat::Int => reader2
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32))
            .collect::<Result<Vec<f32>, _>>()?,
    };

    log::debug!("Total samples - File 1: {}", all_samples1.len());
    log::debug!("Total samples - File 2: {}", all_samples2.len());

    let max_samples = all_samples1.len().max(all_samples2.len());
    log::debug!("Max samples across files: {}", max_samples);

    let samples_per_ms1 = (spec1.sample_rate as f32 / 1000.0) as usize * spec1.channels as usize;
    let samples_per_ms2 = (spec2.sample_rate as f32 / 1000.0) as usize * spec2.channels as usize;

    let min_chunk_samples1 = (500.0 * samples_per_ms1 as f32) as usize;
    let max_chunk_samples1 = (1000.0 * samples_per_ms1 as f32) as usize;
    let min_chunk_samples2 = (500.0 * samples_per_ms2 as f32) as usize;
    let max_chunk_samples2 = (1000.0 * samples_per_ms2 as f32) as usize;

    log::debug!(
        "Track 1 - Samples per ms: {} ({} channels)",
        samples_per_ms1,
        spec1.channels
    );
    log::debug!(
        "Track 1 - Min chunk samples (500ms): {}",
        min_chunk_samples1
    );
    log::debug!(
        "Track 1 - Max chunk samples (1000ms): {}",
        max_chunk_samples1
    );
    log::debug!(
        "Track 2 - Samples per ms: {} ({} channels)",
        samples_per_ms2,
        spec2.channels
    );
    log::debug!(
        "Track 2 - Min chunk samples (500ms): {}",
        min_chunk_samples2
    );
    log::debug!(
        "Track 2 - Max chunk samples (1000ms): {}",
        max_chunk_samples2
    );

    let config = AudioProcessorConfigBuilder::default()
        .target_sample_rate(sample_rate::CD)
        .convert_to_mono(true)
        .output_destination(Some(OutputDestination::File(output_file.into())))
        .build()?;

    let mut processor = AudioProcessor::new(config);
    let sender1 = processor.add_track(spec1);
    let sender2 = processor.add_track(spec2);

    let mut rng = rand::rng();
    let mut processed_samples1 = 0;
    let mut processed_samples2 = 0;

    while processed_samples1 < all_samples1.len() || processed_samples2 < all_samples2.len() {
        let remaining_samples1 = all_samples1.len() - processed_samples1;
        let remaining_samples2 = all_samples2.len() - processed_samples2;

        let chunk_size1 = if remaining_samples1 == 0 {
            0
        } else if remaining_samples1 < min_chunk_samples1 {
            remaining_samples1
        } else {
            rng.random_range(min_chunk_samples1..=max_chunk_samples1.min(remaining_samples1))
        };

        let chunk_size2 = if remaining_samples2 == 0 {
            0
        } else if remaining_samples2 < min_chunk_samples2 {
            remaining_samples2
        } else {
            rng.random_range(min_chunk_samples2..=max_chunk_samples2.min(remaining_samples2))
        };

        let chunk1 = if chunk_size1 > 0 {
            let end = processed_samples1 + chunk_size1;
            &all_samples1[processed_samples1..end]
        } else {
            &[]
        };

        let chunk2 = if chunk_size2 > 0 {
            let end = processed_samples2 + chunk_size2;
            &all_samples2[processed_samples2..end]
        } else {
            &[]
        };

        log::debug!(
            "Processing chunks - Track1: {} samples, Track2: {} samples",
            chunk1.len(),
            chunk2.len()
        );

        if !chunk1.is_empty() {
            sender1.send(chunk1.to_vec())?;
            processed_samples1 += chunk_size1;
        }
        if !chunk2.is_empty() {
            sender2.send(chunk2.to_vec())?;
            processed_samples2 += chunk_size2;
        }

        processor.process_samples()?;

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    processor.flush()?;

    log::debug!("Two-track audio mixing completed!");
    log::debug!("Mixed output saved to: {}", output_file);

    Ok(())
}
