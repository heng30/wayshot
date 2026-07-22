use super::progress::{CancellationToken, ExportPhase, ExportProgress};
use crate::{
    Error, Result,
    tracks::{audio_track::extract_segment_audio, segment::Segment, video_frame_cache::VideoImage},
};
use hound::WavSpec;
use mp4m::{
    AudioConfig, AudioFrameType, Mp4Processor, Mp4ProcessorConfigBuilder, VideoConfig,
    VideoFrameType,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use video_encoder::{EncodedFrame, VideoEncoder, VideoEncoderConfig};
use video_utils::convert::rgba_to_rgb;

#[derive(Debug, Clone)]
pub struct SegmentExportResult {
    pub output_path: PathBuf,
    pub total_frames: u64,
}

/// Configuration for exporting a single segment as MP4.
#[derive(Debug, Clone)]
pub struct SegmentExportConfig {
    pub output_path: PathBuf,
    pub cancellation_token: Option<CancellationToken>,
}

impl SegmentExportConfig {
    pub fn new(output_path: std::path::PathBuf) -> Self {
        Self {
            output_path,
            cancellation_token: None,
        }
    }

    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }
}

/// Export a single segment as an MP4 file.
///
/// The segment's video frames are extracted chunk by chunk (1 second at a time)
/// to limit memory usage. If the segment has audio and is not muted, the audio
/// is also included in the output MP4.
///
/// No filters are applied during export.
pub struct SegmentExporter;

impl SegmentExporter {
    /// Export a single segment as MP4.
    pub fn export(
        segment: &Arc<Segment>,
        config: SegmentExportConfig,
    ) -> Result<SegmentExportResult> {
        Self::export_with_progress(segment, config, |_| {})
    }

