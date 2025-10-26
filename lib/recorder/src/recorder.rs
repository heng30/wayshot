use crate::{
    AudioError, AudioRecorder, EncodedFrame, Frame, FrameUser, ProgressState, RecorderConfig,
    RecorderError, Resolution, SimpleFpsCounter, SpeakerRecorder, StatsUser, VideoEncoder,
};
use capture::{Capture, CaptureIterConfig, capture_output_iter};
use crossbeam::channel::{Receiver, Sender, bounded};
use derive_setters::Setters;
use fast_image_resize::images::Image;
use image::{ImageBuffer, Rgb, Rgba, buffer::ConvertBuffer};
use once_cell::sync::Lazy;
use spin_sleep::SpinSleeper;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type EncoderChannelData = (u64, ResizedImageBuffer);
pub(crate) type ResizedImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

const USER_CHANNEL_SIZE: usize = 64;
const ENCODER_WORKER_CHANNEL_SIZE: usize = 128;

static CAPTURE_MEAN_TIME: Lazy<Mutex<Option<Duration>>> = Lazy::new(|| Mutex::new(None));

#[derive(Setters)]
#[setters(prefix = "with_")]
#[setters(generate = false)]
pub struct RecordingSession {
    config: RecorderConfig,
    stop_sig: Arc<AtomicBool>,

    frame_sender: Option<Sender<Frame>>,
    frame_receiver: Receiver<Frame>,
    capture_workers: Vec<JoinHandle<()>>,

    #[setters(generate)]
    frame_sender_user: Option<Sender<FrameUser>>,

    audio_recorder: Option<AudioRecorder>,
    audio_level_receiver: Option<Receiver<f32>>,

    speaker_level_receiver: Option<Receiver<f32>>,
    speaker_recorder_worker: Option<JoinHandle<Result<(), RecorderError>>>,

    // statistic
    start_time: Instant,
    total_frame_count: Arc<AtomicU64>,
    loss_frame_count: Arc<AtomicU64>,
}

impl RecordingSession {
    pub fn init(screen_name: &str) -> Result<(), RecorderError> {
        let mean_time = capture::capture_mean_time(screen_name, 10)?;

        {
            *CAPTURE_MEAN_TIME.lock().unwrap() = Some(mean_time);
        }

        log::info!("capture_mean_time: {mean_time:.2?}");

        Ok(())
    }

    pub fn init_finished() -> bool {
        CAPTURE_MEAN_TIME.lock().unwrap().is_some()
    }

