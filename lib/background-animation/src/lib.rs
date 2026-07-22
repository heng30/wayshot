pub mod aa_line;
pub mod black_hole;
pub mod bokeh;
pub mod cross_line;
pub mod flow_field;
pub mod fluid;
pub mod galaxy;
pub mod glitch;
pub mod grid;
pub mod ink;
pub mod kaleidoscope;
pub mod light_effects;
pub mod matrix_rain;
pub mod moving_grid;
pub mod noise_flow;
pub mod particle_life;
pub mod particle_network;
pub mod shape;
pub mod triangle;
pub mod wave;

pub(crate) mod noise;

pub use crossbeam;

/// Base height for pixel scaling (1080P standard)
pub const BASE_HEIGHT: u32 = 1080;

/// Scale pixel value based on target height (1080P standard)
/// This ensures consistent visual sizes across different resolutions
pub fn scale_pixel_for_height(value: f32, target_height: u32) -> f32 {
    let scale = target_height as f32 / BASE_HEIGHT as f32;
    value * scale
}

/// Scale u32 pixel value based on target height (1080P standard)
pub fn scale_pixel_u32_for_height(value: u32, target_height: u32) -> u32 {
    let scale = target_height as f32 / BASE_HEIGHT as f32;
    (value as f32 * scale).round() as u32
}

use crossbeam::channel::{self, Receiver, Sender};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use strum_macros::{Display, EnumString};

pub type Result<T> = std::result::Result<T, AnimationError>;

#[derive(thiserror::Error, Debug)]
pub enum AnimationError {
    #[error("Failed to build MP4 config: {0}")]
    ConfigBuild(String),
    #[error("Failed to create video encoder: {0}")]
    EncoderCreation(String),
    #[error("Failed to get encoder headers: {0}")]
    EncoderHeaders(String),
    #[error("Encoding error: {0}")]
    Encoding(String),
    #[error("MP4 processing error: {0}")]
    Mp4Processing(String),
    #[error("Failed to flush encoder: {0}")]
    EncoderFlush(String),
    #[error("WebP encoding error: {0}")]
    WebPEncoding(String),
    #[error("Processor thread error")]
    ProcessorThread,
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AnimationBaseConfig {
    #[derivative(Default(value = "1920"))]
    pub width: u32,

    #[derivative(Default(value = "1080"))]
    pub height: u32,

    #[derivative(Default(value = "25"))]
    pub fps: u32,
}

impl AnimationBaseConfig {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self { width, height, fps }
    }
}
#[derive(Debug, Clone, derive_setters::Setters)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AnimationPreviewConfig {
    pub base: AnimationBaseConfig,

    stop_sig: Arc<AtomicBool>,

    #[setters(skip)]
    sender: Sender<RgbaImage>,

    #[setters(skip)]
    receiver: Receiver<RgbaImage>,
}

impl AnimationPreviewConfig {
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        let (sender, receiver) = channel::bounded(3);
        Self {
            base: AnimationBaseConfig::new(width, height, fps),
            stop_sig: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn receiver(&self) -> Receiver<RgbaImage> {
        self.receiver.clone()
    }

    pub fn sender(&self) -> Sender<RgbaImage> {
        self.sender.clone()
    }

    pub fn stop_sig(&self) -> Arc<AtomicBool> {
        self.stop_sig.clone()
    }
}

#[derive(Debug, Clone, derive_setters::Setters)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AnimationRecordConfig {
    pub base: AnimationBaseConfig,
    pub duration: Duration,
    pub output_path: PathBuf,

    stop_sig: Arc<AtomicBool>,

    #[setters(skip)]
    progress_sender: Sender<f32>,

    #[setters(skip)]
    progress_receiver: Receiver<f32>,
}

