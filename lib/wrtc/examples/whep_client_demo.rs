use anyhow::Result;
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tokio::sync::Notify;
use wrtc::client::WHEPClient;
use image::{ImageBuffer, Rgb};
use hound::{WavWriter, WavSpec};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    println!("WHEP Client Demo");
    println!("================");
    println!("Connecting to WHEP server at: http://localhost:9090");

    // Create output directory
    let output_dir = "/tmp/whep-client";
    if Path::new(output_dir).exists() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    let (client, mut video_rx, mut audio_rx) = WHEPClient::new();

    // Create counters and shared state
    let video_counter = Arc::new(Mutex::new(0u32));
    let audio_duration = Arc::new(Mutex::new(0f64));
    let audio_samples_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

    // Create a shared completion notifier
    let completion_notifier = Arc::new(Notify::new());
    let video_notifier = Arc::new(Notify::new());
    let audio_notifier = Arc::new(Notify::new());

    // Clone counters for video task
    let video_counter_clone = video_counter.clone();
    let output_dir_clone = output_dir.to_string();
    let video_notifier_clone = video_notifier.clone();

    // Spawn a task to handle incoming video frames
    let _video_task = tokio::spawn(async move {
        let mut frame_count = 0;
        while let Some((width, height, rgb_data)) = video_rx.recv().await {
            frame_count += 1;

            println!("Received video frame #{}: {}x{} ({} bytes)",
                frame_count, width, height, rgb_data.len());

            // Check RGB data for content
            let non_zero_count = rgb_data.iter().filter(|&&v| v > 0).count();
            let total_val: u32 = rgb_data.iter().map(|&v| v as u32).sum();
            let avg_val = total_val as f32 / rgb_data.len() as f32;
            println!("  Frame #{} RGB analysis: {} non-zero pixels, avg value: {:.2}", frame_count, non_zero_count, avg_val);

            // Save first 10 frames as PNG image files
            if frame_count <= 10 {
                let filename = format!("{}/frame_{:03}_{}x{}.png",
                    output_dir_clone, frame_count, width, height);

                // Convert RGB data to ImageBuffer and save as PNG
                match ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, rgb_data) {
                    Some(img_buffer) => {
                        match img_buffer.save(&filename) {
                            Ok(_) => {
                                println!("  Saved frame #{} to: {}", frame_count, filename);
                            }
                            Err(e) => eprintln!("Failed to save PNG frame #{}: {}", frame_count, e),
                        }
                    }
                    None => eprintln!("Failed to create image buffer for frame #{}", frame_count),
                }
            }

            // Update shared counter
            *video_counter_clone.lock().unwrap() = frame_count;

            // Stop after saving first 10 frames
            if frame_count >= 10 {
                println!("Successfully saved 10 video frames");
                // Notify that video task is complete
                video_notifier_clone.notify_one();
                break;
            }
        }

        if frame_count == 0 {
            println!("No video frames received");
        }
        println!("Video processing completed");
    });

    // Clone counters for audio task
    let audio_duration_clone = audio_duration.clone();
    let audio_samples_buffer_clone = audio_samples_buffer.clone();
    let output_dir_clone = output_dir.to_string();
    let audio_notifier_clone = audio_notifier.clone();

    // Spawn a task to handle incoming audio samples
    let audio_task = tokio::spawn(async move {
        let mut packet_count = 0;
        let _start_time = Instant::now();
        let _sample_rate = 48000u32; // Expected sample rate

        while let Some((received_sample_rate, samples)) = audio_rx.recv().await {
            packet_count += 1;
            let duration_ms = (samples.len() as f64 / 2.0 / received_sample_rate as f64) * 1000.0;

            println!("Received audio packet #{}: {} Hz, {} samples ({:.2}ms) [STEREO]",
                packet_count, received_sample_rate, samples.len(), duration_ms);

            // Accumulate audio data
            {
                let mut buffer = audio_samples_buffer_clone.lock().unwrap();
                let mut duration = audio_duration_clone.lock().unwrap();
                buffer.extend_from_slice(&samples);
                *duration += duration_ms / 1000.0; // Convert to seconds

                // Save first 10 seconds of audio as WAV
                if *duration >= 10.0 {
                    println!("Collected {:.2} seconds of audio, saving as WAV...", *duration);

                    let output_file = format!("{}/first_10_seconds_audio.wav", output_dir_clone);
                    let info_file = format!("{}/first_10_seconds_audio_info.txt", output_dir_clone);

                    // Create WAV spec
                    let spec = WavSpec {
                        channels: 2,
                        sample_rate: received_sample_rate,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    };

                    // Create WAV writer and write samples
                    match WavWriter::create(&output_file, spec) {
                        Ok(mut writer) => {
                            for &sample in buffer.iter() {
                                // Convert f32 sample to i16
                                let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                if let Err(e) = writer.write_sample(sample_i16) {
                                    eprintln!("Failed to write audio sample: {}", e);
                                    break;
                                }
                            }

                            if let Err(e) = writer.finalize() {
                                eprintln!("Failed to finalize WAV file: {}", e);
                            } else {
                                println!("Saved audio data to: {}", output_file);

                                // Save audio info
                                let total_samples_stereo = buffer.len() / 2; // f32 samples, convert to stereo frames
                                let info_content = format!(
                                    "Sample Rate: {} Hz\nChannels: 2\nDuration: {:.2} seconds\nTotal Stereo Samples: {}\nTotal f32 Samples: {}\nFormat: 16-bit PCM WAV",
                                    received_sample_rate,
                                    *duration,
                                    total_samples_stereo,
                                    buffer.len()
                                );

                                if let Err(e) = fs::write(&info_file, info_content) {
                                    eprintln!("Failed to write audio info file: {}", e);
                                }
                            }
                        }
                        Err(e) => eprintln!("Failed to create WAV file: {}", e),
                    }

                    // Notify that audio task is complete
                    audio_notifier_clone.notify_one();
                    break;
                }
            }
        }

        if packet_count == 0 {
            println!("No audio packets received");
        }
        println!("Audio processing completed");
    });

    // Start the WHEP client connection
    let server_url = "http://localhost:9090";
    let connect_task = tokio::spawn(async move {
        println!("Attempting to connect to WHEP server: {}", server_url);

        match client.connect(server_url).await {
            Ok(_) => {
                println!("WHEP client connection completed successfully");
            }
            Err(e) => {
                eprintln!("Failed to connect to WHEP server: {}", e);
                eprintln!("Make sure the WHEP server is running on {}", server_url);
                eprintln!("You can start it with: cargo run --example whep_server2_demo");
            }
        }
    });

    // Create a coordination task that waits for both video and audio completion
    let video_counter_monitor = video_counter.clone();
    let audio_duration_monitor = audio_duration.clone();
    let completion_notifier_clone = completion_notifier.clone();
    let video_notifier_monitor = video_notifier.clone();
    let audio_notifier_monitor = audio_notifier.clone();
    let coordinator_task = tokio::spawn(async move {
        let mut last_video = 0u32;
        let mut last_audio = 0f64;
        let mut video_done = false;
        let mut audio_done = false;

        loop {
            tokio::select! {
                _ = sleep(Duration::from_millis(1000)) => {
                    let current_video = *video_counter_monitor.lock().unwrap();
                    let current_audio = *audio_duration_monitor.lock().unwrap();

                    if current_video != last_video || current_audio != last_audio {
                        println!("Progress: {} video frames, {:.2} seconds audio",
                            current_video, current_audio);
                        last_video = current_video;
                        last_audio = current_audio;
                    }
                }
                _ = video_notifier_monitor.notified() => {
                    println!("Video task completed (10 frames)");
                    video_done = true;
                }
                _ = audio_notifier_monitor.notified() => {
                    println!("Audio task completed (10 seconds)");
                    audio_done = true;
                }
            }

            // Check if we're done
            if video_done && audio_done {
                println!("Both targets achieved: 10 frames and 10 seconds of audio");
                completion_notifier_clone.notify_one();
                break;
            }
        }
    });

    // Wait for tasks with improved coordination
    tokio::select! {
        _ = coordinator_task => {
            println!("Coordinator task completed - both targets achieved");
        }
        _ = audio_task => {
            println!("Audio task completed");
        }
        _ = connect_task => {
            println!("Connect task completed");
        }
        _ = sleep(Duration::from_secs(30)) => {
            println!("Test completed after 30 seconds timeout");
        }
    }

    // Final summary
    let final_video_count = *video_counter.lock().unwrap();
    let final_audio_duration = *audio_duration.lock().unwrap();

    println!("\n{}", "=".repeat(50));
    println!("WHEP Client Demo Summary");
    println!("=========================");
    println!("Total video frames received: {}", final_video_count);
    println!("Total audio duration received: {:.2} seconds", final_audio_duration);
    println!("Output directory: {}", output_dir);
    println!("Files created:");

    // List created files
    if let Ok(entries) = fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let size_str = if metadata.is_file() {
                    format!(" ({} bytes)", metadata.len())
                } else {
                    String::new()
                };
                println!("  - {}{}", entry.file_name().to_string_lossy(), size_str);
            }
        }
    }

    println!("\nPNG frames have been saved directly to: {}/frame_XXX_1920x1080.png", output_dir);
    println!("WAV audio has been saved directly to: {}/first_10_seconds_audio.wav", output_dir);

    println!("\nYou can view the PNG frames with any image viewer or play the WAV file with any audio player.");

    println!("\nDemo completed successfully!");
    Ok(())
}