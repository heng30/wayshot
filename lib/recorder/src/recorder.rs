use crate::{
    AudioRecorder, CursorTracker, CursorTrackerConfig, EncodedFrame, FPS, Frame, FrameUser,
    ProgressState, RecorderConfig, RecorderError, Resolution, SimpleFpsCounter, SpeakerRecorder,
    StatsUser, VideoEncoder, platform_speaker_recoder, speaker_recorder::SpeakerRecorderConfig,
};
use crossbeam::channel::{Receiver, Sender, bounded};
use derive_setters::Setters;
use fast_image_resize::images::Image;
use hound::WavSpec;
use image::{ImageBuffer, Rgb, Rgba, buffer::ConvertBuffer};
use mp4m::{
    AudioConfig, AudioProcessor, AudioProcessorConfigBuilder, Mp4Processor,
    Mp4ProcessorConfigBuilder, OutputDestination, VideoConfig, VideoFrameType,
};
use once_cell::sync::Lazy;
use screen_capture::{
    Capture, CaptureStreamConfig, LogicalSize, MonitorCursorPositionConfig, Position, Rectangle,
    ScreenCapture, ScreenInfoError,
};
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

type EncoderChannelData = (u64, ResizedImageBuffer);
pub type ResizedImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

const USER_CHANNEL_SIZE: usize = 64;
const ENCODER_WORKER_CHANNEL_SIZE: usize = 128;
const AUDIO_MIXER_CHANNEL_SIZE: usize = 1024;
const CURSOR_CHANNEL_SIZE: usize = 4094;
static CURSOR_POSITION: Lazy<Mutex<Option<Position>>> = Lazy::new(|| Mutex::new(None));
static LAST_CROP_REGION: Lazy<Mutex<Option<Rectangle>>> = Lazy::new(|| Mutex::new(None));

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

    audio_mixer_stop_sig: Option<Arc<AtomicBool>>,
    audio_mixer_finished_sig: Option<Arc<AtomicBool>>,
    audio_mixer_worker: Option<JoinHandle<()>>,
    mp4_writer_worker: Option<JoinHandle<()>>,
    h264_frame_sender: Option<Sender<VideoFrameType>>,

    crop_region_receiver: Option<Receiver<Rectangle>>,
    video_encoder: Option<VideoEncoder>,

    // statistic
    start_time: Instant,
    total_frame_count: Arc<AtomicU64>,
    loss_frame_count: Arc<AtomicU64>,
}

