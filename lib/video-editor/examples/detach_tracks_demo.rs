use std::{path::PathBuf, sync::Arc};
use video_editor::tracks::Track;

fn main() {
    env_logger::init();

    let test_file = PathBuf::from("data/test.mp4");

    log::info!("=== Demo: Detach Audio and Subtitle Tracks from Video ===");
    log::info!("File: {}", test_file.display());

    match Track::new(&test_file, 1.0) {
        Ok(tracks) => {
            log::info!("Successfully created {} tracks", tracks.len());

            for track in tracks {
                if let Track::Video(video_track) = track {
                    let mut video_track = Arc::try_unwrap(video_track)
                        .expect("Failed to unwrap Arc - multiple references exist");

                    log::info!("=== Original Video Track Info ===");
                    log::info!("Duration: {:.2}s", video_track.track.duration.as_secs_f64());
                    log::info!("Segments: {}", video_track.track.segments.len());

                    let metadata = &video_track.track.metadata;
                    log::info!("Audio streams: {}", metadata.audios.len());
                    for (idx, audio) in metadata.audios.iter().enumerate() {
                        log::info!(
                            "  [Audio Stream {}] codec={:?}, sample_rate={}Hz, channels={}",
                            idx,
                            audio.codec_id,
                            audio.sample_rate,
                            audio.channels
                        );
                    }

                    log::info!("Subtitle streams: {}", metadata.subtitles.len());
                    for (idx, subtitle) in metadata.subtitles.iter().enumerate() {
                        log::info!(
                            "  [Subtitle Stream {}] codec={:?}, language={:?}",
                            idx,
                            subtitle.codec_id,
                            subtitle.language
                        );
                    }

                    log::info!("=== Detach Audio Tracks ===");
                    let audio_tracks = video_track.detach_audio_tracks();
                    log::info!("Successfully detached {} audio tracks", audio_tracks.len());

                    for (idx, audio_track) in audio_tracks.iter().enumerate() {
                        log::info!("[Audio Track {}]", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            audio_track.track.duration.as_secs_f64()
                        );
                        log::info!("  Segments: {}", audio_track.track.segments.len());

                        let metadata = &audio_track.track.metadata;
                        if let Some(audio) = metadata.audios.first() {
                            log::info!(
                                "  Codec: {:?}, Sample Rate: {}Hz, Channels: {}",
                                audio.codec_id,
                                audio.sample_rate,
                                audio.channels
                            );
                        }
                    }

                    log::info!("=== Detach Subtitle Tracks ===");
                    let subtitle_tracks = video_track.detach_subtitle_tracks(1.0);
                    log::info!(
                        "Successfully detached {} subtitle tracks",
                        subtitle_tracks.len()
                    );

                    for (idx, subtitle_track) in subtitle_tracks.iter().enumerate() {
                        log::info!("[Subtitle Track {}]", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            subtitle_track.track.duration.as_secs_f64()
                        );
                        log::info!("  Segments: {}", subtitle_track.track.segments.len());

                        let entries = subtitle_track.get_subtitle_entries();
                        log::info!("  Subtitle entries: {}", entries.len());

                        let metadata = &subtitle_track.track.metadata;
                        if let Some(subtitle) = metadata.subtitles.first() {
                            log::info!(
                                "  Codec: {:?}, Language: {:?}",
                                subtitle.codec_id,
                                subtitle.language
                            );
                        }

                        // Display first 3 subtitle entries
                        let show_count = 3.min(entries.len());
                        log::info!("  First {} subtitle entries:", show_count);
                        for (entry_idx, entry) in
                            entries.iter().take(show_count).enumerate()
                        {
                            log::info!(
                                "    [{}] {:.2}s - {:.2}s: {}",
                                entry_idx,
                                entry.start.as_secs_f64(),
                                entry.end.as_secs_f64(),
                                if entry.text.len() > 50 {
                                    format!("{}...", &entry.text[..50])
                                } else {
                                    entry.text.clone()
                                }
                            );
                        }

                        if entries.len() > show_count {
                            log::info!(
                                "    ... and {} more entries",
                                entries.len() - show_count
                            );
                        }
                    }

                    log::info!("=== Video Track Info After Detaching ===");
                    let metadata_after = &video_track.track.metadata;
                    log::info!("Duration: {:.2}s", video_track.track.duration.as_secs_f64());
                    log::info!("Segments: {}", video_track.track.segments.len());
                    log::info!("Audio streams: {}", metadata_after.audios.len());
                    log::info!("Subtitle streams: {}", metadata_after.subtitles.len());
                    log::info!("Video streams: {}", metadata_after.videos.len());
                }
            }
        }
        Err(e) => {
            log::error!("Failed to create tracks: {:?}", e);
        }
    }

    log::info!("=== Demo Complete ===");
}
