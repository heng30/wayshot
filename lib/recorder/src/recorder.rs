use crate::{
    AudioRecorder, EncodedFrame, FPS, Frame, FrameUser, ProcessMode, ProgressState, RecorderConfig,
    RecorderError, Resolution, SpeakerRecorder, platform_speaker_recoder,
    speaker_recorder::SpeakerRecorderConfig,
};
use camera::{CameraClient, CameraConfig, query_camera_id, query_first_camera};
use crossbeam::channel::{Receiver, Sender, bounded};
use derive_setters::Setters;
use image::{GrayImage, ImageBuffer, Rgb};
use mp4m::VideoFrameType;
use screen_capture::{CaptureStreamConfig, LogicalSize, Rectangle, ScreenCapture};
use spin_sleep::SpinSleeper;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use video_encoder::{VideoEncoder, VideoEncoderConfig};

pub type ResizedImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub(crate) type CameraImage = image::RgbImage;
pub(crate) type EncoderChannelData = (u64, ResizedImageBuffer, Option<CameraImage>);

pub(crate) const USER_CHANNEL_SIZE: usize = 64;
pub(crate) const CURSOR_CHANNEL_SIZE: usize = 4094;
pub(crate) const ENCODER_WORKER_CHANNEL_SIZE: usize = 128;

#[derive(Setters)]
#[setters(prefix = "with_")]
#[setters(generate = false)]
pub struct RecordingSession {
    pub(crate) config: RecorderConfig,
    pub(crate) stop_sig: Arc<AtomicBool>,
    pub(crate) sync_sig: Arc<AtomicBool>,

    pub(crate) frame_sender: Option<Sender<Frame>>,
    pub(crate) frame_receiver: Receiver<Frame>,
    pub(crate) capture_workers: Vec<JoinHandle<()>>,

    #[setters(generate)]
    pub(crate) frame_sender_user: Option<Sender<FrameUser>>,

    pub(crate) audio_recorder: Option<AudioRecorder>,
    pub(crate) audio_level_receiver: Option<Receiver<f32>>,

    pub(crate) speaker_level_receiver: Option<Receiver<f32>>,
    pub(crate) speaker_recorder_worker: Option<JoinHandle<Result<(), RecorderError>>>,

    pub(crate) audio_mixer_stop_sig: Option<Arc<AtomicBool>>,
    pub(crate) audio_mixer_finished_sig: Option<Arc<AtomicBool>>,
    pub(crate) audio_mixer_worker: Option<JoinHandle<()>>,
    pub(crate) mp4_writer_worker: Option<JoinHandle<()>>,
    pub(crate) share_screen_worker: Option<JoinHandle<()>>,
    pub(crate) push_stream_worker: Option<JoinHandle<()>>,
    pub(crate) h264_frame_sender: Option<Sender<VideoFrameType>>,

    pub(crate) crop_region_receiver: Option<Receiver<Rectangle>>,
    pub(crate) video_encoder: Option<Box<dyn VideoEncoder>>,

    pub(crate) camera_image_receiver: Option<Receiver<CameraImage>>,
    pub(crate) camera_background_remover_receiver: Option<Receiver<CameraImage>>,
    pub(crate) camera_background_remover_waiting_frame: Arc<AtomicBool>,
    pub(crate) camera_background_mask: Arc<Mutex<Option<GrayImage>>>,

    // statistic
    pub(crate) start_time: Instant,
    pub(crate) total_frame_count: Arc<AtomicU64>,
    pub(crate) loss_frame_count: Arc<AtomicU64>,
}

