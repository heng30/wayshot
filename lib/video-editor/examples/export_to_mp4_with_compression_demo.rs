use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    export::{CompressionPreset, Mp4ExportConfig, Mp4Exporter, Tune},
    metadata::get_metadata,
    tracks::{
        manager::Manager,
        segment::Segment,
        track::{InnerTrack, Track},
        video_track::VideoTrack,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Create manager and add video track
    let mut manager = Manager::new();

    // Load video file
    let video_path = PathBuf::from("data/test.mp4");
    log::info!("Loading video from: {}", video_path.display());

    let metadata = Arc::new(get_metadata(&video_path)?);

    if metadata.videos.is_empty() {
        panic!("No video tracks found in video file");
    }

    let video_meta = &metadata.videos[0];
    log::info!("Video track info:");
    log::info!("  Resolution: {}x{}", video_meta.width, video_meta.height);
    log::info!("  FPS: {}", video_meta.fps);
    log::info!("  Duration: {:.2}s", metadata.duration.as_secs_f64());

    // Create segment
    let segment = Segment::new(Duration::ZERO, metadata.duration, metadata.clone(), 1.0);

    // Create video track
    let inner_track = InnerTrack::new(metadata.clone(), metadata.duration, vec![Arc::new(segment)]);

    let video_track = VideoTrack {
        name: "Video Track".to_string(),
        hiding: false,
        muted: false,
        locked: false,
        track: inner_track,
    };

    manager.add_track(Track::Video(Arc::new(video_track)));

    log::info!("\n=== Testing different compression configurations ===\n");

    // Test 1: High quality (low compression)
    log::info!("Test 1: High quality (CRF=18, Preset=Slow)");
    let config1 = Mp4ExportConfig::default()
        .with_output_path(PathBuf::from("tmp/output_high_quality.mp4"))
        .with_crf(Some(18))
        .with_compression_preset(Some(CompressionPreset::Slow))
        .with_use_crf(true);

    _ = run_export(&manager, config1);

    // Test 2: Balanced (default)
    log::info!("\nTest 2: Balanced quality (CRF=23, Preset=Medium)");
    let config2 = Mp4ExportConfig::default()
        .with_output_path(PathBuf::from("tmp/output_balanced.mp4"))
        .with_crf(Some(23))
        .with_compression_preset(Some(CompressionPreset::Medium))
        .with_use_crf(true);

    _ = run_export(&manager, config2);

    // Test 3: Fast encoding (higher compression, faster)
    log::info!("\nTest 3: Fast encoding (CRF=28, Preset=Veryfast)");
    let config3 = Mp4ExportConfig::default()
        .with_output_path(PathBuf::from("tmp/output_fast.mp4"))
        .with_crf(Some(28))
        .with_compression_preset(Some(CompressionPreset::Veryfast))
        .with_use_crf(true);

    _ = run_export(&manager, config3);

    // Test 4: Optimized for animation
    log::info!("\nTest 4: Animation optimized (CRF=20, Preset=Medium, Tune=Animation)");
    let config4 = Mp4ExportConfig::default()
        .with_output_path(PathBuf::from("tmp/output_animation.mp4"))
        .with_crf(Some(20))
        .with_compression_preset(Some(CompressionPreset::Medium))
        .with_tune(Some(Tune::Animation))
        .with_use_crf(true);

    _ = run_export(&manager, config4);

    // Compare file sizes
    log::info!("\n=== File size comparison ===");
    compare_file_sizes(vec![
        ("High quality (CRF 18, Slow)", "tmp/output_high_quality.mp4"),
        ("Balanced (CRF 23, Medium)", "tmp/output_balanced.mp4"),
        ("Fast (CRF 28, Veryfast)", "tmp/output_fast.mp4"),
        ("Animation tuned", "tmp/output_animation.mp4"),
    ]);

    Ok(())
}

fn run_export(
    manager: &Manager,
    config: Mp4ExportConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let exporter = Mp4Exporter::new(manager.clone(), config);
    let result = exporter.export_with_progress(|progress| {
        let percent = progress.progress() * 100.0;
        if (progress.frames_processed % 30 == 0) || progress.phase.is_complete() {
            log::info!(
                "  Progress: {:.1}% ({} frames)",
                percent,
                progress.frames_processed
            );
        }
    })?;
    let elapsed = start.elapsed();
    log::info!(
        "  Complete: {:.2}s in {:.2}s",
        result.duration.as_secs_f64(),
        elapsed.as_secs_f64()
    );
    Ok(())
}

fn compare_file_sizes(files: Vec<(&str, &str)>) {
    let mut sizes = Vec::new();
    for (name, path) in files {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                let size_kb = metadata.len() / 1024;
                let size_mb = size_kb as f64 / 1024.0;
                sizes.push((name, size_mb));
                log::info!("  {}: {:.2} MB", name, size_mb);
            }
            Err(e) => {
                log::warn!("  {}: Failed to get file size: {}", name, e);
            }
        }
    }

    if !sizes.is_empty() {
        let min_size = sizes.iter().map(|(_, s)| *s).reduce(f64::min).unwrap();
        let max_size = sizes.iter().map(|(_, s)| *s).reduce(f64::max).unwrap();
        if max_size > min_size {
            let ratio = ((max_size - min_size) / min_size) * 100.0;
            log::info!("  Size difference: {:.1}%", ratio);
        }
    }
}
