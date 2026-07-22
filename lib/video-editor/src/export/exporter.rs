use super::{
    config::Mp4ExportConfig,
    progress::{ExportPhase, ExportProgress},
};
use crate::{
    Error, Result,
    preview::cache::clear_global_audio_display_cache,
    tracks::{
        manager::Manager,
        subtitle_track::apply_segment_subtitle_filters,
        text_track::create_text_layer_frame,
        track::Track,
        unified_mixer::{UnifiedFrame, UnifiedMixerConfig},
        video_frame_cache::{
            clear_global_cache, get_global_cache_max_frames, set_global_cache_max_frames,
        },
        video_track::{apply_global_filters, composite_frame},
    },
};
use hound::WavSpec;
use image::RgbaImage;
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
pub struct ExportResult {
    pub output_path: PathBuf,
    pub duration: Duration,
    pub total_frames: u64,
}

pub struct Mp4Exporter {
    config: Mp4ExportConfig,
    manager: Arc<Manager>,
}

struct ExportState {
    output_width: u32,
    output_height: u32,
    output_fps: f32,
    audio_sample_rate: u32,
    audio_channels: u16,
    video_encoder: Arc<Mutex<Option<Box<dyn VideoEncoder>>>>,
    frames_processed: u64,
}

impl Mp4Exporter {
    pub fn new(manager: Manager, config: Mp4ExportConfig) -> Self {
        Self {
            config,
            manager: Arc::new(manager),
        }
    }

    pub fn new_arc(manager: Arc<Manager>, config: Mp4ExportConfig) -> Self {
        Self { config, manager }
    }

    pub fn export(&self) -> Result<ExportResult> {
        self.export_with_progress(|_progress| {})
    }

    fn check_cancelled(&self) -> Result<()> {
        if let Some(token) = &self.config.cancellation_token
            && token.is_cancelled()
        {
            return Err(Error::ExportCancelled);
        }
        Ok(())
    }

