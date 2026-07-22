//! Video scanning: decode frames, extract embeddings, and find matches.
//!
//! Scans one or more video files, sampling frames at a configurable interval,
//! and compares each sampled frame's CNN embedding against a query image.
//!
//! Consecutive matching frames are merged: when multiple frames near each other
//! in time all exceed the similarity threshold, only the one with the highest
//! similarity is kept. This prevents a single static region from producing
//! many duplicate export segments.

use crate::{
    cancel::{CancellationToken, check_cancelled},
    config::SimilarVideoConfig,
    embedder::ImageEmbedder,
    error::Result,
    progress::ScanProgress,
};
use ffmpeg_next as ffmpeg;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// A single match found during scanning.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Path of the video where the match was found.
    pub video_path: PathBuf,
    /// Timestamp of the matching frame within the video.
    pub match_time: Duration,
    /// Cosine similarity score (higher = more similar).
    pub similarity: f32,
    /// Frame number (0-based) of the matching frame.
    pub frame_number: usize,
}

/// Scan one or more videos for frames similar to the query image.
///
/// Consecutive matching frames (within `config.merge_gap_duration`) are
/// merged into a single `MatchResult` — keeping only the best match.
/// This avoids exporting many overlapping segments for the same static region.
///
/// # Arguments
/// * `query_image_path` — Path to the RGB/RGBA query image.
/// * `video_paths` — List of video file paths to scan.
/// * `config` — Scan and export configuration.
/// * `cancellation_token` — Optional token to cancel the operation.
/// * `progress_fn` — Callback invoked with scan progress after each sampled frame.
///
/// # Returns
/// A vector of `MatchResult` for all frames exceeding the similarity threshold,
/// sorted by similarity (descending).
pub fn scan_videos(
    query_image_path: &Path,
    video_paths: &[PathBuf],
    config: &SimilarVideoConfig,
    cancellation_token: Option<CancellationToken>,
    mut progress_fn: impl FnMut(ScanProgress),
) -> Result<Vec<MatchResult>> {
    let embedder = ImageEmbedder::from_image_path(query_image_path)?;

    let total_videos = video_paths.len();
    let start_time = Instant::now();
    let mut all_matches: Vec<MatchResult> = Vec::new();

    for (video_index, video_path) in video_paths.iter().enumerate() {
        check_cancelled(&cancellation_token)?;

        if !video_path.exists() {
            log::warn!("Video not found, skipping: {}", video_path.display());
            continue;
        }

        let raw_matches = scan_single_video(
            &embedder,
            video_path,
            video_index,
            total_videos,
            config,
            &cancellation_token,
            &mut progress_fn,
            start_time,
        )?;

        all_matches.extend(raw_matches);
    }

    // Merge consecutive matches — only keep the best one per cluster
    let merged = merge_consecutive_matches(&all_matches, config.merge_gap_duration);

    // Sort by similarity descending
    let mut sorted = merged;
    sorted.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(sorted)
}

