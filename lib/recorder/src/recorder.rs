use crate::{
    AudioRecorder, CursorTracker, CursorTrackerConfig, EncodedFrame, FPS, Frame, FrameUser,
    ProcessMode, ProgressState, RecorderConfig, RecorderError, Resolution, SimpleFpsCounter,
    SpeakerRecorder, StatsUser, platform_speaker_recoder,
    process_mode::SHARE_SCREEN_CONNECTIONS_COUNT, speaker_recorder::SpeakerRecorderConfig,
};
use crossbeam::channel::{Receiver, Sender, bounded};
use derive_setters::Setters;
use fast_image_resize::images::Image;
use image::{ImageBuffer, Rgb, Rgba, buffer::ConvertBuffer};
use mp4m::VideoFrameType;
use once_cell::sync::Lazy;
use screen_capture::{
    Capture, CaptureStreamConfig, LogicalSize, MonitorCursorPositionConfig, Position, Rectangle,
    ScreenCapture, ScreenInfoError,
};
use spin_sleep::SpinSleeper;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use video_encoder::{VideoEncoder, VideoEncoderConfig};

type EncoderChannelData = (u64, ResizedImageBuffer);
pub type ResizedImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

pub(crate) const USER_CHANNEL_SIZE: usize = 64;
pub(crate) const CURSOR_CHANNEL_SIZE: usize = 4094;
pub(crate) const ENCODER_WORKER_CHANNEL_SIZE: usize = 128;

