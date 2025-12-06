use hound::WavReader;
use image::{ImageBuffer, Rgb};
use mp4m::mp4_processor::{
    AudioConfig, Mp4Processor, Mp4ProcessorConfigBuilder, VideoConfig, VideoFrameType,
};
use std::{path::PathBuf, thread, time::Duration};
use video_encoder::{EncodedFrame, VideoEncoderConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let audio_file = "data/speaker.wav";
    let output_file = "data/tmp/output.mp4";
    let (width, height) = (1920, 1080);
    let (fps, duration_seconds) = (25, 10);
    let total_frames = fps * duration_seconds;

    // Create red, green, blue images (RGB format)
    let red_frame = create_color_frame(width, height, 255, 0, 0);
    let green_frame = create_color_frame(width, height, 0, 255, 0);
    let blue_frame = create_color_frame(width, height, 0, 0, 255);

    // Read audio file metadata first
    let (audio_spec, audio_samples) = if let Ok(mut reader) = WavReader::open(audio_file) {
        let spec = reader.spec();
        log::debug!(
            "Audio file: {} channels, {} Hz, {} bits, format: {:?}",
            spec.channels,
            spec.sample_rate,
            spec.bits_per_sample,
            spec.sample_format
        );

        // Read audio samples as f32
        let samples = read_audio_samples_as_f32(&mut reader)?;
        (Some(spec), samples)
    } else {
        panic!("Warning: Could not read audio file {}", audio_file);
    };

    let spec = &audio_spec.unwrap();

    // Create video config
    let config = Mp4ProcessorConfigBuilder::default()
        .save_path(PathBuf::from(output_file))
        .video_config(VideoConfig { width, height, fps })
        .build()?;

    let mut processor = Mp4Processor::new(config);
    let video_sender = processor.h264_sender();

    let audio_sender = processor.add_audio_track(AudioConfig {
        convert_to_mono: false,
        spec: *spec,
    })?;

    // Process audio samples if available
    if let Some(spec) = audio_spec {
        log::debug!(
            "Audio file has {} samples, {} channels, {} Hz",
            audio_samples.len(),
            spec.channels,
            spec.sample_rate
        );

        // Use AAC-friendly frame size (1024 samples per channel)
        let aac_frame_size = 1024; // AAC typically uses 1024 samples per frame
        // let aac_frame_size = 1124 * 3; // AAC typically uses 1024 samples per frame
        let samples_per_frame = aac_frame_size * spec.channels as usize;

        log::debug!(
            "Using AAC frame size: {} samples per frame ({} per channel, original channels: {})",
            samples_per_frame,
            aac_frame_size,
            spec.channels
        );

        let mut total_sent = 0;
        for chunk in audio_samples.chunks(samples_per_frame) {
            // Ensure we have complete frames (multiple of channel count)
            if chunk.len() == samples_per_frame {
                // Use f32 samples directly for AAC encoding
                let f32_chunk: Vec<f32> = chunk.to_vec();

                if let Err(e) = audio_sender.send(f32_chunk) {
                    log::warn!("audio sender failed: {e}");
                    break;
                }
                total_sent += chunk.len();
            } else {
                log::debug!("Skipping incomplete audio frame: {} samples", chunk.len());
            }
        }
        log::debug!("Sent {} audio samples in total", total_sent);
    }

    let config = VideoEncoderConfig::new(width, height).with_fps(fps);
    let mut h264_encoder = video_encoder::new(config)?;
    let headers_data = h264_encoder.headers()?;

    let processor_thread = thread::spawn(move || {
        if let Err(e) = processor.run_processing_loop(Some(headers_data)) {
            log::warn!("MP4 processing error: {}", e);
        }
    });

    for frame_num in 0..total_frames {
        let img = match (frame_num / fps) % 3 {
            0 => &red_frame,
            1 => &green_frame,
            2 => &blue_frame,
            _ => &red_frame,
        };
        let encoded_frame = h264_encoder.encode_frame(img.clone())?;

        match encoded_frame {
            EncodedFrame::Frame((_, data)) => {
                if let Err(e) = video_sender.send(VideoFrameType::Frame(data)) {
                    log::warn!("video sender encoded frame failed: {e}");
                    break;
                }
            }
            _ => break,
        }
    }

    let video_sender_clone = video_sender.clone();
    if let Err(e) = h264_encoder.flush(Box::new(move |data| {
        if let Err(e) = video_sender_clone.send(VideoFrameType::Frame(data)) {
            log::warn!("video sender send flushed data failed: {e}");
        }
    })) {
        log::warn!("Failed to flush encoder frame: {:?}", e);
    }

    thread::sleep(Duration::from_secs(1));

    _ = video_sender.send(VideoFrameType::End);

    drop(video_sender);
    drop(audio_sender);

    if let Err(e) = processor_thread.join() {
        log::warn!("Processor thread error: {:?}", e);
    }

    log::info!("Video generated successfully: {}", output_file);
    Ok(())
}

fn create_color_frame(
    width: u32,
    height: u32,
    r: u8,
    g: u8,
    b: u8,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let mut frame: Vec<u8> = Vec::with_capacity((width * height * 3) as usize);

    for _ in 0..(width * height) {
        frame.push(r);
        frame.push(g);
        frame.push(b);
    }
    ImageBuffer::from_vec(width, height, frame).unwrap()
}

fn read_audio_samples_as_f32(
    reader: &mut WavReader<std::io::BufReader<std::fs::File>>,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let spec = reader.spec();

    match spec.sample_format {
        hound::SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => {
                    let samples: Vec<i16> =
                        reader.samples::<i16>().filter_map(Result::ok).collect();
                    // Convert i16 to normalized f32 [-1.0, 1.0]
                    Ok(samples
                        .into_iter()
                        .map(|s| s as f32 / i16::MAX as f32)
                        .collect())
                }
                24 => {
                    // Convert i32 (24-bit) to normalized f32 [-1.0, 1.0]
                    let samples: Vec<i32> =
                        reader.samples::<i32>().filter_map(Result::ok).collect();
                    Ok(samples
                        .into_iter()
                        .map(|s| s as f32 / ((1 << 23) - 1) as f32)
                        .collect())
                }
                32 => {
                    // Convert i32 to normalized f32 [-1.0, 1.0]
                    let samples: Vec<i32> =
                        reader.samples::<i32>().filter_map(Result::ok).collect();
                    Ok(samples
                        .into_iter()
                        .map(|s| s as f32 / i32::MAX as f32)
                        .collect())
                }
                _ => Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into()),
            }
        }
        hound::SampleFormat::Float => {
            // Float samples are already in the correct format
            Ok(reader.samples::<f32>().filter_map(Result::ok).collect())
        }
    }
}
