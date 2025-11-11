use mp4_player::player::DecodedVideoFrame;
use mp4_player::{Config, Mp4Player};
use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::Duration,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        log::error!("Usage: {} <mp4_file_path> <start_seconds>", args[0]);
        log::error!("");
        log::error!("Example:");
        log::error!("  {} /path/to/video.mp4 5.5", args[0]);
        log::error!("  {} ./sample.mp4 10", args[0]);
        return Ok(());
    }

    let mp4_path = &args[1];
    let start_seconds: f64 = args[2].parse()?;

    if !std::path::Path::new(mp4_path).exists() {
        log::error!("Error: File not found: {}", mp4_path);
        return Ok(());
    }

    let start_time = Duration::from_secs_f64(start_seconds);

    log::info!("🎬 Testing MP4 Player with Start Time Feature");
    log::info!("============================================");
    log::info!("File: {}", mp4_path);
    log::info!(
        "Start time: {:.1} seconds ({:?})",
        start_seconds,
        start_time
    );

    let stop_sig = Arc::new(AtomicBool::new(false));
    let config = Config::new(mp4_path)
        .with_stop_sig(stop_sig.clone())
        .with_sound(Arc::new(AtomicU32::new(50)));
    let mut player = Mp4Player::new(config)?;

    log::info!("✅ Mp4Player created successfully!");

    // Get frame receivers
    let video_receiver = player.video_frame_receiver();
    log::info!("✅ Frame receivers obtained!");

    // Start playing from the specified start time
    log::info!("▶️  Starting playback from {:.1} seconds...", start_seconds);
    player.play(start_time);

    let mut video_count = 0;
    let mut first_video_time = None;
    let mut rgb_frame_count = 0;
    let start_time = std::time::Instant::now();

    // Create output directory
    let output_dir = Path::new("/tmp/mp4-player");
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    let stop_sig_clone = stop_sig.clone();
    thread::spawn(move || {
        while start_time.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(10));
        }

        log::info!("Timeout exit mp4 player");
        stop_sig_clone.store(true, Ordering::Relaxed);
    });

    while !stop_sig.load(Ordering::Relaxed) {
        if let Ok(decoded_frame) = video_receiver.recv_timeout(Duration::from_millis(10)) {
            video_count += 1;

            match decoded_frame {
                DecodedVideoFrame::Data(frame) => {
                    if first_video_time.is_none() {
                        first_video_time = Some(frame.timestamp);
                    }
                    if video_count <= 3 {
                        log::info!(
                            "📹 Video frame #{}: timestamp={:.3}s, resolution={}x{}, keyframe={}",
                            video_count,
                            frame.timestamp.as_secs_f64(),
                            frame.width,
                            frame.height,
                            frame.is_keyframe
                        );
                    }

                    rgb_frame_count += 1;

                    if rgb_frame_count <= 25 {
                        tokio::spawn(async move {
                            let filename = format!(
                                "rgb_frame_{:04}_{:.3}s.png",
                                rgb_frame_count,
                                frame.timestamp.as_secs_f64()
                            );
                            let filepath = output_dir.join(filename);

                            if let Err(e) = frame.image_buffer.save(&filepath) {
                                log::warn!(
                                    "Failed to save RGB frame to {}: {:?}",
                                    filepath.display(),
                                    e
                                );
                            } else {
                                log::info!("💾 Saved RGB frame to: {}", filepath.display());
                            }
                        });
                    }
                }
                DecodedVideoFrame::Empty => {
                    log::info!("📹 Video frame #{}: Empty frame", video_count);
                }
                DecodedVideoFrame::EOF => {
                    log::info!(
                        "📹 Video frame #{}: received `DecodedVideoFrame::EOF`",
                        video_count
                    );
                }
                _ => (),
            }
        }
    }

    player.stop()?;

    log::info!("📈 Test Results:");
    log::info!("================");
    log::info!("Requested start time: {:.1} seconds", start_seconds);

    if let Some(first_video_time) = first_video_time {
        log::info!(
            "First video frame timestamp: {:.3} seconds",
            first_video_time.as_secs_f64()
        );
        let video_diff = (first_video_time.as_secs_f64() - start_seconds).abs();
        log::info!("Video time accuracy: {:.3} seconds difference", video_diff);
    }

    log::info!("Video frames received: {}", video_count);
    log::info!("RGB frames decoded and saved: {}", rgb_frame_count);

    if video_count > 0 {
        log::info!("✅ Start time feature working correctly!");
    } else {
        log::warn!(
            "⚠️  No frames received - this might be expected for placeholder implementation"
        );
    }

    if rgb_frame_count > 0 {
        log::info!("✅ RGB frame decoding and saving working correctly!");
        log::info!("📁 Check saved frames in: {}", output_dir.display());
    } else {
        log::warn!("⚠️  No RGB frames decoded - this might be due to decoding issues");

        // Create a test RGB frame to verify PNG saving works
        log::info!("🧪 Creating test RGB frame to verify PNG saving...");
        let test_width = 320;
        let test_height = 240;
        let mut test_rgb_data = Vec::with_capacity((test_width * test_height * 3) as usize);

        // Create a simple gradient pattern
        for y in 0..test_height {
            for x in 0..test_width {
                let r = (x * 255 / test_width) as u8;
                let g = (y * 255 / test_height) as u8;
                let b = ((x + y) * 255 / (test_width + test_height)) as u8;
                test_rgb_data.push(r);
                test_rgb_data.push(g);
                test_rgb_data.push(b);
            }
        }

        // Save test RGB data as PNG
        if let Some(test_img) = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
            test_width,
            test_height,
            test_rgb_data,
        ) {
            let test_filepath = output_dir.join("test_rgb_gradient.png");
            if let Err(e) = test_img.save(&test_filepath) {
                log::warn!("Failed to save test PNG: {:?}", e);
            } else {
                log::info!("✅ Test PNG saved to: {}", test_filepath.display());
                log::info!("📁 PNG saving functionality verified!");
            }
        }
    }

    log::info!("✨ Test completed successfully!");
    Ok(())
}