    pub fn new(config: RecorderConfig) -> Self {
        let (frame_sender, frame_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        Self {
            config,

            start_time: std::time::Instant::now(),
            total_frame_count: Arc::new(AtomicU64::new(0)),
            loss_frame_count: Arc::new(AtomicU64::new(0)),

            frame_sender_user: None,
            frame_sender: Some(frame_sender),
            frame_receiver,

            stop_sig: Arc::new(AtomicBool::new(false)),
            capture_workers: vec![],

            audio_recorder: None,
            audio_level_receiver: None,

            speaker_level_receiver: None,
            speaker_recorder_worker: None,
        }
    }

    pub fn start(&mut self) -> Result<(), RecorderError> {
        let thread_counts = self.evaluate_need_threads();
        if thread_counts == 0 {
            return Err(RecorderError::Other(format!("capture thread counts is 0")));
        }

        log::debug!("capture thread counts: {thread_counts}");

        let frame_iterval_ms = self.config.frame_interval_ms();
        let fps_per_thread = self.config.fps.to_u32() as f64 / thread_counts as f64;

        let config = CaptureIterConfig {
            name: self.config.name.clone(),
            include_cursor: self.config.include_cursor,
            fps: Some(fps_per_thread),
            cancel_sig: self.stop_sig.clone(),
        };

        self.start_time = std::time::Instant::now();

        // Start audio recording if enabled
        if let Some(device_name) = self.config.audio_device_name.clone() {
            self.enable_audio(device_name.as_str())?;
            log::info!("Audio recording started with video recording");
        }

        if self.config.enable_recording_speaker {
            self.enable_speaker_audio()?;
            log::info!("Speaker recording started with video recording");
        };

        for i in 0..thread_counts {
            let config_duplicate = config.clone();
            let tx = self.frame_sender.clone().unwrap();

            let handle = thread::spawn(move || {
                SpinSleeper::default().sleep(Duration::from_millis(i as u64 * frame_iterval_ms));

                match capture_output_iter(config_duplicate, move |cb_data| {
                    if let Err(e) = tx.send(Frame {
                        thread_id: i,
                        cb_data,
                        timestamp: std::time::Instant::now(),
                    }) {
                        log::warn!("send frame failed: {e}");
                    }
                }) {
                    Ok(status) => {
                        log::info!("capture thread[{i}] exit. status: {status:?}")
                    }
                    Err(e) => log::warn!("capture thread[{i}] exit. error: {e}"),
                }
            });

            self.capture_workers.push(handle);
        }

        self.frame_sender.take();

        Ok(())
    }

    pub fn wait(self) -> Result<ProgressState, RecorderError> {
        let (encoder_sender, encoder_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);
        let resize_handle = Self::resize_workers(&self, encoder_sender);

        let (width, height) = self.config.resolution.dimensions(
            self.config.screen_logical_size.width as u32,
            self.config.screen_logical_size.height as u32,
        );
        let mut video_encoder = VideoEncoder::new(width, height, self.config.fps)?;

        // TODO:
        // Write encoder headers first if we have a writer
        // if let Some(ref writer) = h264_writer {
        //     let headers = video_encoder.headers()?;
        //     let headers_data = headers.entirety().to_vec();
        //     writer.write_frame(EncodedFrame::Frame((0, headers_data)));
        // }

        loop {
            match encoder_receiver.recv() {
                Ok((total_frame_index, img)) => {
                    let now = std::time::Instant::now();
                    match video_encoder.encode_frame(img.into()) {
                        Ok(EncodedFrame::Frame((_, encoded_frame))) => {
                            log::debug!(
                                "total encoded frame[{total_frame_index}] {} bytes",
                                encoded_frame.len()
                            );

                            // TODO:
                            // if let Some(ref writer) = h264_writer {
                            //     writer.write_frame(EncodedFrame::Frame((
                            //         total_frame_index,
                            //         encoded_frame,
                            //     )));
                            // }
                        }
                        Err(e) => log::warn!("encode frame failed: {e}"),
                        _ => unreachable!("invalid EncodedFrame"),
                    }

                    log::debug!(
                        "frame encoding time: {:.2?}. encoder receiver channel remained: {}\n",
                        now.elapsed(),
                        encoder_receiver.capacity().unwrap_or_default() - encoder_receiver.len(),
                    );
                }
                _ => {
                    log::info!("exit encoder receiver channel");
                    self.stop();
                    self.wait_stop(resize_handle, Some(video_encoder))?;
                    break;
                }
            }
        }

        return Ok(ProgressState::Stopped);
    }

    /// Enable audio recording with specified device
    fn enable_audio(&mut self, device_name: &str) -> Result<(), RecorderError> {
        let (sender, receiver) = if self.config.enable_audio_level_channel {
            let (tx, rx) = bounded(USER_CHANNEL_SIZE);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let mut audio_recorder = AudioRecorder::new()
            .with_level_sender(sender)
            .with_gain(self.config.audio_gain.clone())
            .with_enable_denoise(self.config.enable_denoise);

        audio_recorder
            .start_recording(device_name)
            .map_err(|e: AudioError| RecorderError::AudioError(e.to_string()))?;

        self.audio_recorder = Some(audio_recorder);
        self.audio_level_receiver = receiver;

        Ok(())
    }

    fn enable_speaker_audio(&mut self) -> Result<(), RecorderError> {
        let (sender, receiver) = if self.config.enable_speaker_level_channel {
            let (tx, rx) = bounded(USER_CHANNEL_SIZE);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let stop_sig = self.stop_sig.clone();
        let gain = self.config.speaker_gain.clone();
        let handle = thread::spawn(move || {
            let recorder = SpeakerRecorder::new(stop_sig)
                .map_err(|e| RecorderError::SpeakerError(e.to_string()))?
                .with_level_sender(sender)
                .with_gain(gain);

            recorder
                .start_recording()
                .map_err(|e| RecorderError::SpeakerError(e.to_string()))?;
            Ok(())
        });

        self.speaker_recorder_worker = Some(handle);
        self.speaker_level_receiver = receiver;

        Ok(())
    }

    fn resize_workers(
        session: &RecordingSession,
        encoder_sender: Sender<EncoderChannelData>,
    ) -> JoinHandle<()> {
        let start_time = session.start_time;
        let resolution = session.config.resolution.clone();
        let capture_receiver = session.frame_receiver.clone();
        let frame_sender_user = session.frame_sender_user.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let total_frame_count = session.total_frame_count.clone();
        let mut fps_counter = SimpleFpsCounter::new();

        let handle = thread::spawn(move || {
            loop {
                match capture_receiver.recv() {
                    Ok(frame) => {
                        let total_frame_count =
                            total_frame_count.fetch_add(1, Ordering::Relaxed) + 1;

                        log::debug!(
                            "total frame[{}] thread[{}] thread_frame[{}] capture time: {:.2?}. thread_fps: {:.2}. timestamp: {:.2?}. capture receiver channel remained: {}",
                            total_frame_count,
                            frame.thread_id,
                            frame.cb_data.frame_index,
                            frame.cb_data.capture_time,
                            (frame.cb_data.frame_index + 1) as f64
                                / frame.cb_data.elapse.as_secs_f64(),
                            frame.timestamp.duration_since(start_time),
                            capture_receiver.capacity().unwrap_or_default()
                                - capture_receiver.len()
                        );

                        log::debug!(
                            "encoder channel remained: {}. ",
                            encoder_sender.capacity().unwrap_or_default() - encoder_sender.len()
                        );

                        if let Some(ref sender) = frame_sender_user {
                            let frame_user = FrameUser {
                                stats: StatsUser {
                                    fps: fps_counter.add_frame(frame.timestamp),
                                    total_frames: total_frame_count,
                                    loss_frames: loss_frame_count.load(Ordering::Relaxed),
                                },
                                frame: frame.clone(),
                            };
                            if let Err(e) = sender.try_send(frame_user) {
                                log::warn!("try send frame to user frame channel failed: {e}");
                            }
                        }

                        match Self::resize_frame(frame, resolution) {
                            Err(e) => log::warn!("resize frame failed: {e}"),
                            Ok(img) => {
                                if let Err(e) = encoder_sender.try_send((total_frame_count, img)) {
                                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                                    log::warn!("resize worker try send failed: {e}");
                                }
                            }
                        }
                    }
                    _ => {
                        log::info!("resize forward thread exit");
                        return;
                    }
                }
            }
        });

        handle
    }

    fn wait_stop(
        mut self,
        resize_handle: JoinHandle<()>,
        encoder: Option<VideoEncoder>,
    ) -> Result<(), RecorderError> {
        if let Some(audio_recorder) = self.audio_recorder.take() {
            audio_recorder.stop();
        }

        if let Some(speaker_recorder_handle) = self.speaker_recorder_worker.take()
            && let Err(e) = speaker_recorder_handle.join()
        {
            log::warn!("join speaker recorder thread failed: {:?}", e);
        }

        for (i, thread) in self.capture_workers.into_iter().enumerate() {
            if let Err(e) = thread.join() {
                log::warn!("join capture thread[{i}] failed: {:?}", e);
            }
        }

        if let Err(e) = resize_handle.join() {
            log::warn!("join resize thread failed: {:?}", e);
        }

        if let Some(encoder) = encoder {
            match encoder.flush() {
                Ok(mut flush) => {
                    while let Some(result) = flush.next() {
                        match result {
                            Ok((data, _)) => {
                                // TODO:
                                let _frame_data = data.entirety().to_vec();
                            }
                            Err(e) => {
                                log::warn!("Failed to flush encoder frame: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to flush encoder: {}", e);
                }
            }
        }

        log::info!(
            "Total frame: {}. loss frame: {} ({:.2}%)",
            self.total_frame_count.load(Ordering::Relaxed),
            self.loss_frame_count.load(Ordering::Relaxed),
            self.loss_frame_count.load(Ordering::Relaxed) as f64 * 100.0
                / self.total_frame_count.load(Ordering::Relaxed).max(1) as f64,
        );

        Ok(())
    }

    fn resize_frame(
        frame: Frame,
        resolution: Resolution,
    ) -> Result<ResizedImageBuffer, RecorderError> {
        let img = if matches!(resolution, Resolution::Original(_)) {
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                frame.cb_data.data.width,
                frame.cb_data.data.height,
                frame.cb_data.data.pixel_data,
            )
            .ok_or_else(|| {
                RecorderError::ImageProcessingFailed("Failed to create image buffer".to_string())
            })?;

            let img: ImageBuffer<Rgb<u8>, Vec<u8>> = img.convert();
            img
        } else {
            let (original_width, original_height) =
                (frame.cb_data.data.width, frame.cb_data.data.height);

            let now = std::time::Instant::now();
            let img = Self::resize_image(
                frame.cb_data.data,
                resolution.dimensions(original_width, original_height),
            )?;

            log::debug!("resize image time: {:.2?}", now.elapsed());

            img
        };

        Ok(img)
    }

    pub fn resize_image(
        mut capture: Capture,
        target_size: (u32, u32),
    ) -> Result<ResizedImageBuffer, RecorderError> {
        let (src_width, src_height) = (capture.width as u32, capture.height as u32);
        let (dst_width, dst_height) = target_size;

        // Use fast_image_resize for high-performance resizing
        let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

        let src_image = Image::from_slice_u8(
            src_width,
            src_height,
            &mut capture.pixel_data,
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|e| {
            RecorderError::ImageProcessingFailed(format!("Failed to create source image: {}", e))
        })?;

        let mut dst_image = Image::from_slice_u8(
            dst_width,
            dst_height,
            &mut dst,
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|e| {
            RecorderError::ImageProcessingFailed(format!(
                "Failed to create destination image: {}",
                e
            ))
        })?;

        let mut resizer = fast_image_resize::Resizer::new();
        let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
            fast_image_resize::ResizeAlg::SuperSampling(fast_image_resize::FilterType::Lanczos3, 2),
        );

        resizer
            .resize(&src_image, &mut dst_image, &resize_options)
            .map_err(|e| RecorderError::ImageProcessingFailed(format!("Resize failed: {}", e)))?;

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(dst_width, dst_height, dst)
            .ok_or_else(|| {
                RecorderError::ImageProcessingFailed(
                    "Failed to create resized image buffer".to_string(),
                )
            })?;

        Ok(img.convert())
    }

    pub fn stop(&self) {
        self.stop_sig.store(true, Ordering::Relaxed);
    }

    pub fn get_stop_sig(&self) -> Arc<AtomicBool> {
        self.stop_sig.clone()
    }

    pub fn get_audio_level_receiver(&self) -> Option<Receiver<f32>> {
        self.audio_level_receiver.clone()
    }

    pub fn get_speaker_level_receiver_user(&self) -> Option<Receiver<f32>> {
        self.speaker_level_receiver.clone()
    }

    fn evaluate_need_threads(&self) -> u32 {
        let mean_ms = {
            CAPTURE_MEAN_TIME
                .lock()
                .unwrap()
                .clone()
                .expect("Need to call `RecordingSession::init()`")
                .as_millis() as f64
        };

        let iterval_ms = 1000.0 / self.config.fps.to_u32() as f64;

        ((mean_ms / iterval_ms).ceil() * 2.0).ceil() as u32
    }
}
