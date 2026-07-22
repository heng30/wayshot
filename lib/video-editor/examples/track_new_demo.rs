use std::path::PathBuf;
use video_editor::tracks::Track;

fn main() {
    env_logger::init();

    let test_file = PathBuf::from("data/test.mp4");

    log::info!("=== Testing Track::new with video file containing subtitles ===");
    log::info!("File: {}", test_file.display());

    match Track::new(&test_file, 1.0) {
        Ok(tracks) => {
            log::info!("Created {} tracks", tracks.len());

            for (idx, track) in tracks.iter().enumerate() {
                match track {
                    Track::Video(video_track) => {
                        log::info!("[Track {}] Video", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            video_track.track.duration.as_secs_f64()
                        );
                        log::info!("  Segments: {}", video_track.track.segments.len());
                        for (seg_idx, seg) in video_track.track.segments.iter().enumerate() {
                            log::info!(
                                "    [Segment {}] timeline_offset={:.2}s, duration={:.2}s",
                                seg_idx,
                                seg.timeline_offset.as_secs_f64(),
                                seg.duration.as_secs_f64()
                            );
                        }
                    }
                    Track::Audio(audio_track) => {
                        log::info!("[Track {}] Audio", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            audio_track.track.duration.as_secs_f64()
                        );
                        log::info!("  Segments: {}", audio_track.track.segments.len());
                    }
                    Track::Subtitle(subtitle_track) => {
                        log::info!("[Track {}] Subtitle", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            subtitle_track.track.duration.as_secs_f64()
                        );
                        log::info!(
                            "  Segments: {} (one per subtitle entry)",
                            subtitle_track.track.segments.len()
                        );

                        // 显示前 5 个字幕 segments
                        let show_count = 5.min(subtitle_track.track.segments.len());
                        log::info!("  First {} segments:", show_count);
                        for (seg_idx, seg) in subtitle_track
                            .track
                            .segments
                            .iter()
                            .take(show_count)
                            .enumerate()
                        {
                            log::info!(
                                "    [Segment {}] timeline_offset={:.2}s, duration={:.2}s",
                                seg_idx,
                                seg.timeline_offset.as_secs_f64(),
                                seg.duration.as_secs_f64()
                            );
                        }

                        if subtitle_track.track.segments.len() > show_count {
                            log::info!(
                                "    ... and {} more segments",
                                subtitle_track.track.segments.len() - show_count
                            );
                        }
                    }
                    Track::Image(image_track) => {
                        log::info!("[Track {}] Image", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            image_track.track.duration.as_secs_f64()
                        );
                        log::info!("  Segments: {}", image_track.track.segments.len());
                        log::info!("  Hidden: {}", image_track.hiding);
                    }
                    Track::Text(text_track) => {
                        log::info!("[Track {}] Text", idx);
                        log::info!(
                            "  Duration: {:.2}s",
                            text_track.track.duration.as_secs_f64()
                        );
                        log::info!("  Segments: {}", text_track.track.segments.len());
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to create tracks: {:?}", e);
        }
    }

    log::info!("=== Test completed ===");
}
