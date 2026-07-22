//! Export video segments around match positions.
//!
//! For each match, decodes the video from `(match_time - before_duration)` to
//! `(match_time + after_duration)`, re-encodes as H.264, and writes to an MP4 file.

use crate::{
    cancel::{CancellationToken, check_cancelled},
    config::SimilarVideoConfig,
    error::Result,
    progress::ExportProgress,
    scanner::MatchResult,
};
use ffmpeg_next as ffmpeg;
use hound::WavSpec;
use mp4m::{
    AudioConfig, AudioFrameType, Mp4Processor, Mp4ProcessorConfigBuilder, VideoConfig,
    VideoFrameType,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use video_encoder::{EncodedFrame, VideoEncoder, VideoEncoderConfig};

/// Export video segments for all match results.
///
/// For each match, a video clip is exported covering
/// `[match_time - before_duration, match_time + after_duration]`.
///
/// # Arguments
/// * `matches` — Match results from scanning.
/// * `config` — Scan and export configuration.
/// * `cancellation_token` — Optional token to cancel the operation.
/// * `progress_fn` — Callback invoked with export progress.
/// * `keep_audio` — If true, retain the original audio track in exported segments.
///
/// # Returns
/// Paths of the exported MP4 files.
pub fn export_segments(
    matches: &[MatchResult],
    config: &SimilarVideoConfig,
    cancellation_token: Option<CancellationToken>,
    mut progress_fn: impl FnMut(ExportProgress),
    keep_audio: bool,
) -> Result<Vec<PathBuf>> {
    let total_segments = matches.len();
    let mut exported_paths = Vec::with_capacity(total_segments);

    std::fs::create_dir_all(&config.output_dir)?;

    for (segment_index, match_result) in matches.iter().enumerate() {
        check_cancelled(&cancellation_token)?;

        let output_path =
            generate_output_path(&match_result.video_path, segment_index, &config.output_dir);

        match export_single_segment(match_result, config, &output_path, keep_audio) {
            Ok(frame_count) => {
                log::info!(
                    "Exported segment {} ({} frames) to {}",
                    segment_index,
                    frame_count,
                    output_path.display()
                );
                exported_paths.push(output_path);
            }
            Err(e) => {
                log::warn!(
                    "Failed to export segment {} from {}: {}",
                    segment_index,
                    match_result.video_path.display(),
                    e
                );
            }
        }

        progress_fn(ExportProgress {
            segment_index,
            total_segments,
            frames_processed: 0,
            total_frames: 0,
        });
    }

    Ok(exported_paths)
}

/// Export a single video segment around a match position.
fn export_single_segment(
    match_result: &MatchResult,
    config: &SimilarVideoConfig,
    output_path: &Path,
    keep_audio: bool,
) -> Result<u64> {
    ffmpeg::init()
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let video_path = &match_result.video_path;
    if !video_path.exists() {
        return Err(crate::Error::VideoNotFound(video_path.clone()));
    }

    let input_ctx = ffmpeg::format::input(video_path)
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to open video: {}", e)))?;

    // Find video stream
    let stream = input_ctx
        .streams()
        .find(|s| s.parameters().medium() == ffmpeg::media::Type::Video)
        .ok_or_else(|| crate::Error::NoVideoStream(video_path.clone()))?;

    let stream_index = stream.index();
    let time_base = stream.time_base();

    let fps_rational = stream.avg_frame_rate();
    let fps = if fps_rational.numerator() > 0 && fps_rational.denominator() > 0 {
        fps_rational.numerator() as f32 / fps_rational.denominator() as f32
    } else {
        30.0
    };

    let codec_par = stream.parameters();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(codec_par.clone())
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to create decoder: {}", e)))?
        .decoder()
        .video()
        .map_err(|e| crate::Error::FFmpeg(format!("Failed to get video decoder: {}", e)))?;

    let width = decoder.width();
    let height = decoder.height();
    let fps_u32 = fps as u32;

    // Calculate export time range
    let start_time = match_result
        .match_time
        .saturating_sub(config.before_duration);
    let end_time = match_result.match_time + config.after_duration;

    // Seek to start position
    let mut input_ctx = input_ctx;
    video_utils::ffmpeg::seek_to_time(&mut input_ctx, start_time);

    // Setup MP4 processor
    let mut mp4_processor = Mp4Processor::new(
        Mp4ProcessorConfigBuilder::default()
            .save_path(output_path.to_path_buf())
            .video_config(VideoConfig {
                width,
                height,
                fps: fps_u32,
            })
            .channel_size(1024)
            .build()
            .map_err(|e| crate::Error::Mp4Export(format!("MP4 config error: {}", e)))?,
    );

    // Check for audio stream (only set up if keep_audio is true)
    let mut audio_decoder: Option<ffmpeg::decoder::Audio> = None;
    let mut audio_stream_index: Option<usize> = None;
    let mut audio_time_base: Option<ffmpeg::Rational> = None;
    let audio_sender = if keep_audio
        && let Some(audio_s) = input_ctx
            .streams()
            .find(|s| s.parameters().medium() == ffmpeg::media::Type::Audio)
    {
        let audio_codec_par = audio_s.parameters();
        audio_stream_index = Some(audio_s.index());
        audio_time_base = Some(audio_s.time_base());

        if let Ok(ctx) = ffmpeg::codec::context::Context::from_parameters(audio_codec_par.clone())
            && let Ok(ad) = ctx.decoder().audio()
        {
            let channels = ad.channels();
            let sample_rate = ad.rate();
            let sender = mp4_processor
                .add_audio_track(AudioConfig {
                    convert_to_mono: false,
                    spec: WavSpec {
                        channels,
                        sample_rate,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    },
                })
                .map_err(|e| {
                    crate::Error::Mp4Export(format!("Failed to add audio track: {}", e))
                })?;
            audio_decoder = Some(ad);
            Some(sender)
        } else {
            None
        }
    } else {
        None
    };

    // Setup video encoder
    let video_encoder_config = VideoEncoderConfig::new(width, height).with_fps(fps_u32);
    let mut video_encoder: Box<dyn VideoEncoder> = video_encoder::new(video_encoder_config)
        .map_err(|e| crate::Error::VideoEncode(format!("Failed to create video encoder: {}", e)))?;

    let headers = video_encoder
        .headers()
        .map_err(|e| crate::Error::VideoEncode(format!("Failed to get encoder headers: {}", e)))?;

    let h264_sender = mp4_processor.h264_sender();
    let audio_sender_clone = audio_sender.clone();

    let processor_thread = thread::spawn(move || mp4_processor.run_processing_loop(Some(headers)));

    // Decode and encode video frames
    let mut frames_processed: u64 = 0;
    let duration_secs = (end_time - start_time).as_secs_f64();
    let _total_frames = (duration_secs * fps as f64) as u64;

    for (stream_ref, packet) in input_ctx.packets() {
        // Handle audio packets: decode and forward to MP4 processor
        if let Some(audio_idx) = audio_stream_index {
            if stream_ref.index() == audio_idx {
                if let Some(ref mut ad) = audio_decoder {
                    if let Err(e) = ad.send_packet(&packet) {
                        if !matches!(e, ffmpeg::Error::Other { .. }) {
                            log::warn!("Error sending audio packet: {:?}", e);
                        }
                    } else {
                        let mut audio_frame = ffmpeg::frame::Audio::empty();
                        loop {
                            match ad.receive_frame(&mut audio_frame) {
                                Ok(()) => {
                                    // Check audio frame time range
                                    let audio_tb = audio_time_base.unwrap();
                                    let frame_time = match audio_frame.timestamp() {
                                        Some(ts) if ts >= 0 => Duration::from_secs_f64(
                                            ts as f64 * audio_tb.numerator() as f64
                                                / audio_tb.denominator() as f64,
                                        ),
                                        _ => {
                                            // Fall back to PTS
                                            let pts = audio_frame.pts();
                                            match pts {
                                                Some(p) if p >= 0 => Duration::from_secs_f64(
                                                    p as f64 * audio_tb.numerator() as f64
                                                        / audio_tb.denominator() as f64,
                                                ),
                                                _ => Duration::ZERO,
                                            }
                                        }
                                    };

                                    if frame_time < start_time {
                                        continue;
                                    }
                                    if frame_time > end_time {
                                        break;
                                    }

                                    // Convert audio frame to interleaved f32 samples
                                    let samples =
                                        video_utils::ffmpeg::audio_frame_to_f32(&audio_frame);
                                    if !samples.is_empty() {
                                        if let Some(ref sender) = audio_sender_clone {
                                            if sender
                                                .send(AudioFrameType::Samples(samples))
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                                Err(e) => {
                                    log::warn!("Audio decoder error: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
                continue;
            }
        }

        if stream_ref.index() != stream_index {
            continue;
        }

        // Check if we've gone past the end time
        let packet_time = match packet.dts() {
            Some(dts) if dts >= 0 => Duration::from_secs_f64(
                dts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64,
            ),
            _ => {
                let _ = decoder.send_packet(&packet);
                continue;
            }
        };

        if packet_time > end_time {
            break;
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
                    let frame_time = match decoded_frame.timestamp() {
                        Some(ts) if ts >= 0 => Duration::from_secs_f64(
                            ts as f64 * time_base.numerator() as f64
                                / time_base.denominator() as f64,
                        ),
                        _ => continue,
                    };

                    if frame_time < start_time {
                        continue;
                    }
                    if frame_time > end_time {
                        break;
                    }

                    match video_utils::ffmpeg::frame_to_rgb(&decoded_frame) {
                        Ok(rgb_image) => match video_encoder.encode_frame(rgb_image) {
                            Ok(EncodedFrame::Frame {
                                data, is_keyframe, ..
                            }) => {
                                if h264_sender
                                    .send(VideoFrameType::Frame {
                                        data,
                                        is_sync: is_keyframe,
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                                frames_processed += 1;
                            }
                            Ok(EncodedFrame::Empty(_)) => {}
                            Ok(EncodedFrame::End) => break,
                            Err(e) => {
                                log::warn!("Video encoding failed: {:?}", e);
                            }
                        },
                        Err(e) => {
                            log::warn!("Frame conversion failed: {}", e);
                        }
                    }
                }
                Err(ffmpeg::Error::Other { .. }) | Err(ffmpeg::Error::Eof) => break,
                Err(e) => {
                    log::warn!("Decoder error: {:?}", e);
                    break;
                }
            }
        }
    }

    // Flush video encoder
    let flushed_packets: Arc<Mutex<Vec<(Vec<u8>, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    let flushed_ptr = Arc::clone(&flushed_packets);

    video_encoder
        .flush(Box::new(move |data: Vec<u8>, is_keyframe: bool| {
            let mut packets = flushed_ptr.lock().unwrap();
            packets.push((data, is_keyframe));
        }))
        .map_err(|e| crate::Error::VideoEncode(format!("Failed to flush encoder: {}", e)))?;

    let packets = flushed_packets.lock().unwrap();
    for (data, is_keyframe) in packets.iter() {
        if h264_sender
            .send(VideoFrameType::Frame {
                data: data.clone(),
                is_sync: *is_keyframe,
            })
            .is_err()
        {
            break;
        }
    }

    // Send end signals
    let _ = h264_sender.send(VideoFrameType::End);
    if let Some(ref as_ref) = audio_sender_clone {
        let _ = as_ref.send(AudioFrameType::End);
    }

    // Wait for MP4 processor
    processor_thread
        .join()
        .map_err(|e| crate::Error::Mp4Export(format!("Processor thread error: {:?}", e)))?
        .map_err(|e| crate::Error::Mp4Export(format!("MP4 processing error: {}", e)))?;

    Ok(frames_processed)
}

/// Generate an output file path for a segment.
fn generate_output_path(video_path: &Path, segment_index: usize, output_dir: &Path) -> PathBuf {
    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("segment");
    let filename = format!("{}_match_{:03}.mp4", stem, segment_index);
    output_dir.join(filename)
}
