use std::{path::PathBuf, sync::Arc, time::Duration};
use video_editor::{
    metadata::get_metadata,
    tracks::{
        subtitle_track::{SubtitleSource, UnifiedSubtitleTracksCompositorIterator, extract_subtitles},
        track::Track,
    },
};

fn main() {
    env_logger::init();

    for file_path in ["data/test.srt", "data/test.mp4"] {
        let file_path = PathBuf::from(file_path);
        log::info!("=== Extracting subtitles from: {} ===", file_path.display());

        // 获取元数据
        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                log::error!("Failed to get metadata: {}", e);
                return;
            }
        };

        // 显示字幕信息
        log::info!("Video duration: {:.2}s", metadata.duration.as_secs_f64());
        log::info!("Found {} subtitle stream(s)", metadata.subtitles.len());

        if metadata.subtitles.is_empty() {
            log::warn!("No subtitle streams found in the video file");
            return;
        }

        for (idx, sub) in metadata.subtitles.iter().enumerate() {
            log::info!(
                "  Stream {}: codec={:?}, language={:?}, duration={:.2}s",
                idx,
                sub.codec_id,
                sub.language,
                sub.duration.as_secs_f64()
            );
        }

        let mut subtitle_entries = vec![];

        // 提取每个字幕流的字幕
        for subtitle in metadata.subtitles.iter() {
            let stream_idx = subtitle.index;

            log::info!("=== Extracting from subtitle stream {} ===", stream_idx);

            match extract_subtitles(&metadata.path, stream_idx) {
                Ok(entries) => {
                    log::info!("Found {} subtitle entries", entries.len());

                    if entries.is_empty() {
                        log::warn!("No subtitle entries found in the specified time range");
                        continue;
                    }

                    if subtitle_entries.is_empty() {
                        subtitle_entries = entries.clone();
                    }

                    // 显示前 10 个字幕条目
                    let show_count = 10.min(entries.len());
                    log::info!("First {} subtitle entries:", show_count);

                    for (i, entry) in entries.iter().take(show_count).enumerate() {
                        log::info!(
                            "  [{}] {:.3}s - {:.3}s: {}",
                            i,
                            entry.start.as_secs_f64(),
                            entry.end.as_secs_f64(),
                            entry.text
                        );
                    }

                    if entries.len() > show_count {
                        log::info!("  ... and {} more entries", entries.len() - show_count);
                    }

                    // 统计信息
                    let total_duration = entries
                        .iter()
                        .map(|e| (e.end - e.start).as_secs_f64())
                        .sum::<f64>();
                    let avg_duration = if entries.is_empty() {
                        0.0
                    } else {
                        total_duration / entries.len() as f64
                    };

                    log::info!("Subtitle statistics:");
                    log::info!("  Total entries: {}", entries.len());
                    log::info!("  Total subtitle duration: {:.2}s", total_duration);
                    log::info!("  Average duration per entry: {:.3}s", avg_duration);
                }
                Err(e) => {
                    log::error!("Failed to extract subtitles: {:?}", e);
                }
            }
        }

        log::info!("=== Using UnifiedSubtitleTracksCompositorIterator ===");

        // 判断是纯字幕文件还是包含嵌入字幕的视频/音频文件
        let is_subtitle_only = metadata.subtitles.len() > 0
            && metadata.videos.is_empty()
            && metadata.audios.is_empty();

        let subtitle_tracks: Vec<SubtitleSource> = if is_subtitle_only {
            // 纯字幕文件（如 .srt）：直接使用 Track::new
            match Track::new(&file_path, 1.0) {
                Ok(tracks) => tracks
                    .into_iter()
                    .enumerate()
                    .filter_map(|(idx, track)| {
                        if let Track::Subtitle(arc_track) = track {
                            Some(SubtitleSource {
                                track_index: idx,
                                track: arc_track,
                            })
                        } else {
                            None
                        }
                    })
                    .collect(),
                Err(e) => {
                    log::error!("Failed to create subtitle tracks: {:?}", e);
                    return;
                }
            }
        } else {
            // 视频/音频文件包含嵌入字幕：使用 detach_subtitle_tracks 分离字幕轨道
            log::info!("Detaching subtitle tracks from media file");

            match Track::new(&file_path, 1.0) {
                Ok(tracks) => {
                    let mut detached_tracks = Vec::new();
                    for (idx, track) in tracks.into_iter().enumerate() {
                        if let Track::Video(arc_video_track) = track {
                            // 从视频轨道中分离字幕轨道
                            let mut mutable_arc = arc_video_track;
                            let video_track = Arc::make_mut(&mut mutable_arc);
                            let subtitle_tracks_vec = video_track.detach_subtitle_tracks(1.0);
                            for subtitle_track in subtitle_tracks_vec {
                                detached_tracks.push(SubtitleSource {
                                    track_index: idx,
                                    track: Arc::new(subtitle_track),
                                });
                            }
                        }
                    }
                    detached_tracks
                }
                Err(e) => {
                    log::error!("Failed to create tracks: {:?}", e);
                    return;
                }
            }
        };

        if subtitle_tracks.is_empty() {
            log::warn!("No subtitle tracks created");
            return;
        }

        match UnifiedSubtitleTracksCompositorIterator::new(subtitle_tracks, Duration::ZERO) {
            Ok(mut iterator) => {
                log::info!("Iterator created successfully");

                // 获取特定时间点的字幕（使用字幕中间的时间点，而不是边界值）
                let test_timestamps = vec![
                    Duration::from_millis(500),   // 0.5s - 第一条字幕中间
                    Duration::from_millis(1500),  // 1.5s - 第二条字幕中间
                    Duration::from_millis(2500),  // 2.5s - 第三条字幕中间
                    Duration::from_secs_f64(4.5), // 4.5s - 最后一条字幕末尾
                    Duration::from_secs(10),      // 10s - 超出范围
                ];

                for timestamp in test_timestamps {
                    if let Some(subtitle) = iterator.get_subtitle_at(timestamp) {
                        log::info!(
                            "At {:.1}s: {:.1}s - {:.1}s: {}",
                            timestamp.as_secs_f64(),
                            subtitle.subtitle.start.as_secs_f64(),
                            subtitle.subtitle.end.as_secs_f64(),
                            subtitle.subtitle.text
                        );
                    } else {
                        log::info!("At {:.1}s: No subtitle", timestamp.as_secs_f64());
                    }
                }

                // 使用 iterator 遍历所有字幕
                log::info!("=== Iterating through all subtitles ===");
                iterator.set_timeline_offset(Duration::ZERO);
                let count = iterator.count();
                log::info!("Total entries via iterator: {}", count);
            }
            Err(e) => {
                log::error!("Failed to create iterator: {:?}", e);
            }
        }
    }

    log::info!("=== Test completed ===");
}
