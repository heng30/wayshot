//! Seek performance benchmark for video playback using UnifiedVideoTracksCompositorIterator.
//!
//! This example tests the performance of seeking to different positions in a video
//! to verify that the seek optimization works correctly in real playback scenarios.
//!
//! Usage:
//!   cargo run --release --example seek_performance_demo -- /home/blue/data/2min.mp4

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use video_editor::{
    metadata::get_metadata,
    tracks::{
        segment::Segment,
        track::InnerTrack,
        video_track::{
            UnifiedVideoTracksCompositorIterator, VideoSegmentSourceInfo, VideoSourceInfo,
        },
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();
    let video_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("data").join("test.mp4")
    };

    log::info!("Loading video from: {}", video_path.display());

    let metadata = Arc::new(get_metadata(&video_path)?);
    let video_meta = metadata.videos.first().ok_or("No video track found")?;

    log::info!("\n=== Video Info ===");
    log::info!("  Resolution: {}x{}", video_meta.width, video_meta.height);
    log::info!("  FPS: {}", video_meta.fps);
    log::info!("  Duration: {:.2}s", metadata.duration.as_secs_f64());
    log::info!(
        "  Estimated frame count: {}",
        (metadata.duration.as_secs_f64() * video_meta.fps as f64) as usize
    );

    log::info!("\n=== Seeking Performance Benchmark ===");
    log::info!("Testing seek to various positions using UnifiedVideoTracksCompositorIterator...\n");

    // Create a Segment that spans the entire video
    let segment = Arc::new(Segment::new(
        Duration::ZERO,
        metadata.duration,
        metadata.clone(),
        1.0,
    ));

    // Create VideoTrack and VideoSourceInfo
    let _inner_track = InnerTrack::new(metadata.clone(), metadata.duration, vec![segment.clone()]);

    let segment_info = VideoSegmentSourceInfo::new(
        Some(video_path.clone()),
        Some(video_meta.fps),
        segment.clone(),
        0, // segment_index
    );
    let source_info = VideoSourceInfo::new(0, vec![segment_info]);

    // Define test positions (in seconds)
    let test_positions_sec: Vec<u64> = if metadata.duration.as_secs() >= 120 {
        vec![5, 15, 30, 60, 90, 120]
    } else if metadata.duration.as_secs() >= 60 {
        vec![5, 15, 30, 45, 60]
    } else {
        vec![5, 10, 20, 30]
    };

    // Filter positions that are within video duration
    let test_positions: Vec<u64> = test_positions_sec
        .into_iter()
        .filter(|&pos| pos < metadata.duration.as_secs())
        .collect();

    log::info!(
        "Testing {} positions: {:?}",
        test_positions.len(),
        test_positions
    );

    let output_width = video_meta.width;
    let output_height = video_meta.height;
    let output_fps = video_meta.fps;

    let mut results: Vec<(u64, Duration)> = Vec::new();

    // Test each position
    for position_sec in &test_positions {
        let seek_position = Duration::from_secs(*position_sec);
        log::info!(
            "\n--- Seeking to {}s ({:.1}% of video) ---",
            position_sec,
            seek_position.as_secs_f64() / metadata.duration.as_secs_f64() * 100.0
        );

        // Run multiple iterations for more accurate measurement
        const ITERATIONS: usize = 2;
        let mut times: Vec<Duration> = Vec::new();

        for i in 0..ITERATIONS {
            let start = Instant::now();

            match UnifiedVideoTracksCompositorIterator::new(
                vec![source_info.clone()],
                seek_position,
                Duration::from_secs(3), // cache_duration
                Duration::from_secs(8), // max_cache_duration
                output_width,
                output_height,
                output_fps,
            ) {
                Ok(mut iter) => {
                    // Measure time to get first frame (startup latency)
                    if let Some(first_frame) = iter.next() {
                        let elapsed = start.elapsed();
                        times.push(elapsed);
                        log::info!(
                            "  Iteration {}: {:?} (frame size: {}x{})",
                            i + 1,
                            elapsed,
                            first_frame.composited_image.width(),
                            first_frame.composited_image.height()
                        );
                    } else {
                        log::error!("  Iteration {} FAILED: No frames returned", i + 1);
                    }
                }
                Err(e) => {
                    log::error!("  Iteration {} FAILED to create iterator: {:?}", i + 1, e);
                }
            }
        }

        if !times.is_empty() {
            let avg: Duration = times.iter().sum::<Duration>() / times.len() as u32;
            let min: Duration = *times.iter().min().unwrap();
            let max: Duration = *times.iter().max().unwrap();
            log::info!("  Average: {:?}, Min: {:?}, Max: {:?}", avg, min, max);
            results.push((*position_sec, avg));
        }
    }

    // Summary
    log::info!("\n=== Performance Summary ===");
    log::info!("Position (s) | Avg First Frame Latency");
    log::info!("-------------|------------------------");
    for (pos, time) in &results {
        log::info!("{:>12} | {:?}", pos, time);
    }

    // Analysis
    if results.len() >= 2 {
        log::info!("\n=== Analysis ===");

        // Check if seek times increase significantly with position
        let first_time = results.first().unwrap().1;
        let last_time = results.last().unwrap().1;

        if last_time > first_time * 3 {
            log::warn!(
                "Warning: Seek time increases significantly with position ({:?} -> {:?})",
                first_time,
                last_time
            );
            log::warn!("This may indicate seek is not working correctly or large GOP sizes.");
        } else {
            log::info!(
                "✓ Seek performance is relatively consistent across the video ({:?} to {:?})",
                first_time,
                last_time
            );
        }

        // Check if any seek took too long (> 500ms is problematic for UI)
        let slow_seeks: Vec<_> = results
            .iter()
            .filter(|(_, t)| t.as_millis() > 500)
            .collect();
        if !slow_seeks.is_empty() {
            log::warn!(
                "Warning: {} seek(s) took more than 500ms:",
                slow_seeks.len()
            );
            for (pos, time) in slow_seeks {
                log::warn!("  {}s: {:?}", pos, time);
            }
        } else {
            log::info!("✓ All seek operations completed in under 500ms");
        }

        // Target performance: all seeks should be under 200ms
        let good_seeks: Vec<_> = results
            .iter()
            .filter(|(_, t)| t.as_millis() < 200)
            .collect();
        log::info!(
            "✓ {} out of {} seeks were under 200ms (target performance)",
            good_seeks.len(),
            results.len()
        );
    }

    log::info!("\n✓ Benchmark complete!");
    Ok(())
}

