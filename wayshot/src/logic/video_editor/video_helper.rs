use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use image::{RgbImage, RgbaImage};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use video_editor::tracks::segment::{
    convert_frame_to_image, initialize_video_decoder, seek_to_frame,
};

pub struct SegmentEncoderState {
    video_encoder: Arc<Mutex<Option<Box<dyn video_encoder::VideoEncoder>>>>,
    processor_thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    h264_sender: mp4m::Sender<mp4m::VideoFrameType>,
    audio_sender: Option<mp4m::Sender<mp4m::AudioFrameType>>,
    pub output_path: PathBuf,
}

impl SegmentEncoderState {
    pub fn new(
        width: u32,
        height: u32,
        fps: f32,
        audio_sample_rate: Option<u32>,
        audio_channels: Option<u16>,
        output_path: &PathBuf,
    ) -> Result<Self> {
        let mut mp4_processor = mp4m::Mp4Processor::new(
            mp4m::Mp4ProcessorConfigBuilder::default()
                .save_path(output_path.clone())
                .video_config(mp4m::VideoConfig {
                    width,
                    height,
                    fps: fps as u32,
                })
                .channel_size(1024)
                .build()
                .map_err(|e| anyhow::anyhow!("MP4 config error: {}", e))?,
        );

        let audio_sender = if let (Some(sr), Some(ch)) = (audio_sample_rate, audio_channels) {
            Some(
                mp4_processor
                    .add_audio_track(mp4m::AudioConfig {
                        convert_to_mono: false,
                        spec: hound::WavSpec {
                            channels: ch,
                            sample_rate: sr,
                            bits_per_sample: 32,
                            sample_format: hound::SampleFormat::Float,
                        },
                    })
                    .map_err(|e| anyhow::anyhow!("Failed to add audio track: {}", e))?,
            )
        } else {
            None
        };

        let video_encoder_config =
            video_encoder::VideoEncoderConfig::new(width, height).with_fps(fps as u32);

        let mut video_encoder: Box<dyn video_encoder::VideoEncoder> =
            video_encoder::new(video_encoder_config)
                .map_err(|e| anyhow::anyhow!("Failed to create video encoder: {}", e))?;

        let headers = video_encoder
            .headers()
            .map_err(|e| anyhow::anyhow!("Failed to get encoder headers: {}", e))?;

        let video_encoder = Arc::new(Mutex::new(Some(video_encoder)));
        let h264_sender = mp4_processor.h264_sender();
        let _audio_sender_clone = audio_sender.clone();

        let processor_thread = std::thread::spawn(move || {
            mp4_processor
                .run_processing_loop(Some(headers))
                .map_err(|e| e.to_string())
        });

        Ok(Self {
            video_encoder,
            processor_thread: Some(processor_thread),
            h264_sender,
            audio_sender,
            output_path: output_path.clone(),
        })
    }

    pub fn encode_frame(&mut self, rgb_frame: &RgbImage) -> Result<()> {
        let encoded = self
            .video_encoder
            .lock()
            .unwrap()
            .as_mut()
            .context("Encoder not available")?
            .encode_frame(rgb_frame.clone())
            .map_err(|e| anyhow::anyhow!("Video encoding failed: {}", e))?;

        match encoded {
            video_encoder::EncodedFrame::Frame {
                data, is_keyframe, ..
            } => {
                self.h264_sender
                    .send(mp4m::VideoFrameType::Frame {
                        data,
                        is_sync: is_keyframe,
                    })
                    .map_err(|e| anyhow::anyhow!("Failed to send video frame: {}", e))?;
            }
            video_encoder::EncodedFrame::Empty(_) => {}
            video_encoder::EncodedFrame::End => {}
        }

        Ok(())
    }