impl AnimationRecordConfig {
    pub fn new(
        width: u32,
        height: u32,
        fps: u32,
        duration: Duration,
        output_path: impl AsRef<Path>,
    ) -> Self {
        let (progress_sender, progress_receiver) = channel::bounded(1);

        Self {
            base: AnimationBaseConfig::new(width, height, fps),
            output_path: output_path.as_ref().to_path_buf(),
            duration,
            stop_sig: Arc::new(AtomicBool::new(false)),
            progress_sender,
            progress_receiver,
        }
    }

    pub fn progress_receiver(&self) -> Receiver<f32> {
        self.progress_receiver.clone()
    }

    pub fn progress_sender(&self) -> Sender<f32> {
        self.progress_sender.clone()
    }

    pub fn stop_sig(&self) -> Arc<AtomicBool> {
        self.stop_sig.clone()
    }
}

pub trait Animation: Iterator<Item = RgbaImage> {
    fn animate_preview(&mut self, config: AnimationPreviewConfig) -> Result<()>;
    fn animate_record(&mut self, config: AnimationRecordConfig) -> Result<()>;
    fn animate_record_webp(&mut self, config: AnimationRecordConfig) -> Result<()>;
}

pub trait AnimationInit: Iterator<Item = RgbaImage> {
    fn init(&mut self, width: u32, height: u32, fps: u32);
    fn reset(&mut self);
    fn total_frames(&self) -> usize;
    fn set_total_frames(&mut self, frames: usize);
}