    pub fn export_with_progress<F>(&self, mut progress_fn: F) -> Result<ExportResult>
    where
        F: FnMut(ExportProgress),
    {
        let (output_width, output_height, output_fps) = self.detect_video_params()?;
        let (audio_sample_rate, audio_channels) = self.detect_audio_params()?;

        log::info!(
            "Export parameters: {}x{} @ {:.2}fps",
            output_width,
            output_height,
            output_fps
        );
        if audio_sample_rate > 0 {
            log::info!(
                "Audio: {} Hz, {} channels",
                audio_sample_rate,
                audio_channels
            );
        }

        self.report_progress(
            &mut progress_fn,
            ExportPhase::Initializing,
            0,
            self.manager.duration,
            output_fps,
        );

        let mut mp4_processor = Mp4Processor::new(
            Mp4ProcessorConfigBuilder::default()
                .save_path(self.config.output_path.clone())
                .video_config(VideoConfig {
                    width: output_width,
                    height: output_height,
                    fps: output_fps as u32,
                })
                .channel_size(1024)
                .build()
                .map_err(|e| Error::InvalidConfig(format!("MP4 config error: {}", e)))?,
        );

        let audio_sender = if audio_sample_rate > 0 {
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

        let video_encoder_config = VideoEncoderConfig::new(output_width, output_height)
            .with_fps(output_fps as u32)
            .with_preset(self.config.compression_preset)
            .with_tune(self.config.tune)
            .with_crf(if self.config.use_crf {
                self.config.crf
            } else {
                None
            });

        let video_encoder = Arc::new(Mutex::new(Some(
            video_encoder::new(video_encoder_config).map_err(|e| {
                Error::InvalidConfig(format!("Failed to create video encoder: {}", e))
            })?,
        )));

        let headers = {
            let mut enc = video_encoder.lock().unwrap();
            enc.as_mut().unwrap().headers().map_err(|e| {
                Error::InvalidConfig(format!("Failed to get encoder headers: {}", e))
            })?
        };

        log::info!(
            "Video encoder initialized, {} bytes of headers (AVCC format)",
            headers.len()
        );

        let h264_sender = mp4_processor.h264_sender();
        let audio_sender_clone = audio_sender.clone();

        let mut state = ExportState {
            output_width,
            output_height,
            output_fps,
            audio_sample_rate,
            audio_channels,
            video_encoder,
            frames_processed: 0,
        };

        let processor_thread =
            thread::spawn(move || mp4_processor.run_processing_loop(Some(headers)));

        let frames_processed = self.process_frames(
            &mut state,
            &h264_sender,
            audio_sender_clone.as_ref(),
            &mut progress_fn,
        )?;

        self.flush_video_encoder(&state.video_encoder, &h264_sender)?;
        h264_sender
            .send(VideoFrameType::End)
            .map_err(|e| Error::Sender(format!("Failed to send end signal: {}", e)))?;

        if let Some(audio_sender) = audio_sender_clone.as_ref() {
            audio_sender
                .send(AudioFrameType::End)
                .map_err(|e| Error::Sender(format!("Failed to send audio end signal: {}", e)))?;
            log::info!("Sent AudioFrameType::End signal");
        }

        log::info!("Encoding complete: {} video frames", frames_processed);

        self.report_progress(
            &mut progress_fn,
            ExportPhase::Finalizing,
            frames_processed,
            self.manager.duration,
            output_fps,
        );

        processor_thread
            .join()
            .map_err(|e| Error::InvalidConfig(format!("Processor thread error: {:?}", e)))?
            .map_err(|e| Error::InvalidConfig(format!("MP4 processing error: {}", e)))?;

        log::info!("MP4 file written to: {}", self.config.output_path.display());

        clear_global_cache();
        clear_global_audio_display_cache();

        self.report_progress(
            &mut progress_fn,
            ExportPhase::Complete,
            frames_processed,
            self.manager.duration,
            output_fps,
        );

        Ok(ExportResult {
            output_path: self.config.output_path.clone(),
            duration: self.manager.duration,
            total_frames: frames_processed,
        })
    }

    fn process_frames<F>(
        &self,
        state: &mut ExportState,
        h264_sender: &mp4m::Sender<VideoFrameType>,
        audio_sender: Option<&mp4m::Sender<AudioFrameType>>,
        progress_fn: &mut F,
    ) -> Result<u64>
    where
        F: FnMut(ExportProgress),
    {
        let original_cache_max_frames = get_global_cache_max_frames();
        if self.config.low_memory_mode {
            log::info!("Exporting in low memory mode - disabling global frame cache");
            set_global_cache_max_frames(0);
            clear_global_cache();
        }

        let (cache_duration, max_cache_duration) = if self.config.low_memory_mode {
            (Duration::from_secs(1), Duration::from_secs(2))
        } else {
            (Duration::from_secs(5), Duration::from_secs(10))
        };

        let mixer_iter = self.manager.unified_tracks_mixer_iter_with_config(
            UnifiedMixerConfig::default()
                .with_timeline_offset(Duration::ZERO)
                .with_cache_duration(cache_duration)
                .with_max_cache_duration(max_cache_duration)
                .with_output_width(Some(state.output_width))
                .with_output_height(Some(state.output_height))
                .with_output_fps(Some(state.output_fps))
                .with_output_channels(Some(state.audio_channels))
                .with_output_sample_rate(Some(state.audio_sample_rate)),
        )?;

        for unified_frame in mixer_iter {
            self.check_cancelled()?;
            self.process_single_frame(state, h264_sender, audio_sender, &unified_frame)?;

            self.report_progress(
                progress_fn,
                ExportPhase::EncodingVideo,
                state.frames_processed,
                self.manager.duration,
                state.output_fps,
            );
        }

        if self.config.low_memory_mode {
            set_global_cache_max_frames(original_cache_max_frames);
            log::info!(
                "Restored original cache settings (max_frames={})",
                original_cache_max_frames
            );
        } else {
            // Clear global caches after export to release decoded frame memory
            clear_global_cache();
            clear_global_audio_display_cache();
        }

        Ok(state.frames_processed)
    }

    fn process_single_frame(
        &self,
        state: &mut ExportState,
        h264_sender: &mp4m::Sender<VideoFrameType>,
        audio_sender: Option<&mp4m::Sender<AudioFrameType>>,
        unified_frame: &UnifiedFrame,
    ) -> Result<()> {
        // Only clone the composited image — avoid cloning all layer VideoImage buffers
        let mut composited_image = unified_frame
            .layer_frames
            .as_ref()
            .map(|lf| lf.composited_image.clone())
            .or_else(|| {
                if !unified_frame.text.is_empty() {
                    Some(RgbaImage::new(state.output_width, state.output_height))
                } else {
                    None
                }
            });

        if let Some(ref mut img) = composited_image {
            self.process_video_frame(state, h264_sender, img, unified_frame)?;
        }

        if let (Some(sender), Some(audio_data)) = (audio_sender, &unified_frame.audio) {
            if !audio_data.samples.is_empty() {
                sender
                    .send(AudioFrameType::Samples(audio_data.samples.clone()))
                    .map_err(|e| Error::Sender(format!("Failed to send audio: {}", e)))?;
                log::trace!("Sent {} audio samples to mp4m", audio_data.samples.len());
            }
        }

        Ok(())
    }

    fn process_video_frame(
        &self,
        state: &mut ExportState,
        h264_sender: &mp4m::Sender<VideoFrameType>,
        video_frame: &mut image::RgbaImage,
        unified_frame: &UnifiedFrame,
    ) -> Result<()> {
        for text in &unified_frame.text {
            if let Ok(text_layer) = create_text_layer_frame(
                &text.element,
                text.segment.clone(),
                text.segment_index,
                text.track_index,
                unified_frame.timeline_offset,
                video_frame.width(),
                video_frame.height(),
            ) {
                composite_frame(video_frame, &text_layer.image);
            }
        }

        if self.config.burn_subtitles {
            for sub in &unified_frame.subtitle {
                apply_segment_subtitle_filters(video_frame, &sub.subtitle, sub.segment.clone())
                    .map_err(|e| Error::InvalidConfig(format!("Failed to burn subtitle: {}", e)))?;
            }
        }

        // Apply post-composite global filters (e.g., rotation) after subtitle/text
        if !unified_frame.post_composite_global_filters.is_empty() {
            apply_global_filters(
                video_frame,
                &unified_frame.post_composite_global_filters,
                unified_frame.timeline_offset,
                unified_frame.duration,
                true,
            );
        }

        let rgb_frame = rgba_to_rgb(video_frame);

        {
            match state
                .video_encoder
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .encode_frame(rgb_frame)
                .map_err(|e| Error::InvalidConfig(format!("Video encoding failed: {}", e)))?
            {
                EncodedFrame::Frame {
                    timestamp: _,
                    data,
                    is_keyframe,
                } => {
                    h264_sender
                        .send(VideoFrameType::Frame {
                            data,
                            is_sync: is_keyframe,
                        })
                        .map_err(|e| Error::Sender(format!("Failed to send video frame: {}", e)))?;
                    state.frames_processed += 1;

                    if state.frames_processed.is_multiple_of(30) {
                        log::trace!("Encoded {} video frames", state.frames_processed);
                    }
                }
                EncodedFrame::Empty(_) => {}
                EncodedFrame::End => {}
            }
        }

        Ok(())
    }

    fn flush_video_encoder(
        &self,
        video_encoder: &Arc<Mutex<Option<Box<dyn video_encoder::VideoEncoder>>>>,
        h264_sender: &mp4m::Sender<VideoFrameType>,
    ) -> Result<()> {
        let flushed_packets = Arc::new(Mutex::new(Vec::new()));
        let frames_flushed = Arc::new(Mutex::new(0u64));

        let packets_ptr = Arc::clone(&flushed_packets);
        let flushed_ptr = Arc::clone(&frames_flushed);

        let flush_callback = move |data: Vec<u8>, is_keyframe: bool| {
            let mut packets = packets_ptr.lock().unwrap();
            let mut count = flushed_ptr.lock().unwrap();
            packets.push((data, is_keyframe));
            *count += 1;
        };

        let encoder = video_encoder
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| Error::InvalidConfig("Encoder not available".to_string()))?;

        encoder
            .flush(Box::new(flush_callback))
            .map_err(|e| Error::InvalidConfig(format!("Failed to flush encoder: {}", e)))?;

        let packets = flushed_packets.lock().unwrap();
        let count = *frames_flushed.lock().unwrap();
        log::info!("Flushing {} delayed frames", count);

        for (data, is_keyframe) in packets.iter() {
            h264_sender
                .send(VideoFrameType::Frame {
                    data: data.clone(),
                    is_sync: *is_keyframe,
                })
                .map_err(|e| Error::Sender(format!("Failed to send flushed frame: {}", e)))?;
        }

        Ok(())
    }

