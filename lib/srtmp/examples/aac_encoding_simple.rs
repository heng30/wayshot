use srtmp::{AacEncoder, AacEncoderConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = AacEncoderConfig::new(44100, 2)?;
    println!(
        "AAC encoder config: {}Hz, {} channels",
        config.sample_rate, config.channels
    );

    let mut encoder = AacEncoder::new(config)?;

    let sample_rate = 44100;
    let frequency = 440.0;
    let frame_samples = 1024; // AAC frame size (samples per channel)
    let mut pcm_data = Vec::with_capacity(frame_samples * 2);

    for i in 0..frame_samples {
        let t = i as f32 / sample_rate as f32;
        let sample_value = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.3;
        pcm_data.push(sample_value);
        pcm_data.push(sample_value);
    }

    println!("Generated {} PCM samples (stereo)", pcm_data.len());

    // Encode PCM to AAC
    let aac_data = encoder.encode(&pcm_data)?;
    println!("Encoded to {} bytes of AAC data", aac_data.len());
    println!(
        "Compression ratio: {:.2}%",
        (aac_data.len() * 100) / (pcm_data.len() * 4)
    );

    println!(
        "Input frame size: {} samples per channel",
        encoder.input_frame_size()
    );
    println!("Channels: {}", encoder.channels());

    Ok(())
}
