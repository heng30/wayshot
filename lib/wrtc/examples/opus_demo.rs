use hound::{WavReader, WavSpec, WavWriter};
use log::{debug, error, info, warn};
use std::path::Path;
use wrtc::opus::{OpusCoder, OpusCoderError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let input_path = Path::new("data/test-44100.wav");
    let output_path = "/tmp/opus-coder-44100.wav";

    info!("Opus Codec Demo");
    info!("===============");
    info!("Input file: {:?}", input_path);
    info!("Output file: {}", output_path);

    // 1. Read WAV file
    info!("Reading WAV file...");
    let (audio_data, spec) = read_wav_file(input_path)?;
    info!("  Sample rate: {} Hz", spec.sample_rate);
    info!("  Channels: {}", spec.channels);
    let duration = audio_data.len() as f32 / (spec.sample_rate as f32 * spec.channels as f32);
    info!("  Duration: {:.2} seconds", duration);
    info!("  Total samples: {}", audio_data.len());

    // 2. Initialize Opus coder
    info!("Initializing Opus coder...");
    let channels = if spec.channels == 1 {
        audiopus::Channels::Mono
    } else if spec.channels == 2 {
        audiopus::Channels::Stereo
    } else {
        error!(
            "Only mono and stereo audio are supported, got {} channels",
            spec.channels
        );
        return Err("Only mono and stereo audio are supported".into());
    };

    let mut opus_encoder = OpusCoder::new(spec.sample_rate, channels)?;
    let mut opus_decoder = OpusCoder::new(spec.sample_rate, channels)?;
    info!(
        "  Internal frame size: {} samples (48kHz)",
        opus_encoder.frame_size()
    );
    info!(
        "  Input frame size: {} samples ({}Hz)",
        (spec.sample_rate as usize * 20) / 1000,
        spec.sample_rate
    );

    // 3. Encode audio data
    info!("Encoding audio with Opus...");
    let encoded_packets = encode_audio(&mut opus_encoder, &audio_data)?;
    info!("  Encoded {} frames", encoded_packets.len());

    // 4. Decode audio data
    info!("Decoding audio from Opus...");
    let decoded_audio = decode_audio(&mut opus_decoder, &encoded_packets)?;
    info!("  Decoded {} samples", decoded_audio.len());

    // 5. Write decoded audio to WAV file
    info!("Writing decoded audio to WAV file at 48kHz...");

    // Opus always outputs at 48kHz, so we save at 48kHz regardless of input sample rate
    let mut output_spec = spec;
    output_spec.sample_rate = 48000;

    write_wav_file(output_path, &decoded_audio, output_spec)?;
    info!("  Output written to: {}", output_path);
    info!("  Output sample rate: 48000 Hz (Opus native rate)");

    info!("Demo completed successfully!");

    // Calculate and display compression ratio
    let original_size = audio_data.len() * std::mem::size_of::<f32>();
    let compressed_size: usize = encoded_packets.iter().map(|packet| packet.len()).sum();
    let compression_ratio = original_size as f32 / compressed_size as f32;
    info!("Original size: {} bytes", original_size);
    info!("Compressed size: {} bytes", compressed_size);
    info!("Compression ratio: {:.2}:1", compression_ratio);

    Ok(())
}

fn read_wav_file(path: &Path) -> Result<(Vec<f32>, WavSpec), Box<dyn std::error::Error>> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| match s {
            Ok(sample) => sample as f32 / 32768.0,
            Err(e) => {
                warn!("Failed to read sample: {}", e);
                0.0
            }
        })
        .collect();

    Ok((samples, spec))
}

fn encode_audio(
    opus_coder: &mut OpusCoder,
    audio_data: &[f32],
) -> Result<Vec<Vec<u8>>, OpusCoderError> {
    let mut encoded_packets = Vec::new();
    let samples_per_frame = opus_coder.input_samples_per_frame();
    let total_frames = (audio_data.len() + samples_per_frame - 1) / samples_per_frame;

    debug!(
        "Encoding {} frames from {} samples",
        total_frames,
        audio_data.len()
    );

    for (frame_idx, chunk) in audio_data.chunks(samples_per_frame).enumerate() {
        let mut frame = vec![0.0f32; samples_per_frame];
        frame[..chunk.len()].copy_from_slice(chunk);

        match opus_coder.encode(&frame) {
            Ok(packet) => {
                let packet_size = packet.len();
                encoded_packets.push(packet);
                debug!(
                    "Encoded frame {}/{}: {} bytes",
                    frame_idx + 1,
                    total_frames,
                    packet_size
                );
            }
            Err(e) => {
                warn!("Encoding frame {} failed: {}", frame_idx + 1, e);
                encoded_packets.push(Vec::new());
            }
        }
    }

    Ok(encoded_packets)
}

fn decode_audio(
    opus_coder: &mut OpusCoder,
    encoded_packets: &[Vec<u8>],
) -> Result<Vec<f32>, OpusCoderError> {
    let mut decoded_audio = Vec::new();
    let total_packets = encoded_packets.len();

    debug!("Decoding {} packets", total_packets);

    for (packet_idx, packet) in encoded_packets.iter().enumerate() {
        if packet.is_empty() {
            let silence_frame = vec![0.0f32; opus_coder.samples_per_frame()];
            decoded_audio.extend_from_slice(&silence_frame);
            debug!(
                "Silence packet {}/{}: {} samples",
                packet_idx + 1,
                total_packets,
                silence_frame.len()
            );
        } else {
            match opus_coder.decode(packet) {
                Ok(frame) => {
                    decoded_audio.extend_from_slice(&frame);
                    debug!(
                        "Decoded packet {}/{}: {} samples",
                        packet_idx + 1,
                        total_packets,
                        frame.len()
                    );
                }
                Err(e) => {
                    warn!("Decoding packet {} failed: {}", packet_idx + 1, e);
                    let silence_frame = vec![0.0f32; opus_coder.samples_per_frame()];
                    decoded_audio.extend_from_slice(&silence_frame);
                }
            }
        }
    }

    Ok(decoded_audio)
}

fn write_wav_file(
    path: &str,
    audio_data: &[f32],
    spec: WavSpec,
) -> Result<(), Box<dyn std::error::Error>> {
    let writer_spec = WavSpec {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, writer_spec)?;

    for &sample in audio_data {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_sample(i16_sample)?;
    }

    writer.finalize()?;
    Ok(())
}
