use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    export::{audio_exporter::{AudioExporter, AudioExportConfig, AudioExportFormat}, ExportPhase},
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

    if metadata.videos.is_empty() && metadata.audios.is_empty() {
        panic!("No video or audio tracks found in video file");
    }

    log::info!("Media info:");
    log::info!("  Duration: {:.2}s", metadata.duration.as_secs_f64());
    log::info!("  Video tracks: {}", metadata.videos.len());
    log::info!("  Audio tracks: {}", metadata.audios.len());

    if !metadata.videos.is_empty() {
        let video_meta = &metadata.videos[0];
        log::info!("  Resolution: {}x{}", video_meta.width, video_meta.height);
        log::info!("  FPS: {}", video_meta.fps);
    }

    if !metadata.audios.is_empty() {
        let audio_meta = &metadata.audios[0];
        log::info!("  Audio: {} ch, {} Hz", audio_meta.channels, audio_meta.sample_rate);
    }

    // Create segment
    let segment = Segment::new(Duration::ZERO, metadata.duration, metadata.clone(), 1.0);

    // Create video track (which includes audio)
    let inner_track = InnerTrack::new(metadata.clone(), metadata.duration, vec![Arc::new(segment)]);

    let video_track = VideoTrack {
        name: "Video Track".to_string(),
        hiding: false,
        muted: false,
        locked: false,
        track: inner_track,
    };

    manager.add_track(Track::Video(Arc::new(video_track)));

    // Add audio track from WAV file
    let wav_path = PathBuf::from("data/test.wav");
    log::info!("Loading audio from: {}", wav_path.display());

    if let Ok(audio_tracks) = Track::new(&wav_path, 1.0) {
        for audio_track in audio_tracks {
            manager.add_track(audio_track);
            log::info!("Added audio track from WAV file");
        }
    } else {
        log::warn!("Failed to load audio from: {}", wav_path.display());
    }

    // Export audio in different formats
    let formats = vec![
        (AudioExportFormat::Aac, "tmp/output_audio.aac"),
        (AudioExportFormat::Mp3, "tmp/output_audio.mp3"),
        (AudioExportFormat::Wav, "tmp/output_audio.wav"),
    ];

    for (format, output_file) in formats {
        log::info!("=== Exporting audio as {:?} ===", format);

        let config = AudioExportConfig::default()
            .with_output_path(PathBuf::from(output_file))
            .with_format(format)
            .with_bitrate(192_000);

        // Export with progress callback
        log::info!("Starting export to: {}", output_file);
        let exporter = AudioExporter::new(Arc::new(manager.clone()), config);

        match exporter.export_with_progress(|progress| {
            let percent = if progress.total_duration.as_secs_f64() > 0.0 {
                (progress.current_position.as_secs_f64()
                    / progress.total_duration.as_secs_f64())
                    * 100.0
            } else {
                0.0
            };
            let phase_str = match progress.phase {
                ExportPhase::Initializing => "Initializing",
                ExportPhase::EncodingVideo => "Encoding",
                ExportPhase::ProcessingAudio => "Processing Audio",
                ExportPhase::Finalizing => "Finalizing",
                ExportPhase::Complete => "Complete",
            };
            log::info!(
                "[{:?}] Progress: {:.1}%",
                phase_str,
                percent
            );
        }) {
            Ok(result) => {
                log::info!("Export complete!");
                log::info!("  Output: {:?}", result.output_path);
                log::info!("  Duration: {:.2}s", result.duration.as_secs_f64());
                log::info!("  Channels: {}", result.channels);
                log::info!("  Sample rate: {} Hz", result.sample_rate);
                log::info!("  Total samples: {}", result.total_samples);
            }
            Err(e) => {
                log::error!("Failed to export audio as {:?}: {}", format, e);
            }
        }
    }

    // Export with custom audio parameters
    log::info!("=== Exporting audio with custom parameters ===");

    let config = AudioExportConfig::default()
        .with_output_path(PathBuf::from("tmp/output_audio_custom.aac"))
        .with_format(AudioExportFormat::Aac)
        .with_bitrate(256_000)  // Higher bitrate
        .with_channels(Some(2)) // Stereo
        .with_sample_rate(Some(48000)); // 48 kHz

    let exporter = AudioExporter::new(Arc::new(manager.clone()), config);

    match exporter.export() {
        Ok(result) => {
            log::info!("Custom export complete!");
            log::info!("  Output: {:?}", result.output_path);
            log::info!("  Duration: {:.2}s", result.duration.as_secs_f64());
        }
        Err(e) => {
            log::error!("Failed to export audio with custom parameters: {}", e);
        }
    }

    Ok(())
}