static CURSOR_POSITION: AtomicU64 = AtomicU64::new(u64::MAX);
static LAST_CROP_REGION: Lazy<Mutex<Option<Rectangle>>> = Lazy::new(|| Mutex::new(None));

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
                    self.cursor_thread(screen_capturer.clone(), crop_region_sender)?;
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
            }
        }

        self.frame_sender.take();

        Ok(())
    }

    pub fn wait(mut self) -> Result<ProgressState, RecorderError> {
        let (encoder_sender, encoder_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);
        let process_frame_handles = Self::process_frame_workers(&self, encoder_sender);

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
                    self.wait_stop(process_frame_handles)?;
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
        .with_fps(self.config.fps.to_u32())
        .with_debounce_radius(self.config.debounce_radius)
        .with_stable_radius(self.config.stable_radius)
        .with_zoom_in_transition_type(self.config.zoom_in_transition_type)
        .with_zoom_out_transition_type(self.config.zoom_out_transition_type)
        .with_fast_moving_duration(Duration::from_millis(self.config.fast_moving_duration))
        .with_zoom_transition_duration(Duration::from_millis(self.config.zoom_transition_duration))
        .with_reposition_edge_threshold(self.config.reposition_edge_threshold)
        .with_reposition_transition_duration(Duration::from_millis(
            self.config.reposition_transition_duration,
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

        let stable_radius = self.config.stable_radius;
        let cursor_monitor_stop_sig = stop_sig.clone();
        thread::spawn(move || {
            CURSOR_POSITION.store(u64::MAX, Ordering::SeqCst);

            {
                *LAST_CROP_REGION.lock().unwrap() = Some(Rectangle::new(
                    0,
                    0,
                    screen_info.logical_size.width,
                    screen_info.logical_size.height,
                ));
            }

            let config = MonitorCursorPositionConfig::new(screen_info, cursor_monitor_stop_sig)
                .with_use_transparent_layer_surface(true)
                .with_hole_radius((stable_radius as i32 / 2).max(30));

            if let Err(e) = screen_capturer.monitor_cursor_position(config, move |position| {
                let current_position =
                    (((position.x as u64) << 32) & 0xffff_ffff_0000_0000) | (position.y as u64);
                CURSOR_POSITION.store(current_position, Ordering::Relaxed);

                log::debug!(
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

    fn process_frame_workers(
        session: &RecordingSession,
        encoder_sender: Sender<EncoderChannelData>,
    ) -> Vec<JoinHandle<()>> {
        let mut handles = vec![];

        let (frame_sender, frame_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);
        let (collect_sender, collect_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        handles.push(Self::process_forward_worker(session, frame_sender));

        for i in 0..3 {
            handles.push(Self::process_frame_worker(
                session,
                collect_sender.clone(),
                frame_receiver.clone(),
                i,
            ));
        }

        handles.push(Self::process_collect_worker(
            session,
            encoder_sender,
            collect_receiver,
        ));
        handles
    }

    fn process_forward_worker(
        session: &RecordingSession,
        sender: Sender<(u64, Frame)>,
    ) -> JoinHandle<()> {
        let start_time = session.start_time;
        let receiver = session.frame_receiver.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let total_frame_count = session.total_frame_count.clone();

        thread::spawn(move || {
            while let Ok(frame) = receiver.recv() {
                let total_frame_count = total_frame_count.fetch_add(1, Ordering::Relaxed) + 1;

                log::debug!(
                    "total frame[{}] thread[{}] thread_frame[{}] capture time: {:.2?}. thread_fps: {:.2}. timestamp: {:.2?}. capture channel remained: {}",
                    total_frame_count,
                    frame.thread_id,
                    frame.cb_data.frame_index,
                    frame.cb_data.capture_time,
                    (frame.cb_data.frame_index + 1) as f64 / frame.cb_data.elapse.as_secs_f64(),
                    frame.timestamp.duration_since(start_time),
                    receiver.capacity().unwrap_or_default() - receiver.len()
                );

                if let Err(e) = sender.try_send((total_frame_count, frame)) {
                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                    log::warn!("process worker try send failed: {e}");
                }
            }

            log::info!("process forward thread exit");
        })
    }

    fn process_frame_worker(
        session: &RecordingSession,
        sender: Sender<(usize, Instant, EncoderChannelData)>,
        receiver: Receiver<(u64, Frame)>,
        thread_index: usize,
    ) -> JoinHandle<()> {
        let resolution = session.config.resolution.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let enable_cursor_tracking = session.config.enable_cursor_tracking;
        let crop_region_receiver = session.crop_region_receiver.clone();

        thread::spawn(move || {
            while let Ok((total_frame_count, frame)) = receiver.recv() {
                let now = Instant::now();
                let frame_timestamp = frame.timestamp;

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

                if let Err(e) =
                    sender.try_send((thread_index, frame_timestamp, (total_frame_count, img)))
                {
                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                    log::warn!("process worker try send failed: {e}");
                }
            }
            log::info!("process worker thread exit");
        })
    }

    #[inline]
    fn send_frame_to_encoder(
        img: ResizedImageBuffer,
        encoder_sender: &Sender<EncoderChannelData>,
        frame_sender_user: &Option<Sender<FrameUser>>,
        expect_total_frame_index: u64,
        total_frame_count: Arc<AtomicU64>,
        loss_frame_count: Arc<AtomicU64>,
        fps: f32,
    ) {
        if let Some(sender) = frame_sender_user {
            let frame_user = FrameUser {
                stats: StatsUser {
                    fps,
                    total_frames: total_frame_count.load(Ordering::Relaxed),
                    loss_frames: loss_frame_count.load(Ordering::Relaxed),
                    share_screen_connections: SHARE_SCREEN_CONNECTIONS_COUNT
                        .load(Ordering::Relaxed),
                },
                buffer: img.clone(),
            };

            if let Err(e) = sender.try_send(frame_user) {
                log::warn!("try send frame to user frame channel failed: {e}");
            }
        }

        if let Err(e) = encoder_sender.try_send((expect_total_frame_index, img)) {
            loss_frame_count.fetch_add(1, Ordering::Relaxed);
            log::warn!("collected thread try send to encoder reciever failed: {e}");
        }
    }

    fn process_collect_worker(
        session: &RecordingSession,
        sender: Sender<EncoderChannelData>,
        receiver: Receiver<(usize, Instant, EncoderChannelData)>,
    ) -> JoinHandle<()> {
        let total_frame_count = session.total_frame_count.clone();
        let loss_frame_count = session.loss_frame_count.clone();
        let frame_sender_user = session.frame_sender_user.clone();

        thread::spawn(move || {
            let mut expect_total_frame_index = 1;
            let mut disorder_frame_counts = 0;
            let mut frame_cache: HashMap<u64, (u64, ResizedImageBuffer)> = HashMap::new();
            let mut fps_counter = SimpleFpsCounter::new();

            while let Ok((thread_index, frame_timestamp, (total_frame_index, img))) =
                receiver.recv()
            {
                // FIXME: no accuracy. because frame_timestamp may be disorder
                let fps = fps_counter.add_frame(frame_timestamp);

                if expect_total_frame_index == total_frame_index {
                    disorder_frame_counts = 0;

                    Self::send_frame_to_encoder(
                        img,
                        &sender,
                        &frame_sender_user,
                        expect_total_frame_index,
                        total_frame_count.clone(),
                        loss_frame_count.clone(),
                        fps,
                    );

                    loop {
                        expect_total_frame_index += 1;
                        match frame_cache.remove(&expect_total_frame_index) {
                            Some(frame) => {
                                Self::send_frame_to_encoder(
                                    frame.1,
                                    &sender,
                                    &frame_sender_user,
                                    expect_total_frame_index,
                                    total_frame_count.clone(),
                                    loss_frame_count.clone(),
                                    fps,
                                );
                            }
                            _ => break,
                        }
                    }
                } else if expect_total_frame_index > total_frame_index {
                    loss_frame_count.fetch_add(1, Ordering::Relaxed);
                    log::warn!(
                        "too late thread[{thread_index}] frame, frame index: {total_frame_index}, expected index: {expect_total_frame_index}"
                    );
                } else {
                    frame_cache.insert(total_frame_index, (total_frame_index, img));
                    disorder_frame_counts += 1;

                    if disorder_frame_counts > 5 {
                        disorder_frame_counts = 0;

                        log::warn!(
                            "disorder frame counts > 5. So moving to next expected frame index: {expect_total_frame_index} -> {}",
                            expect_total_frame_index + 1
                        );

                        loop {
                            expect_total_frame_index += 1;
                            match frame_cache.remove(&expect_total_frame_index) {
                                Some(frame) => {
                                    Self::send_frame_to_encoder(
                                        frame.1,
                                        &sender,
                                        &frame_sender_user,
                                        expect_total_frame_index,
                                        total_frame_count.clone(),
                                        loss_frame_count.clone(),
                                        fps,
                                    );
                                }
                                _ => break,
                            }
                        }
                    }
                }
            }

            log::info!("precess collected thread exit");
        })
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

    fn get_matched_crop_region(crop_region_receiver: Receiver<Rectangle>) -> Rectangle {
        loop {
            match crop_region_receiver.try_recv() {
                Ok(v) => {
                    let cursor_position = CURSOR_POSITION.load(Ordering::Relaxed);
                    if cursor_position == u64::MAX {
                        return LAST_CROP_REGION.lock().unwrap().clone().unwrap();
                    };

                    let cursor_position = Position::new(
                        ((cursor_position >> 32) & 0x0000_0000_ffff_ffff) as i32,
                        (cursor_position & 0x0000_0000_ffff_ffff) as i32,
                    );

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

        log::debug!("crop region: {:?}", region);

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

        let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
            fast_image_resize::ResizeAlg::SuperSampling(fast_image_resize::FilterType::Lanczos3, 2),
        );

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
