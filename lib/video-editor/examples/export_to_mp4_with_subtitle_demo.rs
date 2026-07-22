use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    export::{Mp4ExportConfig, Mp4Exporter},
    filters::subtitle::style::{
        alignment::AlignmentFilter, font_path::FontPathFilter, font_size::FontSizeFilter,
        margin::MarginVerticalFilter,
    },
    metadata::get_metadata,
    tracks::{
        manager::Manager,
        segment::Segment,
        track::{InnerTrack, Track},
        video_track::VideoTrack,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

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

    // Create video segment (no subtitle filters needed)
    let video_segment = Arc::new(Segment::new(Duration::ZERO, metadata.duration, metadata.clone(), 1.0));

    // Create video track
    let video_inner_track = InnerTrack::new(metadata.clone(), metadata.duration, vec![video_segment]);

    let video_track = VideoTrack {
        name: "Video Track".to_string(),
        hiding: false,
        muted: false,
        locked: false,
        track: video_inner_track,
    };

    manager.add_track(Track::Video(Arc::new(video_track)));

    // Load subtitle track from SRT file
    let subtitle_path = PathBuf::from("data/test.srt");
    log::info!("Loading subtitle from: {}", subtitle_path.display());

    if subtitle_path.exists() {
        match Track::new(&subtitle_path, 1.0) {
            Ok(mut subtitle_tracks) => {
                for subtitle_track in &mut subtitle_tracks {
                    if let Track::Subtitle(arc_subtitle_track) = subtitle_track {
                        let subtitle_track = Arc::make_mut(arc_subtitle_track);

                        // Apply subtitle filters to all segments
                        for segment in &mut subtitle_track.track.segments {
                            let segment_mut = Arc::make_mut(segment);

                            // Font path
                            let font_path = PathBuf::from("../../wayshot/ui/fonts/SourceHanSansCN.otf");
                            if font_path.exists() {
                                segment_mut.add_subtitle_filter(Box::new(FontPathFilter::new(font_path, "SourceHanSansCN".to_string(), String::new())));
                            }

                            // Font size
                            segment_mut.add_subtitle_filter(Box::new(FontSizeFilter::new(48)));

                            // Alignment: top-center
                            segment_mut.add_subtitle_filter(Box::new(AlignmentFilter::top_center()));

                            // Margin vertical
                            segment_mut.add_subtitle_filter(Box::new(MarginVerticalFilter::new(Some(100))));
                        }

                        log::info!("Loaded {} subtitle entries", subtitle_track.get_subtitle_entries().len());
                        manager.add_track(Track::Subtitle(Arc::clone(arc_subtitle_track)));
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to load subtitle file: {:?}", e);
            }
        }
    } else {
        log::warn!("Subtitle file not found: {}", subtitle_path.display());
    }

    // Configure export with burned subtitles
    let output_path = PathBuf::from("tmp/output_with_subtitle.mp4");

    let config = Mp4ExportConfig::default()
        .with_output_path(output_path.clone())
        .with_burn_subtitles(true);

    // Export with progress callback
    log::info!("Starting export to: {}", output_path.display());
    let exporter = Mp4Exporter::new(manager, config);
    let result = exporter.export_with_progress(|progress| {
        let percent = progress.progress() * 100.0;
        let phase_str = match progress.phase {
            video_editor::export::ExportPhase::Initializing => "Initializing",
            video_editor::export::ExportPhase::EncodingVideo => "Encoding Video",
            video_editor::export::ExportPhase::ProcessingAudio => "Processing Audio",
            video_editor::export::ExportPhase::Finalizing => "Finalizing",
            video_editor::export::ExportPhase::Complete => "Complete",
        };
        log::info!(
            "[{:?}] Progress: {:.1}% ({} / {} frames)",
            phase_str,
            percent,
            progress.frames_processed,
            progress.total_frames
        );
    })?;

    log::info!("Export complete!");
    log::info!("  Output: {:?}", result.output_path);
    log::info!("  Duration: {:.2}s", result.duration.as_secs_f64());
    log::info!("  Total frames: {}", result.total_frames);

    Ok(())
}
