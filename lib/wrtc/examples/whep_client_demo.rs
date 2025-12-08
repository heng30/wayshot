use anyhow::Result;
use crossbeam::channel::bounded;
use hound::{WavSpec, WavWriter};
use image::{ImageBuffer, Rgb};
use log::{info, trace, warn};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
use wrtc::client::{AudioSamples, RGBFrame, WHEPClient, WHEPClientConfig};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let server_url = "http://localhost:9090".to_string();

    info!("WHEP Client Demo");
    info!("================");
    info!("Connecting to WHEP server at: {server_url}");

    let output_dir = "/tmp/whep-client";
    if Path::new(output_dir).exists() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    let video_counter = Arc::new(Mutex::new(0u32));
    let audio_duration = Arc::new(Mutex::new(0f64));
    let audio_samples_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let (video_tx, video_rx) = bounded::<RGBFrame>(1024);
    let (audio_tx, audio_rx) = bounded::<AudioSamples>(1024);

    let client = WHEPClient::new(
        WHEPClientConfig::new(server_url).with_auth_token("123".to_string()),
        Some(video_tx),
        Some(audio_tx),
    )
    .await?;

    let video_counter_clone = video_counter.clone();
    let output_dir_clone = output_dir.to_string();

    tokio::spawn(async move {
        let mut frame_count = 0;
        while let Ok((width, height, rgb_data)) = video_rx.recv() {
            frame_count += 1;

            trace!(
                "Received video frame #{}: {}x{} ({} bytes)",
                frame_count,
                width,
                height,
                rgb_data.len()
            );

            if frame_count <= 10 {
                let filename = format!(
                    "{}/frame_{:03}_{}x{}.png",
                    output_dir_clone, frame_count, width, height
                );

                match ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, rgb_data) {
                    Some(img_buffer) => match img_buffer.save(&filename) {
                        Ok(_) => info!("  Saved frame #{} to: {}", frame_count, filename),
                        Err(e) => warn!("Failed to save PNG frame #{}: {}", frame_count, e),
                    },
                    None => warn!("Failed to create image buffer for frame #{}", frame_count),
                }
            }

            *video_counter_clone.lock().unwrap() = frame_count;
        }

        info!("Video receive thread exit...");
    });

    let audio_duration_clone = audio_duration.clone();
    let output_dir_clone = output_dir.to_string();
    let audio_info = client.media_info.audio.clone();

    let audio_task = tokio::spawn(async move {
        let mut packet_count = 0;

        while let Ok(samples) = audio_rx.recv() {
            packet_count += 1;
            let duration_ms =
                (samples.len() as f64 / audio_info.channels as f64 / audio_info.sample_rate as f64)
                    * 1000.0;

            trace!(
                "Received audio packet #{}: {} Hz, {} samples ({:.2}ms) [STEREO]",
                packet_count,
                audio_info.sample_rate,
                samples.len(),
                duration_ms
            );

            let mut buffer = audio_samples_buffer.lock().unwrap();
            let mut duration = audio_duration_clone.lock().unwrap();
            buffer.extend_from_slice(&samples);
            *duration += duration_ms / 1000.0;

            if *duration >= 10.0 {
                info!(
                    "Collected {:.2} seconds of audio, saving as WAV...",
                    *duration
                );

                let output_file = format!("{}/first_10_seconds_audio.wav", output_dir_clone);
                let spec = WavSpec {
                    channels: audio_info.channels,
                    sample_rate: audio_info.sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };

                match WavWriter::create(&output_file, spec) {
                    Ok(mut writer) => {
                        for &sample in buffer.iter() {
                            let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            if let Err(e) = writer.write_sample(sample_i16) {
                                warn!("Failed to write audio sample: {}", e);
                                break;
                            }
                        }

                        if let Err(e) = writer.finalize() {
                            warn!("Failed to finalize WAV file: {}", e);
                        }
                    }
                    Err(e) => warn!("Failed to create WAV file: {}", e),
                }
                break;
            }
        }

        info!("Audio receive thread exit...");
    });

    let connect_task = tokio::spawn(async move {
        info!("Attempting to connect to WHEP server");

        match client.connect().await {
            Ok(_) => info!("WHEP client connection completed successfully"),
            Err(e) => warn!("Failed to connect to WHEP server: {e}"),
        }
    });

    tokio::select! {
        _ = audio_task => {
            info!("Audio task completed");
        }
        _ = connect_task => {
            info!("Connect task completed");
        }
        _ = sleep(Duration::from_secs(15)) => {
            warn!("Test completed after 15 seconds timeout");
        }
    }

    let final_video_count = *video_counter.lock().unwrap();
    let final_audio_duration = *audio_duration.lock().unwrap();

    info!("=========================");
    info!("WHEP Client Demo Summary");
    info!("=========================");
    info!("Total video frames received: {}", final_video_count);
    info!(
        "Total audio duration received: {:.2} seconds",
        final_audio_duration
    );
    info!("Output directory: {}", output_dir);
    info!("Files created:");

    if let Ok(entries) = fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let size_str = if metadata.is_file() {
                    format!(" ({} bytes)", metadata.len())
                } else {
                    String::new()
                };
                info!("  - {}{}", entry.file_name().to_string_lossy(), size_str);
            }
        }
    }

    info!("Demo completed successfully!");
    Ok(())
}
