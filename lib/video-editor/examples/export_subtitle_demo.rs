use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    export::{
        ExportPhase,
        subtitle_exporter::{SubtitleExportConfig, SubtitleExporter, SubtitleFormat},
    },
    filters::traits::SubtitleEntry,
    metadata::get_metadata,
    tracks::{
        manager::Manager,
        segment::Segment,
        subtitle_track::SubtitleTrack,
        track::{InnerTrack, Track},
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create manager
    let mut manager = Manager::new();

    // Test with different subtitle sources
    let test_cases = vec![
        ("data/test.srt", "External SRT file"),
        ("data/test.mp4", "Video with embedded subtitles"),
    ];

    for (file_path, description) in test_cases {
        log::info!("=== Testing: {} ===", description);
        log::info!("File: {}", file_path);

        let path = PathBuf::from(file_path);

        // Check if file exists
        if !path.exists() {
            log::warn!("File not found, skipping: {}", path.display());
            continue;
        }

        // Get metadata
        let metadata = match get_metadata(&path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                log::warn!("Failed to get metadata: {}, skipping", e);
                continue;
            }
        };

        log::info!("Duration: {:.2}s", metadata.duration.as_secs_f64());

        // Check for subtitles
        if metadata.subtitles.is_empty() {
            log::warn!("No subtitles found, skipping");
            continue;
        }

        // Create tracks
        let tracks = Track::new(&path, 1.0)?;
        let mut subtitle_count = 0;

        for track in tracks {
            match track {
                Track::Subtitle(subtitle_track) => {
                    subtitle_count += 1;
                    log::info!(
                        "Found subtitle track: {} entries",
                        subtitle_track.get_subtitle_entries().len()
                    );
                    manager.add_track(Track::Subtitle(Arc::clone(&subtitle_track)));
                }
                Track::Video(video_track) => {
                    // Detach subtitle tracks from video
                    let mut mutable_track = Arc::clone(&video_track);
                    let video = Arc::make_mut(&mut mutable_track);
                    let detached = video.detach_subtitle_tracks(1.0);

                    for sub_track in detached {
                        subtitle_count += 1;
                        log::info!(
                            "Detached subtitle track: {} entries",
                            sub_track.get_subtitle_entries().len()
                        );
                        manager.add_track(Track::Subtitle(Arc::new(sub_track)));
                    }
                }
                _ => {}
            }
        }

        if subtitle_count == 0 {
            log::warn!("No subtitle tracks added, skipping");
            continue;
        }

        // Export in different formats
        let formats = vec![
            (SubtitleFormat::Srt, "srt"),
            (SubtitleFormat::Vtt, "vtt"),
            (SubtitleFormat::Ass, "ass"),
        ];

        for (format, ext) in formats {
            log::info!("=== Exporting as {} ===", ext.to_uppercase());

            let output_base = format!("tmp/output_subtitle_{}", ext);

            let config = SubtitleExportConfig::default()
                .with_output_base_path(PathBuf::from(output_base))
                .with_format(format)
                .with_include_track_index(true);

            let exporter = SubtitleExporter::new(Arc::new(manager.clone()), config);

            // Export all tracks
            match exporter.export_all_tracks() {
                Ok(results) => {
                    for result in results {
                        log::info!(
                            "Exported track {}: {} ({} entries)",
                            result.track_index,
                            result.output_path.display(),
                            result.entry_count
                        );
                    }
                }
                Err(e) => {
                    log::error!("Failed to export as {}: {}", ext, e);
                }
            }
        }

        // Export with progress callback
        log::info!("=== Exporting with progress callback ===");

        let config = SubtitleExportConfig::default()
            .with_output_base_path(PathBuf::from("tmp/output_subtitle_progress"))
            .with_format(SubtitleFormat::Srt)
            .with_include_track_index(true);

        let exporter = SubtitleExporter::new(Arc::new(manager.clone()), config);

        match exporter.export_all_tracks_with_progress(|progress| {
            let percent = if progress.total_duration.as_secs_f64() > 0.0 {
                (progress.current_position.as_secs_f64() / progress.total_duration.as_secs_f64())
                    * 100.0
            } else {
                0.0
            };
            let phase_str = match progress.phase {
                ExportPhase::Initializing => "Initializing",
                ExportPhase::EncodingVideo => "Exporting",
                ExportPhase::ProcessingAudio => "Processing",
                ExportPhase::Finalizing => "Finalizing",
                ExportPhase::Complete => "Complete",
            };
            log::info!(
                "[{:?}] Progress: {:.1}% ({}/{})",
                phase_str,
                percent,
                progress.frames_processed,
                progress.total_frames
            );
        }) {
            Ok(results) => {
                log::info!("Export complete with progress!");
                for result in results {
                    log::info!(
                        "  Track {}: {} ({} entries)",
                        result.track_index,
                        result.output_path.display(),
                        result.entry_count
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to export with progress: {}", e);
            }
        }

        // Export single track
        log::info!("=== Exporting single track (index 0) ===");

        let config = SubtitleExportConfig::default()
            .with_output_base_path(PathBuf::from("tmp/output_subtitle_single"))
            .with_format(SubtitleFormat::Srt);

        let exporter = SubtitleExporter::new(Arc::new(manager.clone()), config);

        match exporter.export_track(0) {
            Ok(result) => {
                log::info!(
                    "Exported single track: {} ({} entries)",
                    result.output_path.display(),
                    result.entry_count
                );
            }
            Err(e) => {
                log::error!("Failed to export single track: {}", e);
            }
        }

        // Export with custom suffix
        log::info!("=== Exporting with custom suffix ===");

        let config = SubtitleExportConfig::default()
            .with_output_base_path(PathBuf::from("tmp/output_subtitle_custom"))
            .with_format(SubtitleFormat::Srt)
            .with_include_track_index(false)
            .with_suffix(Some("en".to_string()));

        let exporter = SubtitleExporter::new(Arc::new(manager.clone()), config);

        match exporter.export_track(0) {
            Ok(result) => {
                log::info!("Exported with suffix: {}", result.output_path.display());
            }
            Err(e) => {
                log::error!("Failed to export with suffix: {}", e);
            }
        }

        // Clear manager for next test case
        manager = Manager::new();
    }

    // Manual subtitle creation and export
    log::info!("=== Creating manual subtitle track ===");

    let mut manager = Manager::new();

    // Create a dummy metadata for manual subtitles
    let dummy_path = PathBuf::from("manual.srt");
    let dummy_metadata = Arc::new(video_editor::metadata::Metadata {
        path: dummy_path.clone(),
        size: 0,
        bitrate: 0,
        duration: Duration::from_secs(9),
        format: vec!["SubRip".to_string()],
        videos: vec![],
        audios: vec![],
        subtitles: vec![],
    });

    // Create subtitles as segments with subtitle_text
    let subtitles = vec![
        SubtitleEntry {
            start: Duration::from_secs(0),
            end: Duration::from_secs(3),
            text: "First subtitle".to_string(),
        },
        SubtitleEntry {
            start: Duration::from_secs(3),
            end: Duration::from_secs(6),
            text: "Second subtitle".to_string(),
        },
        SubtitleEntry {
            start: Duration::from_secs(6),
            end: Duration::from_secs(9),
            text: "Third subtitle".to_string(),
        },
    ];

    let segments: Vec<Arc<Segment>> = subtitles
        .iter()
        .map(|entry| {
            let segment_duration = entry.end.saturating_sub(entry.start);
            Arc::new(
                Segment::new_with_source_offset(
                    entry.start,
                    entry.start,
                    segment_duration,
                    1.0,
                    1.0,
                    dummy_metadata.clone(),
                )
                .with_subtitle_text(&entry.text),
            )
        })
        .collect();

    // Create inner track
    let inner_track = InnerTrack::new(dummy_metadata.clone(), Duration::from_secs(9), segments);

    let subtitle_track = SubtitleTrack::new(inner_track);

    manager.add_track(Track::Subtitle(Arc::new(subtitle_track)));

    let config = SubtitleExportConfig::default()
        .with_output_base_path(PathBuf::from("tmp/output_subtitle_manual"))
        .with_format(SubtitleFormat::Srt);

    let exporter = SubtitleExporter::new(Arc::new(manager), config);

    match exporter.export_track(0) {
        Ok(result) => {
            log::info!(
                "Exported manual subtitles: {} ({} entries)",
                result.output_path.display(),
                result.entry_count
            );
        }
        Err(e) => {
            log::error!("Failed to export manual subtitles: {}", e);
        }
    }

    log::info!("=== All tests completed ===");

    Ok(())
}