/// Merge consecutive matches that belong to the same visual region.
///
/// When matches are close in time (within `gap`), they likely represent the
/// same static content. We keep only the best match from each cluster.
fn merge_consecutive_matches(matches: &[MatchResult], gap: Duration) -> Vec<MatchResult> {
    if matches.is_empty() {
        return Vec::new();
    }

    // Sort by video path then by match_time
    let mut sorted: Vec<&MatchResult> = matches.iter().collect();
    sorted.sort_by(|a, b| {
        a.video_path
            .cmp(&b.video_path)
            .then_with(|| a.match_time.cmp(&b.match_time))
    });

    let mut result: Vec<MatchResult> = Vec::new();
    let mut cluster: Vec<&MatchResult> = vec![sorted[0]];

    for m in sorted.iter().skip(1) {
        let prev = cluster.last().unwrap();

        // Same video and within gap → same cluster
        if m.video_path == prev.video_path && m.match_time <= prev.match_time + gap {
            cluster.push(m);
        } else {
            // Flush current cluster — keep the best match
            let best = cluster
                .iter()
                .max_by(|a, b| {
                    a.similarity
                        .partial_cmp(&b.similarity)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            result.push((*best).clone());
            cluster.clear();
            cluster.push(m);
        }
    }

    // Flush last cluster
    if !cluster.is_empty() {
        let best = cluster
            .iter()
            .max_by(|a, b| {
                a.similarity
                    .partial_cmp(&b.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        result.push((*best).clone());
    }

    result
}

/// Scan a single video file for matches.
fn scan_single_video(
    embedder: &ImageEmbedder,
    video_path: &Path,
    video_index: usize,
    total_videos: usize,
    config: &SimilarVideoConfig,
    cancellation_token: &Option<CancellationToken>,
    progress_fn: &mut impl FnMut(ScanProgress),
    start_time: Instant,
) -> Result<Vec<MatchResult>> {
    ffmpeg::init()
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let mut input_ctx = ffmpeg::format::input(video_path).map_err(|e| {
        crate::Error::FFmpeg(format!(
            "Failed to open video {}: {}",
            video_path.display(),
            e
        ))
    })?;

    // Find the best video stream
    let stream = input_ctx
        .streams()
        .find(|s| s.parameters().medium() == ffmpeg::media::Type::Video)
        .ok_or_else(|| crate::Error::NoVideoStream(video_path.to_path_buf()))?;

    let stream_index = stream.index();
    let time_base = stream.time_base();

    // Get video metadata
    let fps_rational = stream.avg_frame_rate();
    let fps = if fps_rational.numerator() > 0 && fps_rational.denominator() > 0 {
        fps_rational.numerator() as f32 / fps_rational.denominator() as f32
    } else {
        30.0 // fallback
    };

    let total_frames = if fps > 0.0 {
        let duration_micros = input_ctx.duration();
        if duration_micros > 0 {
            (duration_micros as f64 / 1_000_000.0 * fps as f64) as u64
        } else {
            // Estimate from stream duration
            let stream_dur_ts = stream.duration();
            if stream_dur_ts > 0 && time_base.denominator() > 0 {
                let secs = stream_dur_ts as f64 * time_base.numerator() as f64
                    / time_base.denominator() as f64;
                (secs * fps as f64) as u64
            } else {
                0
            }
        }
    } else {
        0
    };

    let codec_par = stream.parameters();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_par.clone())
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to create decoder: {}", e)))?
        .decoder()
        .video()
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to get video decoder: {}", e)))?;

    let mut matches: Vec<MatchResult> = Vec::new();
    let mut frames_processed: u64 = 0;
    let mut frame_count: usize = 0;
    let mut best_similarity: f32 = 0.0;
    let sample_interval = config.sample_interval.max(1) as usize;

    // Iterate over packets and decode
    for (stream_ref, packet) in input_ctx.packets() {
        check_cancelled(cancellation_token)?;

        if stream_ref.index() != stream_index {
            continue;
        }

        if let Err(e) = decoder.send_packet(&packet) {
            if !matches!(e, ffmpeg::Error::Other { .. }) {
                log::warn!("Error sending packet: {:?}", e);
            }
            continue;
        }

        let mut decoded_frame = ffmpeg::frame::Video::empty();
        loop {
            match decoder.receive_frame(&mut decoded_frame) {
                Ok(()) => {
                    frame_count += 1;

                    // Only process every N-th frame
                    if frame_count % sample_interval != 0 {
                        continue;
                    }

                    let similarity = process_frame(embedder, &decoded_frame);
                    frames_processed += 1;

                    if similarity > best_similarity {
                        best_similarity = similarity;
                    }

                    if similarity >= config.similarity_threshold {
                        let frame_time = frame_time_from_index(frame_count, fps);
                        matches.push(MatchResult {
                            video_path: video_path.to_path_buf(),
                            match_time: frame_time,
                            similarity,
                            frame_number: frame_count,
                        });
                    }

                    progress_fn(ScanProgress {
                        video_index,
                        total_videos,
                        current_video: video_path.to_path_buf(),
                        frames_processed,
                        total_frames,
                        best_similarity,
                        elapsed: start_time.elapsed(),
                    });
                }
                Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                Err(e) => {
                    log::warn!("Decoder error: {:?}", e);
                    break;
                }
            }
        }
    }

    // Flush decoder
    let _ = decoder.send_eof();
    let mut decoded_frame = ffmpeg::frame::Video::empty();
    loop {
        match decoder.receive_frame(&mut decoded_frame) {
            Ok(()) => {
                frame_count += 1;
                if frame_count % sample_interval != 0 {
                    continue;
                }

                let similarity = process_frame(embedder, &decoded_frame);
                frames_processed += 1;

                if similarity > best_similarity {
                    best_similarity = similarity;
                }

                if similarity >= config.similarity_threshold {
                    let frame_time = frame_time_from_index(frame_count, fps);
                    matches.push(MatchResult {
                        video_path: video_path.to_path_buf(),
                        match_time: frame_time,
                        similarity,
                        frame_number: frame_count,
                    });
                }

                progress_fn(ScanProgress {
                    video_index,
                    total_videos,
                    current_video: video_path.to_path_buf(),
                    frames_processed,
                    total_frames,
                    best_similarity,
                    elapsed: start_time.elapsed(),
                });
            }
            Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
            Err(_) => break,
        }
    }

    log::info!(
        "Scanned {} frames (sampled {}) from {}, found {} matches (best={:.3})",
        frame_count,
        frames_processed,
        video_path.display(),
        matches.len(),
        best_similarity,
    );

    Ok(matches)
}

/// Compute similarity between the query image and a decoded video frame.
fn process_frame(embedder: &ImageEmbedder, frame: &ffmpeg::frame::Video) -> f32 {
    match video_utils::ffmpeg::frame_to_rgba(frame) {
        Ok(rgba) => embedder.similarity(&rgba),
        Err(e) => {
            log::warn!("Frame conversion failed: {}", e);
            0.0
        }
    }
}

/// Calculate the timestamp for a given frame number.
fn frame_time_from_index(frame_number: usize, fps: f32) -> Duration {
    if fps > 0.0 {
        Duration::from_secs_f64(frame_number as f64 / fps as f64)
    } else {
        Duration::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_match(video: &str, time_secs: f64, similarity: f32, frame: usize) -> MatchResult {
        MatchResult {
            video_path: PathBuf::from(video),
            match_time: Duration::from_secs_f64(time_secs),
            similarity,
            frame_number: frame,
        }
    }

    #[test]
    fn test_merge_consecutive_no_merge() {
        let matches = vec![
            make_match("a.mp4", 1.0, 0.8, 10),
            make_match("a.mp4", 20.0, 0.85, 200),
            make_match("b.mp4", 5.0, 0.9, 50),
        ];
        let gap = Duration::from_secs(5);
        let merged = merge_consecutive_matches(&matches, gap);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_consecutive_same_cluster() {
        // Three matches in same video within 5s gap → only best kept
        let matches = vec![
            make_match("a.mp4", 1.0, 0.8, 10),
            make_match("a.mp4", 2.0, 0.9, 20),
            make_match("a.mp4", 3.0, 0.85, 30),
        ];
        let gap = Duration::from_secs(5);
        let merged = merge_consecutive_matches(&matches, gap);
        assert_eq!(merged.len(), 1);
        assert!((merged[0].similarity - 0.9).abs() < 0.001);
        assert_eq!(merged[0].frame_number, 20);
    }

    #[test]
    fn test_merge_consecutive_multiple_clusters() {
        // First cluster: 1s, 2s (within gap) → keep best
        // Second cluster: 15s (beyond gap from first) → keep
        // Third cluster: different video → keep
        let matches = vec![
            make_match("a.mp4", 1.0, 0.8, 10),
            make_match("a.mp4", 2.0, 0.9, 20),
            make_match("a.mp4", 15.0, 0.7, 150),
            make_match("b.mp4", 1.0, 0.85, 10),
        ];
        let gap = Duration::from_secs(5);
        let merged = merge_consecutive_matches(&matches, gap);
        assert_eq!(merged.len(), 3);
        // Cluster 1 best: 0.9 at frame 20
        assert_eq!(merged[0].frame_number, 20);
        // Cluster 2: 0.7 at frame 150
        assert_eq!(merged[1].frame_number, 150);
        // Cluster 3: different video, 0.85 at frame 10
        assert_eq!(merged[2].frame_number, 10);
    }

    #[test]
    fn test_merge_empty() {
        let merged = merge_consecutive_matches(&[], Duration::from_secs(5));
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_different_videos_not_merged() {
        let matches = vec![
            make_match("a.mp4", 1.0, 0.8, 10),
            make_match("b.mp4", 1.5, 0.85, 15),
        ];
        let gap = Duration::from_secs(5);
        let merged = merge_consecutive_matches(&matches, gap);
        assert_eq!(merged.len(), 2);
    }
}
