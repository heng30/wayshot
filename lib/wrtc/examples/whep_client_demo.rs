use anyhow::Result;
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::time::sleep;
use wrtc::client::WHEPClient;

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

    // Clone counters for video task
    let video_counter_clone = video_counter.clone();
    let output_dir_clone = output_dir.to_string();

    // Spawn a task to handle incoming video frames
    let video_task = tokio::spawn(async move {
        let mut frame_count = 0;
        while let Some((width, height, rgb_data)) = video_rx.recv().await {
            frame_count += 1;

            println!("Received video frame #{}: {}x{} ({} bytes)",
                frame_count, width, height, rgb_data.len());

            // Save first 10 frames as image files
            if frame_count <= 10 {
                let filename = format!("{}/frame_{:03}_{}x{}.rgb",
                    output_dir_clone, frame_count, width, height);

                match fs::write(&filename, &rgb_data) {
                    Ok(_) => {
                        println!("  Saved frame #{} to: {}", frame_count, filename);

                        // Also create a simple header file with dimensions
                        let header_filename = format!("{}/frame_{:03}_{}x{}.txt",
                            output_dir_clone, frame_count, width, height);
                        let header_content = format!("Width: {}\nHeight: {}\nData length: {}\n",
                            width, height, rgb_data.len());

                        if let Err(e) = fs::write(&header_filename, header_content) {
                            eprintln!("Failed to write header file: {}", e);
                        }
                    }
                    Err(e) => eprintln!("Failed to save frame #{}: {}", frame_count, e),
                }
            }

            // Update shared counter
            *video_counter_clone.lock().unwrap() = frame_count;

            // Stop after saving first 10 frames
            if frame_count >= 10 {
                println!("Successfully saved 10 video frames");
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

    // Spawn a task to handle incoming audio samples
    let audio_task = tokio::spawn(async move {
        let mut packet_count = 0;
        let _start_time = Instant::now();
        let _sample_rate = 48000u32; // Expected sample rate

        while let Some((received_sample_rate, samples)) = audio_rx.recv().await {
            packet_count += 1;
            let duration_ms = (samples.len() as f64 / received_sample_rate as f64) * 1000.0;

            println!("Received audio packet #{}: {} Hz, {} samples ({:.2}ms)",
                packet_count, received_sample_rate, samples.len(), duration_ms);

            // Accumulate audio data
            {
                let mut buffer = audio_samples_buffer_clone.lock().unwrap();
                let mut duration = audio_duration_clone.lock().unwrap();
                buffer.extend_from_slice(&samples);
                *duration += duration_ms / 1000.0; // Convert to seconds

                // Save first 10 seconds of audio
                if *duration >= 10.0 {
                    println!("Collected {:.2} seconds of audio, saving...", *duration);

                    let output_file = format!("{}/first_10_seconds_audio.raw", output_dir_clone);
                    let header_file = format!("{}/first_10_seconds_audio_info.txt", output_dir_clone);

                    // Save raw audio data
                    match fs::write(&output_file,
                        buffer.iter()
                            .flat_map(|&sample| sample.to_le_bytes())
                            .collect::<Vec<u8>>()) {
                        Ok(_) => {
                            println!("Saved audio data to: {}", output_file);

                            // Save audio info
                            let data_size = buffer.len() * 4; // f32 is 4 bytes
                            let info_content = format!(
                                "Sample Rate: {} Hz\nChannels: 2\nDuration: {:.2} seconds\nTotal Samples: {}\nData Size: {} bytes",
                                received_sample_rate,
                                *duration,
                                buffer.len(),
                                data_size
                            );

                            if let Err(e) = fs::write(&header_file, info_content) {
                                eprintln!("Failed to write audio info file: {}", e);
                            }
                        }
                        Err(e) => eprintln!("Failed to save audio data: {}", e),
                    }

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

    // Create a monitoring task to check progress
    let video_counter_monitor = video_counter.clone();
    let audio_duration_monitor = audio_duration.clone();
    let monitor_task = tokio::spawn(async move {
        let mut last_video = 0u32;
        let mut last_audio = 0f64;

        loop {
            sleep(Duration::from_millis(1000)).await;

            let current_video = *video_counter_monitor.lock().unwrap();
            let current_audio = *audio_duration_monitor.lock().unwrap();

            if current_video != last_video || current_audio != last_audio {
                println!("Progress: {} video frames, {:.2} seconds audio",
                    current_video, current_audio);
                last_video = current_video;
                last_audio = current_audio;
            }

            // Check if we're done
            if current_video >= 10 && current_audio >= 10.0 {
                println!("Target achieved: 10 frames and 10 seconds of audio");
                break;
            }
        }
    });

    // Wait for tasks with timeout
    tokio::select! {
        _ = video_task => {
            println!("Video task completed");
        }
        _ = audio_task => {
            println!("Audio task completed");
        }
        _ = connect_task => {
            println!("Connect task completed");
        }
        _ = monitor_task => {
            println!("Monitor task completed");
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

    println!("\nTo view the RGB frames, you can use:");
    println!("  ffmpeg -f rawvideo -pixel_format rgb24 -video_size 1920x1080 -i {}/frame_001_1920x1080.rgb {}/frame_001.png", output_dir, output_dir);

    println!("To convert audio to WAV:");
    println!("  ffmpeg -f f32le -ar 48000 -ac 2 -i {}/first_10_seconds_audio.raw {}/first_10_seconds_audio.wav", output_dir, output_dir);

    println!("\nDemo completed successfully!");
    Ok(())
}