    /// Send a chunk of interleaved f32 audio samples to the audio encoder.
    pub fn send_audio_chunk(&self, chunk: Vec<f32>) -> Result<()> {
        if let Some(audio_sender) = &self.audio_sender {
            audio_sender
                .send(mp4m::AudioFrameType::Samples(chunk))
                .map_err(|e| anyhow::anyhow!("Failed to send audio samples: {}", e))?;
        }
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<()> {
        // Flush encoder
        let encoder = self
            .video_encoder
            .lock()
            .unwrap()
            .take()
            .context("Encoder not available")?;

        let flushed = Arc::new(Mutex::new(Vec::new()));
        let flushed_ptr = flushed.clone();

        encoder
            .flush(Box::new(move |data: Vec<u8>, is_keyframe: bool| {
                flushed_ptr.lock().unwrap().push((data, is_keyframe));
            }))
            .map_err(|e| anyhow::anyhow!("Encoder flush failed: {}", e))?;

        for (data, is_keyframe) in flushed.lock().unwrap().drain(..) {
            self.h264_sender
                .send(mp4m::VideoFrameType::Frame {
                    data,
                    is_sync: is_keyframe,
                })
                .map_err(|e| anyhow::anyhow!("Failed to send flushed frame: {}", e))?;
        }

        // Send end signals
        self.h264_sender
            .send(mp4m::VideoFrameType::End)
            .map_err(|e| anyhow::anyhow!("Failed to send video end: {}", e))?;

        if let Some(audio_sender) = &self.audio_sender {
            audio_sender
                .send(mp4m::AudioFrameType::End)
                .map_err(|e| anyhow::anyhow!("Failed to send audio end: {}", e))?;
        }

        // Wait for processor thread
        if let Some(thread) = self.processor_thread.take() {
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("Processor thread panicked"))?
                .map_err(|e| anyhow::anyhow!("MP4 processing error: {}", e))?;
        }

        Ok(())
    }
}

pub fn extract_frame_at_time(
    path: &PathBuf,
    stream_index: usize,
    target_time: Duration,
) -> Option<RgbaImage> {
    let (mut input_ctx, mut decoder, time_base) =
        initialize_video_decoder(path, stream_index).ok()?;

    if target_time > Duration::ZERO {
        seek_to_frame(&mut input_ctx, path, target_time, time_base);
    }

    for (stream_idx, packet) in input_ctx.packets() {
        if stream_idx.index() != stream_index {
            continue;
        }

        decoder.send_packet(&packet).ok()?;

        let mut decoded = ffmpeg::frame::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let frame_time = Duration::from_secs_f64(
                decoded.pts()? as f64 * time_base.numerator() as f64
                    / time_base.denominator() as f64,
            );
            if frame_time >= target_time {
                let video_image = convert_frame_to_image(&decoded).ok()?;
                return match video_image {
                    video_editor::tracks::video_frame_cache::VideoImage::Image { buffer } => {
                        Some(buffer)
                    }
                    _ => None,
                };
            }
        }
    }

    None
}

pub fn extract_frames_for_duration(
    path: &PathBuf,
    stream_index: usize,
    start_time: Duration,
    frame_count: u64,
    _output_width: u32,
    _output_height: u32,
) -> Vec<RgbImage> {
    let Ok((mut input_ctx, mut decoder, time_base)) = initialize_video_decoder(path, stream_index)
    else {
        return vec![];
    };

    let end_time = start_time + Duration::from_secs(1);

    if start_time > Duration::ZERO {
        seek_to_frame(&mut input_ctx, path, start_time, time_base);
    }

    let mut frames = Vec::new();
    let mut collected = 0u64;

    for (stream_idx, packet) in input_ctx.packets() {
        if stream_idx.index() != stream_index {
            continue;
        }

        if let Err(_) = decoder.send_packet(&packet) {
            continue;
        }

        let mut decoded = ffmpeg::frame::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let frame_time = match decoded.pts() {
                Some(pts) => Duration::from_secs_f64(
                    pts as f64 * time_base.numerator() as f64 / time_base.denominator() as f64,
                ),
                None => continue,
            };

            if frame_time < start_time {
                continue;
            }
            if frame_time >= end_time {
                return frames;
            }

            if let Ok(video_image) = convert_frame_to_image(&decoded) {
                match video_image {
                    video_editor::tracks::video_frame_cache::VideoImage::Image { buffer } => {
                        // Convert RgbaImage to RgbImage
                        let rgb = image::DynamicImage::ImageRgba8(buffer).to_rgb8();
                        frames.push(rgb);
                        collected += 1;
                        if collected >= frame_count {
                            return frames;
                        }
                    }
                    _ => continue,
                }
            }
        }
    }

    frames
}

/// Extract all audio samples from a video/audio file as interleaved f32.
/// Uses symphonia (via audio_utils) for reliable decoding with correct
/// sample rate handling, avoiding the ffmpeg resampler speed issues.
/// Returns (channels, sample_rate, samples) or None on failure.
pub fn extract_all_audio_samples(
    path: &PathBuf,
    _stream_index: usize,
) -> Option<(u16, u32, Vec<f32>)> {
    let audio_config = audio_utils::loader::load_audio_file(path).ok()?;
    if audio_config.samples.is_empty() {
        return None;
    }
    Some((
        audio_config.channel,
        audio_config.sample_rate,
        audio_config.samples,
    ))
}