    fn detect_video_params(&self) -> Result<(u32, u32, f32)> {
        let detect_from_tracks = || -> Option<(u32, u32, f32)> {
            for track in &self.manager.tracks {
                if let Track::Video(video_track) = track
                    && !video_track.hiding
                    && let Some(video_meta) = video_track.track.metadata.videos.first()
                {
                    return Some((video_meta.width, video_meta.height, video_meta.fps));
                }
            }
            None
        };

        if self.config.width.is_some() && self.config.height.is_some() && self.config.fps.is_some()
        {
            return Ok((
                self.config.width.unwrap(),
                self.config.height.unwrap(),
                self.config.fps.unwrap() as f32,
            ));
        }

        if let Some((detected_w, detected_h, detected_f)) = detect_from_tracks() {
            let width = self.config.width.unwrap_or(detected_w);
            let height = self.config.height.unwrap_or(detected_h);
            let fps = self.config.fps.map(|f| f as f32).unwrap_or(detected_f);
            return Ok((width, height, fps));
        }

        let width = self.config.width.unwrap_or(1920);
        let height = self.config.height.unwrap_or(1080);
        let fps = self.config.fps.map(|f| f as f32).unwrap_or(25.0);
        Ok((width, height, fps))
    }

    fn detect_audio_params(&self) -> Result<(u32, u16)> {
        let detect_from_tracks = || -> Option<(u32, u16)> {
            for track in &self.manager.tracks {
                if let Track::Audio(audio_track) = track
                    && !audio_track.hiding
                    && let Some(audio_meta) = audio_track.track.metadata.audios.first()
                {
                    return Some((audio_meta.sample_rate, audio_meta.channels));
                }

                if let Track::Video(video_track) = track
                    && !video_track.hiding
                    && video_track.has_audio_in_any_segment()
                    && let Some(audio_meta) = video_track.first_audio_meta()
                {
                    return Some((audio_meta.sample_rate, audio_meta.channels));
                }
            }
            None
        };

        if self.config.audio_channels.is_some() && self.config.audio_sample_rate.is_some() {
            return Ok((
                self.config.audio_sample_rate.unwrap(),
                self.config.audio_channels.unwrap(),
            ));
        }

        if let Some((detected_sr, detected_ch)) = detect_from_tracks() {
            let sample_rate = self.config.audio_sample_rate.unwrap_or(detected_sr);
            let channels = self.config.audio_channels.unwrap_or(detected_ch);
            return Ok((sample_rate, channels));
        }

        Ok((0, 0))
    }

    fn report_progress<F>(
        &self,
        progress_fn: &mut F,
        phase: ExportPhase,
        frames_processed: u64,
        total_duration: Duration,
        output_fps: f32,
    ) where
        F: FnMut(ExportProgress),
    {
        let total_frames = (total_duration.as_secs_f64() * output_fps as f64) as u64;

        let progress = ExportProgress {
            current_position: Duration::from_secs_f64(
                frames_processed as f64 / total_frames.max(1) as f64 * total_duration.as_secs_f64(),
            ),
            total_duration,
            frames_processed,
            total_frames,
            phase,
        };

        progress_fn(progress);
    }
}