impl RecordingSession {
    pub fn new(config: RecorderConfig) -> Self {
        let (frame_sender, frame_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        Self {
            config,
            stop_sig: Arc::new(AtomicBool::new(false)),

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
            h264_frame_sender: None,

            crop_region_receiver: None,
            video_encoder: None,

            start_time: std::time::Instant::now(),
            total_frame_count: Arc::new(AtomicU64::new(0)),
            loss_frame_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(
        &mut self,
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
        let mut video_encoder =
            VideoEncoder::new(encoder_width, encoder_height, self.config.fps, false)?;
        let headers_data = video_encoder.headers()?.entirety().to_vec();
        let (audio_sender, speaker_sender) = self.mp4_worker(Some(headers_data.clone()))?;

        self.video_encoder = Some(video_encoder);

        if let Some(ref sender) = self.h264_frame_sender {
            if let Err(e) = sender.try_send(VideoFrameType::Frame(headers_data)) {
                log::warn!("Try send h264 header frames faield: {e}");
            }
        }

        if self.config.enable_cursor_tracking {
            let (crop_region_sender, crop_region_receiver) = bounded(CURSOR_CHANNEL_SIZE);
            self.cursor_thread(screen_capturer.clone(), crop_region_sender)?;
            self.crop_region_receiver = Some(crop_region_receiver);
        }

        if let Some(device_name) = self.config.audio_device_name.clone() {
            self.enable_audio(device_name.as_str(), audio_sender)?;
            log::info!("Enable audio recording successfully");
        }

        if self.config.enable_recording_speaker {
            self.enable_speaker_audio(speaker_sender)?;
            log::info!("Enable speaker recording successfully");
        };

        let frame_iterval_ms = self.config.frame_interval_ms();
        let fps_per_thread = self.config.fps.to_u32() as f64 / thread_counts as f64;
        let config = CaptureStreamConfig {
            name: self.config.screen_name.clone(),
            include_cursor: self.config.include_cursor,
            fps: Some(fps_per_thread),
            cancel_sig: self.stop_sig.clone(),
        };

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
        }

        self.frame_sender.take();

        Ok(())
    }

    pub fn wait(mut self) -> Result<ProgressState, RecorderError> {
        let (encoder_sender, encoder_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);
        let process_frame_handle = Self::process_frame_worker(&self, encoder_sender);

        loop {
            match encoder_receiver.recv() {
                Ok((total_frame_index, img)) => {
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
                    self.wait_stop(process_frame_handle)?;
                    break;
                }
            }
        }

        return Ok(ProgressState::Stopped);
    }

    fn cursor_thread(
        &mut self,
        mut screen_capturer: impl ScreenCapture + Clone + Send + 'static,
        crop_region_sender: Sender<Rectangle>,
    ) -> Result<(), RecorderError> {
        let stop_sig = self.stop_sig.clone();
        let screen_name = self.config.screen_name.clone();

        let screen_info = screen_capturer
            .available_screens()?
            .iter()
            .find(|item| item.name == screen_name)
            .ok_or(RecorderError::ScreenInfoFailed(ScreenInfoError::Other(
                format!("No found screen in cursor monitor thread {screen_name}"),
            )))?
            .clone();

        let (cursor_sender, cursor_receiver) = bounded(CURSOR_CHANNEL_SIZE);
        let target_size = LogicalSize::new(
            self.config.region_width.max(1),
            self.config.region_height.max(1),
        );

        let cursor_tracker_config = CursorTrackerConfig::new(
            screen_info.logical_size,
            target_size,
            crop_region_sender,
            cursor_receiver,
            stop_sig.clone(),
        )?
        .with_stable_radius(self.config.stable_radius)
        .with_fast_moving_duration(Duration::from_millis(self.config.fast_moving_duration))
        .with_linear_transition_duration(Duration::from_millis(
            self.config.linear_transition_duration,
        ))
        .with_max_stable_region_duration(Duration::from_secs(
            self.config.max_stable_region_duration,
        ));

        thread::spawn(move || {
            let cursor_tracker = match CursorTracker::new(cursor_tracker_config) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("New cursor tracker faild: {e}");
                    return;
                }
            };

            if let Err(e) = cursor_tracker.run() {
                log::error!("Run Cursor tracker failed: {e}");
            }

            log::info!("Exit cursor tracker thread");
        });

        let cursor_monitor_stop_sig = stop_sig.clone();
        thread::spawn(move || {
            {
                *CURSOR_POSITION.lock().unwrap() = None;
                *LAST_CROP_REGION.lock().unwrap() = Some(Rectangle::new(
                    0,
                    0,
                    screen_info.logical_size.width,
                    screen_info.logical_size.height,
                ));
            }

            let config = MonitorCursorPositionConfig::new(screen_info, cursor_monitor_stop_sig)
                .with_use_transparent_layer_surface(true)
                .with_hole_radius(15);

            if let Err(e) = screen_capturer.monitor_cursor_position(config, move |position| {
                {
                    *CURSOR_POSITION.lock().unwrap() = Some(Position::new(position.x, position.y));
                }

                log::info!(
                    "dimensions: {}x{} at ({}, {}). (x, y) = ({}, {})",
                    position.output_width,
                    position.output_height,
                    position.output_x,
                    position.output_y,
                    position.x,
                    position.y
                );

                if let Err(e) = cursor_sender.try_send((Instant::now(), position)) {
                    log::warn!("cursor sender failed: {e}");
                }
            }) {
                log::error!("monitor cursor position faield: {e}");
            }

            log::info!("Exit monitor cursor position thread");
        });

        Ok(())
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

    fn process_frame_worker(
        session: &RecordingSession,
        encoder_sender: Sender<EncoderChannelData>,
    ) -> JoinHandle<()> {
        let start_time = session.start_time;
        let resolution = session.config.resolution.clone();
        let capture_receiver = session.frame_receiver.clone();
        let frame_sender_user = session.frame_sender_user.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let total_frame_count = session.total_frame_count.clone();
        let enable_cursor_tracking = session.config.enable_cursor_tracking;
        let crop_region_receiver = session.crop_region_receiver.clone();
        let mut fps_counter = SimpleFpsCounter::new();

        let handle = thread::spawn(move || {
            loop {
                match capture_receiver.recv() {
                    Ok(frame) => {
                        let fps = fps_counter.add_frame(frame.timestamp);
                        let total_frame_count =
                            total_frame_count.fetch_add(1, Ordering::Relaxed) + 1;

                        log::debug!(
                            "total frame[{}] thread[{}] thread_frame[{}] capture time: {:.2?}. thread_fps: {:.2}. timestamp: {:.2?}. capture channel remained: {}",
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

                        let now = Instant::now();
                        let img = if enable_cursor_tracking {
                            match Self::crop_and_resize_frame(
                                frame,
                                resolution,
                                crop_region_receiver.clone().unwrap(),
                            ) {
                                Ok(img) => img,
                                Err(e) => {
                                    log::warn!("crop and resize frame failed: {e}");
                                    continue;
                                }
                            }
                        } else {
                            match Self::resize_frame(frame, resolution) {
                                Ok(img) => img,
                                Err(e) => {
                                    log::warn!("resize frame failed: {e}");
                                    continue;
                                }
                            }
                        };

                        log::debug!(
                            "{} frame spent: {:.2?}",
                            if enable_cursor_tracking {
                                "crop"
                            } else {
                                "resize"
                            },
                            now.elapsed()
                        );

                        if let Some(ref sender) = frame_sender_user {
                            let frame_user = FrameUser {
                                stats: StatsUser {
                                    fps,
                                    total_frames: total_frame_count,
                                    loss_frames: loss_frame_count.load(Ordering::Relaxed),
                                },
                                buffer: img.clone(),
                            };
                            if let Err(e) = sender.try_send(frame_user) {
                                log::warn!("try send frame to user frame channel failed: {e}");
                            }
                        }

                        if let Err(e) = encoder_sender.try_send((total_frame_count, img)) {
                            loss_frame_count.fetch_add(1, Ordering::Relaxed);
                            log::warn!("resize worker try send failed: {e}");
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

    fn mp4_worker(
        &mut self,
        video_encoder_header_data: Option<Vec<u8>>,
    ) -> Result<(Option<Sender<Vec<f32>>>, Option<Sender<Vec<f32>>>), RecorderError> {
        let mut specs = vec![];
        let (mut audio_sender, mut speak_sender) = (None, None);

        if let Some(ref device_name) = self.config.audio_device_name {
            specs.push(AudioRecorder::new().spec(device_name)?);
        }

        if self.config.enable_recording_speaker {
            specs.push(platform_speaker_recoder(SpeakerRecorderConfig::default())?.spec());
        }

        let (encoder_width, encoder_height) = self.config.resolution.dimensions(
            self.config.screen_size.width as u32,
            self.config.screen_size.height as u32,
        );

        let mut mp4_processor = Mp4Processor::new(
            Mp4ProcessorConfigBuilder::default()
                .save_path(self.config.save_path.clone())
                .channel_size(AUDIO_MIXER_CHANNEL_SIZE)
                .video_config(VideoConfig {
                    width: encoder_width,
                    height: encoder_height,
                    fps: self.config.fps.to_u32(),
                })
                .build()?,
        );

        if !specs.is_empty() {
            let target_sample_rate = specs
                .iter()
                .max_by_key(|item| item.sample_rate)
                .unwrap()
                .sample_rate;

            let target_channels = if self.config.convert_to_mono {
                1
            } else {
                specs
                    .iter()
                    .max_by_key(|item| item.channels)
                    .unwrap()
                    .channels
            };

            let mp4_audio_sender = mp4_processor.add_audio_track(AudioConfig {
                convert_to_mono: false,
                spec: WavSpec {
                    channels: target_channels,
                    sample_rate: target_sample_rate,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            })?;

            let config = AudioProcessorConfigBuilder::default()
                .target_sample_rate(target_sample_rate)
                .channel_size(AUDIO_MIXER_CHANNEL_SIZE)
                .convert_to_mono(self.config.convert_to_mono)
                .output_destination(Some(OutputDestination::<f32>::Channel(mp4_audio_sender)))
                .build()?;

            let mut audio_processor = AudioProcessor::new(config);

            if self.config.audio_device_name.is_some() && self.config.enable_recording_speaker {
                audio_sender = Some(audio_processor.add_track(specs[0]));
                speak_sender = Some(audio_processor.add_track(specs[1]));
            } else if self.config.audio_device_name.is_some() {
                audio_sender = Some(audio_processor.add_track(specs[0]));
            } else if self.config.enable_recording_speaker {
                speak_sender = Some(audio_processor.add_track(specs[0]));
            }

            self.audio_mixer_stop_sig = Some(Arc::new(AtomicBool::new(false)));
            self.audio_mixer_finished_sig = Some(Arc::new(AtomicBool::new(false)));

            let stop_sig = self.audio_mixer_stop_sig.clone().unwrap();
            let finished_sig = self.audio_mixer_finished_sig.clone().unwrap();

            let handle = thread::spawn(move || {
                loop {
                    if let Err(e) = audio_processor.process_samples() {
                        log::warn!("Audio mixer process samples failed: {e}");
                    }

                    if stop_sig.load(Ordering::Relaxed) {
                        if let Err(e) = audio_processor.flush() {
                            log::warn!("Audio mixer flush sample failed: {e}");
                        }
                        finished_sig.store(true, Ordering::Relaxed);
                        return;
                    }

                    thread::sleep(Duration::from_millis(100));
                }
            });

            self.audio_mixer_worker = Some(handle);
        }

        self.h264_frame_sender = Some(mp4_processor.h264_sender());
        let handle = thread::spawn(move || {
            if let Err(e) = mp4_processor.run_processing_loop(video_encoder_header_data) {
                log::warn!("MP4 processing error: {}", e);
            }
        });
        self.mp4_writer_worker = Some(handle);

        Ok((audio_sender, speak_sender))
    }

    fn wait_stop(mut self, process_frame_handle: JoinHandle<()>) -> Result<(), RecorderError> {
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

        if let Err(e) = process_frame_handle.join() {
            log::warn!("join process frame thread failed: {:?}", e);
        } else {
            log::info!("join process frame thread successfully");
        }

        match self.video_encoder.take().unwrap().flush() {
            Ok(mut flush) => {
                while let Some(result) = flush.next() {
                    match result {
                        Ok((data, _)) => {
                            if let Some(ref sender) = self.h264_frame_sender {
                                if let Err(e) =
                                    sender.try_send(VideoFrameType::Frame(data.entirety().to_vec()))
                                {
                                    log::warn!("Try send h264 flushed frame faield: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to flush encoder frame: {e:?}");
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to flush encoder: {e}");
            }
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

        log::info!(
            "Total frame: {}. loss frame: {} ({:.2}%)",
            self.total_frame_count.load(Ordering::Relaxed),
            self.loss_frame_count.load(Ordering::Relaxed),
            self.loss_frame_count.load(Ordering::Relaxed) as f64 * 100.0
                / self.total_frame_count.load(Ordering::Relaxed).max(1) as f64,
        );

        if self.config.save_path.exists() {
            log::info!("Successfully save: {}", self.config.save_path.display())
        } else {
            log::info!("No found: {}", self.config.save_path.display())
        }

        Ok(())
    }

    fn get_matched_crop_region(crop_region_receiver: Receiver<Rectangle>) -> Rectangle {
        let cursor_position = CURSOR_POSITION.lock().unwrap().clone();
        let Some(cursor_position) = cursor_position else {
            return LAST_CROP_REGION.lock().unwrap().clone().unwrap();
        };

        loop {
            match crop_region_receiver.try_recv() {
                Ok(v) => {
                    if v.contain_position(&cursor_position) {
                        *LAST_CROP_REGION.lock().unwrap() = Some(v);
                        return v;
                    }
                }
                _ => return LAST_CROP_REGION.lock().unwrap().clone().unwrap(),
            };
        }
    }

    fn crop_and_resize_frame(
        frame: Frame,
        resolution: Resolution,
        crop_region_receiver: Receiver<Rectangle>,
    ) -> Result<ResizedImageBuffer, RecorderError> {
        let region = Self::get_matched_crop_region(crop_region_receiver);

        if matches!(resolution, Resolution::Original(_))
            && region.width as u32 == frame.cb_data.data.width
            && region.height as u32 == frame.cb_data.data.height
        {
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                frame.cb_data.data.width,
                frame.cb_data.data.height,
                frame.cb_data.data.pixel_data,
            )
            .ok_or_else(|| {
                RecorderError::ImageProcessingFailed("Failed to create image buffer".to_string())
            })?;

            let img: ImageBuffer<Rgb<u8>, Vec<u8>> = img.convert();
            return Ok(img);
        }

        let (original_width, original_height) =
            (frame.cb_data.data.width, frame.cb_data.data.height);
        let target_size = resolution.dimensions(original_width, original_height);
        Self::resize_image(frame.cb_data.data, target_size, Some(region))
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

            let img = Self::resize_image(
                frame.cb_data.data,
                resolution.dimensions(original_width, original_height),
                None,
            )?;

            img
        };

        Ok(img)
    }

    pub fn resize_image(
        mut capture: Capture,
        target_size: (u32, u32),
        region: Option<Rectangle>,
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

        let resize_options =
            fast_image_resize::ResizeOptions::new().resize_alg(if region.is_some() {
                fast_image_resize::ResizeAlg::Interpolation(fast_image_resize::FilterType::Lanczos3)
            } else {
                fast_image_resize::ResizeAlg::SuperSampling(
                    fast_image_resize::FilterType::Lanczos3,
                    2,
                )
            });

        let resize_options = if let Some(region) = region {
            resize_options.crop(
                region.x as f64,
                region.y as f64,
                region.width as f64,
                region.height as f64,
            )
        } else {
            resize_options
        };

        fast_image_resize::Resizer::new()
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
        match VideoEncoder::new(encoder_width, encoder_height, fps, false) {
            Ok(_) => log::info!("Warmup video encoder successfully"),
            Err(e) => log::warn!("Warmup video encoder failed: {e}"),
        }
    }

    fn evaluate_need_threads(
        &self,
        screen_capturer: &mut impl ScreenCapture,
    ) -> Result<u32, RecorderError> {
        let mean_ms = screen_capturer
            .capture_mean_time(&self.config.screen_name, 3)?
            .as_millis() as f64;

        log::info!("capture mean time: {mean_ms:.2?}ms");

        let iterval_ms = self.config.frame_interval_ms() as f64;
        Ok(((mean_ms / iterval_ms).ceil() * 2.0).ceil() as u32)
    }
}