    /// Export a single segment as MP4 with progress reporting.
    pub fn export_with_progress<F>(
        segment: &Arc<Segment>,
        config: SegmentExportConfig,
        mut progress_fn: F,
    ) -> Result<SegmentExportResult>
    where
        F: FnMut(ExportProgress),
    {
        let video_meta = segment
            .metadata
            .first_video()
            .ok_or_else(|| Error::InvalidConfig("No video stream found in segment".into()))?;

        let source_fps = video_meta.fps as f64;
        let width = video_meta.width;
        let height = video_meta.height;
        let fps = source_fps as u32;
        let duration_secs = segment.duration.as_secs_f64();
        let start_frame = (segment.source_offset.as_secs_f64() * source_fps) as usize;

        // Audio info
        let audio_meta = segment.metadata.audios.first().cloned();
        let has_audio = audio_meta.is_some() && !segment.audio_muted;
        let audio_sample_rate = audio_meta.as_ref().map(|m| m.sample_rate).unwrap_or(0);
        let audio_channels = audio_meta.as_ref().map(|m| m.channels).unwrap_or(0);

        Self::report_progress(
            &mut progress_fn,
            ExportPhase::Initializing,
            0,
            duration_secs,
            fps,
        );

        // Setup MP4 processor
        let mut mp4_processor = Mp4Processor::new(
            Mp4ProcessorConfigBuilder::default()
                .save_path(config.output_path.clone())
                .video_config(VideoConfig { width, height, fps })
                .channel_size(1024)
                .build()
                .map_err(|e| Error::InvalidConfig(format!("MP4 config error: {}", e)))?,
        );

        let audio_sender = if has_audio && audio_sample_rate > 0 {
            Some(
                mp4_processor
                    .add_audio_track(AudioConfig {
                        convert_to_mono: false,
                        spec: WavSpec {
                            channels: audio_channels,
                            sample_rate: audio_sample_rate,
                            bits_per_sample: 32,
                            sample_format: hound::SampleFormat::Float,
                        },
                    })
                    .map_err(|e| {
                        Error::InvalidConfig(format!("Failed to add audio track: {}", e))
                    })?,
            )
        } else {
            None
        };

        // Setup video encoder
        let video_encoder_config = VideoEncoderConfig::new(width, height).with_fps(fps);
        let mut video_encoder: Box<dyn VideoEncoder> = video_encoder::new(video_encoder_config)
            .map_err(|e| Error::InvalidConfig(format!("Failed to create video encoder: {}", e)))?;

        let headers = video_encoder
            .headers()
            .map_err(|e| Error::InvalidConfig(format!("Failed to get encoder headers: {}", e)))?;

        let h264_sender = mp4_processor.h264_sender();
        let audio_sender_clone = audio_sender.clone();

        let processor_thread =
            thread::spawn(move || mp4_processor.run_processing_loop(Some(headers)));

        // Process video frames chunk by chunk (1 second at a time) to limit memory usage
        let frames_per_chunk = source_fps.ceil() as usize;
        let total_chunks = duration_secs.ceil() as usize;

        // Extract and encode audio for the full segment if available
        if let (Some(audio_sender_ref), Some(audio_m)) = (&audio_sender_clone, &audio_meta) {
            Self::report_progress(
                &mut progress_fn,
                ExportPhase::ProcessingAudio,
                0,
                duration_secs,
                fps,
            );

            let audio_result = extract_segment_audio(
                &segment.metadata.path,
                audio_m.index,
                segment,
                segment.timeline_offset,
                segment.duration,
                audio_m.channels,
                audio_m.sample_rate,
                audio_channels,
                audio_sample_rate,
            );

            match audio_result {
                Ok(segment_samples) => {
                    let samples: Vec<f32> = segment_samples
                        .samples
                        .into_iter()
                        .filter_map(|s| s)
                        .collect();

                    if !samples.is_empty() {
                        // Send audio samples in chunks (1 second per chunk)
                        let samples_per_chunk =
                            audio_sample_rate as usize * audio_channels as usize;
                        for chunk in samples.chunks(samples_per_chunk) {
                            Self::check_cancelled(&config.cancellation_token)?;
                            if let Err(e) =
                                audio_sender_ref.send(AudioFrameType::Samples(chunk.to_vec()))
                            {
                                log::warn!("Failed to send audio chunk: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to extract audio for MP4 export: {}", e);
                }
            }
        }

        // Process video frames chunk by chunk
        let mut frames_processed: u64 = 0;

        for chunk_idx in 0..total_chunks {
            Self::check_cancelled(&config.cancellation_token)?;

            let chunk_start_frame = start_frame + (chunk_idx * frames_per_chunk);
            let remaining_frames =
                ((duration_secs - chunk_idx as f64) * source_fps).ceil() as usize;
            let chunk_frame_count = frames_per_chunk.min(remaining_frames);

            let chunk_frames = segment.extract_video(chunk_start_frame, chunk_frame_count)?;

            for vi in chunk_frames {
                match vi {
                    VideoImage::Image { buffer } => {
                        let rgb_frame = rgba_to_rgb(&buffer);
                        match video_encoder.encode_frame(rgb_frame).map_err(|e| {
                            Error::InvalidConfig(format!("Video encoding failed: {}", e))
                        })? {
                            EncodedFrame::Frame {
                                data, is_keyframe, ..
                            } => {
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
                            EncodedFrame::Empty(_) => {}
                            EncodedFrame::End => break,
                        }
                    }
                    VideoImage::Empty => {}
                }
            }

            Self::report_progress(
                &mut progress_fn,
                ExportPhase::EncodingVideo,
                frames_processed,
                duration_secs,
                fps,
            );
        }

        // Flush video encoder
        let flushed_packets: Arc<Mutex<Vec<(Vec<u8>, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let flushed_ptr = Arc::clone(&flushed_packets);

        video_encoder
            .flush(Box::new(move |data: Vec<u8>, is_keyframe: bool| {
                let mut packets = flushed_ptr.lock().unwrap();
                packets.push((data, is_keyframe));
            }))
            .map_err(|e| Error::InvalidConfig(format!("Failed to flush encoder: {}", e)))?;

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
        if h264_sender.send(VideoFrameType::End).is_err() {
            log::warn!("Failed to send video end signal");
        }

        if let Some(ref as_ref) = audio_sender_clone {
            if as_ref.send(AudioFrameType::End).is_err() {
                log::warn!("Failed to send audio end signal");
            }
        }

        Self::report_progress(
            &mut progress_fn,
            ExportPhase::Finalizing,
            frames_processed,
            duration_secs,
            fps,
        );

        // Wait for MP4 processor to finish
        processor_thread
            .join()
            .map_err(|e| Error::InvalidConfig(format!("Processor thread error: {:?}", e)))?
            .map_err(|e| Error::InvalidConfig(format!("MP4 processing error: {}", e)))?;

        Self::report_progress(
            &mut progress_fn,
            ExportPhase::Complete,
            frames_processed,
            duration_secs,
            fps,
        );

        Ok(SegmentExportResult {
            output_path: config.output_path,
            total_frames: frames_processed,
        })
    }

    fn check_cancelled(token: &Option<CancellationToken>) -> Result<()> {
        if let Some(t) = token {
            if t.is_cancelled() {
                return Err(Error::ExportCancelled);
            }
        }
        Ok(())
    }

    fn report_progress<F>(
        progress_fn: &mut F,
        phase: ExportPhase,
        frames_processed: u64,
        total_duration_secs: f64,
        fps: u32,
    ) where
        F: FnMut(ExportProgress),
    {
        let total_duration = Duration::from_secs_f64(total_duration_secs);
        let total_frames = (total_duration_secs * fps as f64) as u64;
        progress_fn(ExportProgress {
            current_position: Duration::from_secs_f64(
                frames_processed as f64 / total_frames.max(1) as f64 * total_duration_secs,
            ),
            total_duration,
            frames_processed,
            total_frames,
            phase,
        });
    }
}