impl RecordingSession {
    pub fn new(config: RecorderConfig) -> Self {
        let (frame_sender, frame_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        Self {
            config,
            stop_sig: Arc::new(AtomicBool::new(false)),
            sync_sig: Arc::new(AtomicBool::new(false)),

            frame_sender: Some(frame_sender),
            frame_receiver,
            capture_workers: vec![],

            frame_sender_user: None,

            audio_recorder: None,
            audio_level_receiver: None,

            speaker_recorder_worker: None,
            speaker_level_receiver: None,

            audio_mixer_stop_sig: None,
            audio_mixer_finished_sig: None,
            audio_mixer_worker: None,

            mp4_writer_worker: None,
            share_screen_worker: None,
            push_stream_worker: None,
            h264_frame_sender: None,

            crop_region_receiver: None,
            video_encoder: None,

            camera_image_receiver: None,
            camera_background_remover_receiver: None,
            camera_background_remover_waiting_frame: Arc::new(AtomicBool::new(true)),
            camera_background_mask: Arc::new(Mutex::new(None)),

            start_time: std::time::Instant::now(),
            total_frame_count: Arc::new(AtomicU64::new(0)),
            loss_frame_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(
        &mut self,
        rt_handle: tokio::runtime::Handle,
        mut screen_capturer: impl ScreenCapture + Clone + Send + 'static,
    ) -> Result<(), RecorderError> {
        if !self
            .config
            .save_path
            .parent()
            .ok_or(RecorderError::InvalidConfig(format!(
                "No found parent directory: {}",
                self.config.save_path.display()
            )))?
            .exists()
        {
            return Err(RecorderError::InvalidConfig(format!(
                "No found saved directory: {}",
                self.config.save_path.parent().unwrap().display()
            )));
        }

        let thread_counts = self.evaluate_need_threads(&mut screen_capturer)?;
        if thread_counts == 0 {
            return Err(RecorderError::Other(format!("capture thread counts is 0")));
        }

        log::info!("capture thread counts: {thread_counts}");

        self.start_time = std::time::Instant::now();

        let (encoder_width, encoder_height) = self.config.resolution.dimensions(
            self.config.screen_size.width as u32,
            self.config.screen_size.height as u32,
        );

        let video_encoder_config = VideoEncoderConfig::new(encoder_width, encoder_height)
            .with_fps(self.config.fps.to_u32())
            .with_annexb(match self.config.process_mode {
                ProcessMode::RecordScreen => false,
                ProcessMode::ShareScreen | ProcessMode::PushStream => true,
            });

        let mut video_encoder = video_encoder::new(video_encoder_config)?;
        let headers_data = video_encoder.headers()?;

        let (
            audio_sender,
            speak_sender,
            mix_audio_receiver,
            mix_audio_channels,
            mix_audio_sample_rate,
        ) = self.mix_audio_tracks()?;

        let h264_frame_sender = match self.config.process_mode {
            ProcessMode::RecordScreen => self.mp4_worker(
                Some(headers_data.clone()),
                mix_audio_receiver,
                mix_audio_channels,
                mix_audio_sample_rate,
            )?,
            ProcessMode::ShareScreen => self.share_screen_worker(
                rt_handle,
                Some(headers_data.clone()),
                mix_audio_receiver,
                mix_audio_channels,
                mix_audio_sample_rate,
            )?,
            ProcessMode::PushStream => self.push_stream_worker(
                rt_handle,
                Some(headers_data.clone()),
                mix_audio_receiver,
                mix_audio_channels,
                mix_audio_sample_rate,
            )?,
        };

        self.h264_frame_sender = h264_frame_sender;
        self.video_encoder = Some(video_encoder);

        if let Some(ref sender) = self.h264_frame_sender {
            if let Err(e) = sender.try_send(VideoFrameType::Frame(headers_data)) {
                log::warn!("Try send h264 header frames faield: {e}");
            }
        }

        let frame_iterval_ms = self.config.frame_interval_ms();
        let fps_per_thread = self.config.fps.to_u32() as f64 / thread_counts as f64;
        let config = CaptureStreamConfig {
            name: self.config.screen_name.clone(),
            include_cursor: self.config.include_cursor,
            fps: Some(fps_per_thread),
            cancel_sig: self.stop_sig.clone(),
            sync_sig: self.sync_sig.clone(),
        };

        // start screen capture
        for i in 0..thread_counts {
            let config_duplicate = config.clone();
            let screen_capturer_duplicate = screen_capturer.clone();
            let tx = self.frame_sender.clone().unwrap();

            let handle = thread::spawn(move || {
                SpinSleeper::default().sleep(Duration::from_millis(i as u64 * frame_iterval_ms));

                match screen_capturer_duplicate.capture_output_stream(
                    config_duplicate,
                    move |cb_data| {
                        if let Err(e) = tx.send(Frame {
                            thread_id: i,
                            cb_data,
                            timestamp: std::time::Instant::now(),
                        }) {
                            log::warn!("send frame failed: {e}");
                        }
                    },
                ) {
                    Ok(status) => {
                        log::info!("capture thread[{i}] exit. status: {status:?}")
                    }
                    Err(e) => log::warn!("capture thread[{i}] exit. error: {e}"),
                }
            });

            self.capture_workers.push(handle);

            if i == 0 {
                let (mut try_counts, mut now) = (0, Instant::now());
                while !self.sync_sig.load(Ordering::Relaxed) {
                    if now.elapsed() > Duration::from_secs(5) {
                        log::warn!("Waiting 5 seconds for `sync_sig`");
                        now = Instant::now();
                        try_counts += 1;

                        if try_counts == 3 {
                            return Err(RecorderError::Other(
                                "waiting synchronization signal for a long time".to_string(),
                            ));
                        }
                    }

                    thread::sleep(Duration::from_millis(10));
                }

                log::info!(
                    "`sync_sig` is true. start to run audio, speaker and cursor tracker threads"
                );

                if self.config.enable_cursor_tracking {
                    let (crop_region_sender, crop_region_receiver) = bounded(CURSOR_CHANNEL_SIZE);
                    self.cursor_tracker_worker(screen_capturer.clone(), crop_region_sender)?;
                    self.crop_region_receiver = Some(crop_region_receiver);
                }

                if let Some(device_name) = self.config.audio_device_name.clone() {
                    self.enable_audio(device_name.as_str(), audio_sender.clone())?;
                    log::info!("Enable audio recording successfully");
                }

                if self.config.enable_recording_speaker {
                    self.enable_speaker_audio(speak_sender.clone())?;
                    log::info!("Enable speaker recording successfully");
                };

                if self.config.camera_mix_config.enable {
                    self.enable_camera()?;
                    log::info!("Enable camera mix successfully");
                }
            }
        }

        self.frame_sender.take();

        Ok(())
    }

    pub fn wait(mut self) -> Result<ProgressState, RecorderError> {
        let (encoder_sender, encoder_receiver) =
            bounded::<EncoderChannelData>(ENCODER_WORKER_CHANNEL_SIZE);
        let process_frame_handles = Self::process_frame_workers(&self, encoder_sender);

        loop {
            match encoder_receiver.recv() {
                Ok((total_frame_index, img, _)) => {
                    let now = std::time::Instant::now();
                    match self
                        .video_encoder
                        .as_mut()
                        .unwrap()
                        .encode_frame(img.into())
                    {
                        Ok(EncodedFrame::Frame((_, encoded_frame))) => {
                            log::debug!(
                                "total encoded frame[{total_frame_index}] {} bytes",
                                encoded_frame.len()
                            );

                            if let Some(ref sender) = self.h264_frame_sender {
                                if let Err(e) =
                                    sender.try_send(VideoFrameType::Frame(encoded_frame))
                                {
                                    self.loss_frame_count.fetch_add(1, Ordering::Relaxed);
                                    log::warn!("Try send h264 body frame faield: {e}");
                                }
                            }
                        }
                        Err(e) => log::warn!("encode frame failed: {e}"),
                        _ => unreachable!("invalid EncodedFrame"),
                    }

                    log::debug!(
                        "frame encoding time: {:.2?}. encoder channel remained: {}. h264 channel remained: {}.\n",
                        now.elapsed(),
                        encoder_receiver.capacity().unwrap_or_default() - encoder_receiver.len(),
                        if self.h264_frame_sender.is_some() {
                            self.h264_frame_sender
                                .as_ref()
                                .unwrap()
                                .capacity()
                                .unwrap_or_default()
                                - self.h264_frame_sender.as_ref().unwrap().len()
                        } else {
                            0
                        }
                    );
                }
                _ => {
                    log::info!("encoder receiver channel exit...");
                    self.stop();
                    self.wait_stop(process_frame_handles)?;
                    break;
                }
            }
        }

        return Ok(ProgressState::Stopped);
    }

    fn enable_audio(
        &mut self,
        device_name: &str,
        frame_sender: Option<Sender<Vec<f32>>>,
    ) -> Result<(), RecorderError> {
        let (sender, receiver) = if self.config.enable_audio_level_channel {
            let (tx, rx) = bounded(USER_CHANNEL_SIZE);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let mut audio_recorder = AudioRecorder::new()
            .with_level_sender(sender)
            .with_frame_sender(frame_sender)
            .with_gain(self.config.audio_gain.clone())
            .with_enable_denoise(self.config.enable_denoise);

        audio_recorder.start_recording(device_name)?;
        self.audio_recorder = Some(audio_recorder);
        self.audio_level_receiver = receiver;

        Ok(())
    }

    fn enable_speaker_audio(
        &mut self,
        frame_sender: Option<Sender<Vec<f32>>>,
    ) -> Result<(), RecorderError> {
        let (sender, receiver) = if self.config.enable_speaker_level_channel {
            let (tx, rx) = bounded(USER_CHANNEL_SIZE);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let stop_sig = self.stop_sig.clone();
        let gain = self.config.speaker_gain.clone();
        let handle = thread::spawn(move || {
            let config = SpeakerRecorderConfig::new(stop_sig)
                .with_level_sender(sender)
                .with_frame_sender(frame_sender)
                .with_gain(gain);

            let recorder = platform_speaker_recoder(config)?;
            recorder.start_recording()?;
            Ok(())
        });

        self.speaker_recorder_worker = Some(handle);
        self.speaker_level_receiver = receiver;

        Ok(())
    }

    fn enable_camera(&mut self) -> Result<(), RecorderError> {
        camera::init();

        let camera_index = if let Some(ref camera_name) = self.config.camera_mix_config.camera_name
        {
            query_camera_id(camera_name)?
        } else {
            query_first_camera()?
        };

        let (camera_image_sender, camera_image_receiver) = bounded(5);
        let (camera_background_remover_sender, camera_background_remover_receiver) = bounded(1);

        self.camera_image_receiver = Some(camera_image_receiver);
        self.camera_background_remover_receiver = Some(camera_background_remover_receiver);

        let camera_config = CameraConfig::default()
            .with_fps(self.config.camera_mix_config.fps)
            .with_width(self.config.camera_mix_config.width)
            .with_height(self.config.camera_mix_config.height)
            .with_pixel_format(self.config.camera_mix_config.pixel_format)
            .with_mirror_horizontal(self.config.camera_mix_config.mirror_horizontal);

        let mut camera_client = CameraClient::new(camera_index, camera_config)?;
        let waiting_frame = self.camera_background_remover_waiting_frame.clone();

        let stop_sig = self.stop_sig.clone();
        thread::spawn(move || {
            if let Err(e) = camera_client.start() {
                log::error!("Failed to start camera: {}", e);
                return;
            }

            while !stop_sig.load(Ordering::Relaxed) {
                if let Ok(frame) = camera_client.last_frame_rgb() {
                    if waiting_frame.load(Ordering::Relaxed) {
                        if camera_background_remover_sender
                            .try_send(frame.clone())
                            .is_ok()
                        {
                            waiting_frame.store(false, Ordering::Relaxed);
                        }
                    }

                    if let Err(e) = camera_image_sender.try_send(frame) {
                        log::warn!("Failed to send camera frame: {}", e);
                    }
                }

                std::thread::sleep(Duration::from_millis(
                    1000 / camera_client.frame_rate().max(24) as u64,
                ));
            }

            if let Err(e) = camera_client.stop() {
                log::error!("Failed to stop camera: {}", e);
            }
        });

        if let Some(path) = self
            .config
            .camera_mix_config
            .background_remover_model_path
            .clone()
        {
            self.background_remover_worker(path)?;
            log::info!("Background remover worker started");
        }

        Ok(())
    }

    fn wait_stop(
        mut self,
        process_frame_handles: Vec<JoinHandle<()>>,
    ) -> Result<(), RecorderError> {
        if let Some(audio_recorder) = self.audio_recorder.take() {
            audio_recorder.stop();
            log::info!("audio recorder exit...");
        }

        if let Some(speaker_recorder_handle) = self.speaker_recorder_worker.take() {
            if let Err(e) = speaker_recorder_handle.join() {
                log::warn!("join speaker recorder thread failed: {:?}", e);
            } else {
                log::info!("speaker recorder exit...");
            }
        }

        for (i, thread) in self.capture_workers.into_iter().enumerate() {
            if let Err(e) = thread.join() {
                log::warn!("join capture thread[{i}] failed: {:?}", e);
            } else {
                log::info!("join capture thread[{i}] successfully");
            }
        }

        for (index, handle) in process_frame_handles.into_iter().enumerate() {
            if let Err(e) = handle.join() {
                log::warn!("join process frame thread[{index}] failed: {:?}", e);
            } else {
                log::info!("join process frame thread[{index}] successfully");
            }
        }

        if let Some(sender) = self.h264_frame_sender.clone()
            && let Some(ve) = self.video_encoder.take()
            && let Err(e) = ve.flush(Box::new(move |data| {
                if let Err(e) = sender.try_send(VideoFrameType::Frame(data)) {
                    log::warn!("Try send h264 flushed frame faield: {e}");
                }
            }))
        {
            log::warn!("Failed to flush encoder frame: {e:?}");
        }

        if let Some(stop_sig) = self.audio_mixer_stop_sig {
            stop_sig.store(true, Ordering::Relaxed);

            let mut try_counts = 0;
            while let Some(ref finished_sig) = self.audio_mixer_finished_sig {
                if finished_sig.load(Ordering::Relaxed) || try_counts > 5 {
                    break;
                }

                try_counts += 1;
                thread::sleep(Duration::from_millis(100));
            }
        }

        if let Some(sender) = self.h264_frame_sender.take()
            && let Err(e) = sender.try_send(VideoFrameType::End)
        {
            log::warn!("h264_frame_sender send `End` failed: {e:?}");
        }

        if let Some(handle) = self.audio_mixer_worker.take() {
            if let Err(e) = handle.join() {
                log::warn!("join audio mixer worker failed: {:?}", e);
            } else {
                log::info!("join audio mixer worker successfully");
            }
        }

        if let Some(handle) = self.mp4_writer_worker.take() {
            if let Err(e) = handle.join() {
                log::warn!("join mp4 writer worker failed: {:?}", e);
            } else {
                log::info!("join mp4 writer worker successfully");
            }
        }

        if let Some(handle) = self.share_screen_worker.take() {
            if let Err(e) = handle.join() {
                log::warn!("join share screen worker failed: {:?}", e);
            } else {
                log::info!("join share screen worker successfully");
            }
        }

        if let Some(handle) = self.push_stream_worker.take() {
            if let Err(e) = handle.join() {
                log::warn!("join push stream worker failed: {:?}", e);
            } else {
                log::info!("join push stream worker successfully");
            }
        }

        log::info!(
            "Total frame: {}. loss frame: {} ({:.2}%)",
            self.total_frame_count.load(Ordering::Relaxed),
            self.loss_frame_count.load(Ordering::Relaxed),
            self.loss_frame_count.load(Ordering::Relaxed) as f64 * 100.0
                / self.total_frame_count.load(Ordering::Relaxed).max(1) as f64,
        );

        if matches!(self.config.process_mode, ProcessMode::RecordScreen)
            || (matches!(self.config.process_mode, ProcessMode::ShareScreen)
                && self.config.share_screen_config.save_mp4)
            || (matches!(self.config.process_mode, ProcessMode::PushStream)
                && self.config.push_stream_config.save_mp4)
        {
            if self.config.save_path.exists() {
                log::info!("Successfully save: {}", self.config.save_path.display())
            } else {
                log::info!("No found: {}", self.config.save_path.display())
            }
        }

        Ok(())
    }

    pub fn save_path(&self) -> PathBuf {
        self.config.save_path.clone()
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

    pub fn get_speaker_level_receiver(&self) -> Option<Receiver<f32>> {
        self.speaker_level_receiver.clone()
    }

    pub fn warmup_video_encoder(screen_size: LogicalSize, resolution: Resolution, fps: FPS) {
        let (encoder_width, encoder_height) =
            resolution.dimensions(screen_size.width as u32, screen_size.height as u32);

        let video_encoder_config =
            VideoEncoderConfig::new(encoder_width, encoder_height).with_fps(fps.to_u32());
        match ::video_encoder::new(video_encoder_config) {
            Ok(_) => log::info!("Warmup video encoder successfully"),
            Err(e) => log::warn!("Warmup video encoder failed: {e}"),
        }
    }

    fn evaluate_need_threads(
        &self,
        screen_capturer: &mut impl ScreenCapture,
    ) -> Result<u32, RecorderError> {
        let mean_ms = match screen_capturer.capture_mean_time(&self.config.screen_name, 3)? {
            None => return Ok(1),
            Some(ms) => ms.as_millis() as f64,
        };

        log::info!("capture mean time: {mean_ms:.2?}ms");

        let iterval_ms = self.config.frame_interval_ms() as f64;
        Ok(((mean_ms / iterval_ms).ceil() * 2.0).ceil() as u32)
    }
}