#[macro_export]
macro_rules! impl_animation {
    ($ty:ty) => {
        impl crate::Animation for $ty {
            fn animate_preview(
                &mut self,
                config: crate::AnimationPreviewConfig,
            ) -> crate::Result<()> {
                self.reset();
                self.init(config.base.width, config.base.height, config.base.fps);
                self.set_total_frames(usize::MAX);

                let sender = config.sender();
                let stop_sig = config.stop_sig();
                let fps = config.base.fps;
                let frame_duration = std::time::Duration::from_secs_f32(1.0 / fps as f32);
                let start = std::time::Instant::now();
                let mut frame_idx: u32 = 0;

                while let Some(frame) = self.next() {
                    if stop_sig.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    spin_sleep::sleep_until(start + frame_duration * frame_idx);
                    _ = sender.try_send(frame);
                    frame_idx += 1;
                }
                Ok(())
            }

            fn animate_record(
                &mut self,
                config: crate::AnimationRecordConfig,
            ) -> crate::Result<()> {
                self.reset();
                self.init(config.base.width, config.base.height, config.base.fps);
                self.set_total_frames(
                    (config.duration.as_secs_f32() * config.base.fps as f32).ceil() as usize,
                );

                let total_frames = self.total_frames();
                let progress_sender = config.progress_sender();
                let stop_sig = config.stop_sig();
                let width = config.base.width;
                let height = config.base.height;
                let fps = config.base.fps;

                let mp4_config = mp4m::mp4_processor::Mp4ProcessorConfigBuilder::default()
                    .save_path(config.output_path.clone())
                    .video_config(mp4m::mp4_processor::VideoConfig { width, height, fps })
                    .build()
                    .map_err(|e| crate::AnimationError::ConfigBuild(e.to_string()))?;

                let mut processor = mp4m::mp4_processor::Mp4Processor::new(mp4_config);
                let video_sender = processor.h264_sender();

                let encoder_config =
                    video_encoder::VideoEncoderConfig::new(width, height).with_fps(fps);
                let mut encoder = video_encoder::new(encoder_config)
                    .map_err(|e| crate::AnimationError::EncoderCreation(e.to_string()))?;
                let headers = encoder
                    .headers()
                    .map_err(|e| crate::AnimationError::EncoderHeaders(e.to_string()))?;

                let processor_thread = std::thread::spawn(move || {
                    if let Err(e) = processor.run_processing_loop(Some(headers)) {
                        log::warn!("MP4 processing error: {}", e);
                    }
                });

                let mut frame_idx = 0;
                while let Some(frame) = self.next() {
                    if stop_sig.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    frame_idx += 1;
                    // Cap at 0.99 so monitor only breaks after receiving final 1.0 (sent after flush/join)
                    let progress = (frame_idx as f32 / total_frames as f32).min(0.99);
                    let _ = progress_sender.try_send(progress);

                    let rgb_frame = video_utils::convert::rgba_to_rgb(&frame);

                    match encoder.encode_frame(rgb_frame) {
                        Ok(video_encoder::EncodedFrame::Frame {
                            data, is_keyframe, ..
                        }) => {
                            if video_sender
                                .send(mp4m::mp4_processor::VideoFrameType::Frame {
                                    data,
                                    is_sync: is_keyframe,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(video_encoder::EncodedFrame::End) => break,
                        Err(e) => {
                            return Err(crate::AnimationError::Encoding(e.to_string()));
                        }
                        _ => {}
                    }
                }

                let video_sender_clone = video_sender.clone();
                encoder
                    .flush(Box::new(move |data, is_keyframe| {
                        if video_sender_clone
                            .send(mp4m::mp4_processor::VideoFrameType::Frame {
                                data,
                                is_sync: is_keyframe,
                            })
                            .is_err()
                        {
                            log::warn!("Failed to send flushed frame");
                        }
                    }))
                    .map_err(|e| crate::AnimationError::EncoderFlush(e.to_string()))?;

                _ = video_sender.send(mp4m::mp4_processor::VideoFrameType::End);
                drop(video_sender);

                processor_thread
                    .join()
                    .map_err(|_| crate::AnimationError::ProcessorThread)?;

                _ = progress_sender.try_send(1.0);

                Ok(())
            }

            fn animate_record_webp(
                &mut self,
                config: crate::AnimationRecordConfig,
            ) -> crate::Result<()> {
                self.reset();
                self.init(config.base.width, config.base.height, config.base.fps);
                self.set_total_frames(
                    (config.duration.as_secs_f32() * config.base.fps as f32).ceil() as usize,
                );

                let total_frames = self.total_frames();
                let progress_sender = config.progress_sender();
                let stop_sig = config.stop_sig();
                let width = config.base.width;
                let height = config.base.height;
                let fps = config.base.fps;

                let mut encoder = webpx::AnimationEncoder::new(width, height)
                    .map_err(|e| crate::AnimationError::WebPEncoding(e.to_string()))?;

                let frame_duration_ms = (1000.0 / fps as f32) as i32;
                let mut frame_idx = 0i32;

                while let Some(frame) = self.next() {
                    if stop_sig.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }

                    let timestamp_ms = frame_idx * frame_duration_ms;
                    encoder
                        .add_frame_rgba(frame.as_raw(), timestamp_ms)
                        .map_err(|e| crate::AnimationError::WebPEncoding(e.to_string()))?;

                    frame_idx += 1;
                    let progress = (frame_idx as f32 / total_frames as f32).min(0.99);
                    let _ = progress_sender.try_send(progress);
                }

                let end_timestamp_ms = frame_idx * frame_duration_ms;
                let webp_data = encoder
                    .finish(end_timestamp_ms)
                    .map_err(|e| crate::AnimationError::WebPEncoding(e.to_string()))?;

                std::fs::write(&config.output_path, webp_data)
                    .map_err(|e| crate::AnimationError::WebPEncoding(e.to_string()))?;

                let _ = progress_sender.try_send(1.0);

                Ok(())
            }
        }
    };
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, Display, EnumString)]
pub enum FlowDirection {
    #[default]
    Up,
    Down,
    Left,
    Right,
    Diagonal,
}

fn pseudo_phase(seed: usize) -> f32 {
    let val = seed as f32 * 0.6180339887;
    val.fract() * std::f32::consts::TAU
}

/// Returns a pseudo-random value in [0, 1] range for density/probability checks
fn pseudo_density(seed: usize) -> f32 {
    let val = seed as f32 * 0.6180339887;
    val.fract()
}
