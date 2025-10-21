use crate::{
    AudioError, AudioRecorder, Denoise, EncodedFrame, Frame, FrameUser, H264Writer,
    MergeTracksConfig, ProgressState, RecorderConfig, RecorderError, Resolution, SimpleFpsCounter,
    SpeakerRecorder, StatsUser, StreamingAudioRecorder, VideoEncoder, merge_tracks,
};
use capture::{Capture, CaptureIterConfig, capture_output_iter};
use crossbeam::channel::{Receiver, Sender, bounded};
use fast_image_resize::images::Image;
use image::{ImageBuffer, Rgb, Rgba, buffer::ConvertBuffer};
use once_cell::sync::Lazy;
use spin_sleep::SpinSleeper;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type ResizeChannelData = (u64, Frame);
type EncoderChannelData = (u64, ResizedImageBuffer);
pub type ResizedImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

const RAW_VIDEO_EXTENSION: &str = "h264";
const INPUT_AUDIO_EXTENSION: &str = "input.wav";
const INPUT_AUDIO_DENOISE_EXTENSION: &str = "denoise.input.wav";
const SPEAKER_AUDIO_EXTENSION: &str = "speaker.wav";
const TMP_OUTPUT_VIDEO_EXTENSION: &str = "tmp.mp4";

const USER_CHANNEL_SIZE: usize = 64;
const RESIZE_WORKER_CHANNEL_SIZE: usize = 64;
const ENCODER_WORKER_CHANNEL_SIZE: usize = 128;

static CAPTURE_MEAN_TIME: Lazy<Mutex<Option<Duration>>> = Lazy::new(|| Mutex::new(None));

/// Main recording session that manages the entire screen recording pipeline.
///
/// This struct coordinates multiple threads for screen capture, frame processing,
/// video encoding, and audio recording. It provides a high-level API for starting,
/// controlling, and waiting for recording sessions to complete.
///
/// # Lifecycle
///
/// 1. **Initialize**: Call `RecordingSession::init("eDP-1")` once at application startup
/// 2. **Create**: Create a new session with `RecordingSession::new(config)`
/// 3. **Start**: Begin recording with `session.start()`
/// 4. **Wait**: Process frames and wait for completion with `session.wait()`
/// 5. **Stop**: Optionally stop recording early with `session.stop()`
///
/// # Examples
///
/// ```no_run
/// use recorder::{RecordingSession, RecorderConfig, Resolution, FPS};
/// use capture::LogicalSize;
/// use std::path::PathBuf;
///
/// // Initialize once at application startup
/// RecordingSession::init("eDP-1").unwrap();
///
/// // Create configuration
/// let config = RecorderConfig::new(
///     "HDMI-A-1".to_string(),
///     LogicalSize { width: 1920, height: 1080 },
///     PathBuf::from("recording.mp4"),
/// )
/// .with_fps(FPS::Fps30)
/// .with_resolution(Resolution::P1080);
///
/// // Start recording
/// let mut session = RecordingSession::new(config);
/// session.start().unwrap();
///
/// // Wait for completion with progress callback
/// let result = session.wait(|progress| {
///     println!("Recording progress: {:.1}%", progress * 100.0);
/// });
///
/// match result {
///     Ok(state) => println!("Recording finished: {:?}", state),
///     Err(e) => eprintln!("Recording failed: {}", e),
/// }
/// ```
pub struct RecordingSession {
    config: RecorderConfig,

    // statistic
    start_time: Instant,
    total_frame_count: Arc<AtomicU64>,
    loss_frame_count: Arc<AtomicU64>,

    frame_sender: Option<Arc<Sender<Frame>>>,
    frame_receiver: Receiver<Frame>,
    capture_workers: Vec<JoinHandle<()>>,

    frame_sender_user: Option<Sender<FrameUser>>,
    frame_receiver_user: Option<Arc<Receiver<FrameUser>>>,

    stop_sig: Arc<AtomicBool>,
    stop_sig_combine: Arc<AtomicBool>,
    stop_sig_denoise: Arc<AtomicBool>,

    audio_recorder: Option<StreamingAudioRecorder>,

    speaker_level_receiver_user: Option<Arc<Receiver<f32>>>,
    speaker_recorder_worker: Option<JoinHandle<Result<(), RecorderError>>>,
}

impl RecordingSession {
    /// Initialize the recording system by evaluating screen capture performance.
    ///
    /// This method should be called once at application startup before creating
    /// any recording sessions. It measures the average time required for screen
    /// capture operations to optimize thread allocation.
    ///
    /// # Returns
    ///
    /// `Ok(())` if initialization succeeded, or `Err(RecorderError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::RecordingSession;
    ///
    /// // Initialize once at application startup
    /// RecordingSession::init("eDP-1").unwrap();
    /// ```
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

    /// Create a new recording session with the given configuration.
    ///
    /// This constructor sets up the internal channels and state for recording
    /// but does not start the actual recording process. Use `start()` to begin recording.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the recording session
    ///
    /// # Returns
    ///
    /// A new `RecordingSession` instance ready to be started.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    /// use capture::LogicalSize;
    ///
    /// let config = RecorderConfig::new(
    ///     "HDMI-A-1".to_string(),
    ///     LogicalSize { width: 1920, height: 1080 },
    ///     "recording.mp4".into(),
    /// );
    ///
    /// let session = RecordingSession::new(config);
    /// ```
    pub fn new(config: RecorderConfig) -> Self {
        let (frame_sender, frame_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        let (frame_sender_user, frame_receiver_user) = if config.enable_frame_channel_user {
            let (tx, rx) = bounded(USER_CHANNEL_SIZE);
            (Some(tx), Some(Arc::new(rx)))
        } else {
            (None, None)
        };

        Self {
            config,

            start_time: std::time::Instant::now(),
            total_frame_count: Arc::new(AtomicU64::new(0)),
            loss_frame_count: Arc::new(AtomicU64::new(0)),

            frame_sender: Some(Arc::new(frame_sender)),
            frame_receiver,
            frame_sender_user,
            frame_receiver_user,

            stop_sig: Arc::new(AtomicBool::new(false)),
            stop_sig_combine: Arc::new(AtomicBool::new(false)),
            stop_sig_denoise: Arc::new(AtomicBool::new(false)),
            capture_workers: vec![],

            audio_recorder: None,

            speaker_level_receiver_user: None,
            speaker_recorder_worker: None,
        }
    }

    pub fn output_path(&self) -> PathBuf {
        self.config.output_path.clone()
    }

    /// Start the recording session.
    ///
    /// This method begins the actual recording process by spawning multiple threads
    /// for screen capture, audio recording (if enabled), and starting the recording
    /// pipeline. The session will continue until `stop()` is called or the recording
    /// is completed.
    ///
    /// # Returns
    ///
    /// `Ok(())` if recording started successfully, or `Err(RecorderError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into());
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    /// ```
    pub fn start(&mut self) -> Result<(), RecorderError> {
        let thread_counts = self.evaluate_need_threads();
        if thread_counts == 0 {
            return Err(RecorderError::Other(format!("capture thread counts is 0")));
        }

        log::debug!("thread_counts: {thread_counts}");

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

    /// Wait for recording to complete and process all captured frames.
    ///
    /// This method blocks until the recording session is finished, processing
    /// all captured frames through the encoding pipeline and combining video
    /// and audio tracks into the final output file.
    ///
    /// # Arguments
    ///
    /// * `combine_progress_cb` - Callback function that receives progress updates
    ///   (0.0 to 1.0) during the track combining phase
    ///
    /// # Returns
    ///
    /// `Ok(ProgressState)` indicating whether recording completed successfully
    /// or was stopped, or `Err(RecorderError)` if processing failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig, ProgressState};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into());
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// let result = session.wait(|progress| {
    ///     println!("Combining tracks: {:.1}%", progress * 100.0);
    /// });
    ///
    /// match result {
    ///     Ok(ProgressState::Finished) => println!("Recording completed"),
    ///     Ok(ProgressState::Stopped) => println!("Recording stopped"),
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn wait(
        self,
        denoise_progress_cb: Option<impl FnMut(f32)>,
        combine_progress_cb: impl FnMut(f32),
    ) -> Result<ProgressState, RecorderError> {
        let (encoder_sender, encoder_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        let resize_handles = Self::resize_workers(
            self.frame_receiver.clone(),
            encoder_sender,
            self.frame_sender_user.clone(),
            (self.capture_workers.len() / 2).max(2),
            self.start_time,
            self.config.resolution.clone(),
            self.total_frame_count.clone(),
            self.loss_frame_count.clone(),
            self.config.enable_preview_mode,
        );

        let (width, height) = self.config.resolution.dimensions(
            self.config.screen_logical_size.width as u32,
            self.config.screen_logical_size.height as u32,
        );
        let h264_writer = if self.config.disable_save_file {
            None
        } else {
            Some(H264Writer::new(
                self.config.output_path.with_extension(RAW_VIDEO_EXTENSION),
                ENCODER_WORKER_CHANNEL_SIZE,
            ))
        };

        // Create encoder in main thread since x264 is not thread-safe
        let mut video_encoder = VideoEncoder::new(width, height, self.config.fps)?;

        // Write encoder headers first if we have a writer
        if let Some(ref writer) = h264_writer {
            let headers = video_encoder.headers()?;
            let headers_data = headers.entirety().to_vec();
            writer.write_frame(EncodedFrame::Frame((0, headers_data)));
        }

        loop {
            match encoder_receiver.recv() {
                Ok((total_frame_index, img)) => {
                    if self.config.disable_save_file {
                        continue;
                    }

                    let now = std::time::Instant::now();
                    match video_encoder.encode_frame(img.into()) {
                        Ok(EncodedFrame::Frame((_, encoded_frame))) => {
                            log::debug!(
                                "total encoded frame[{total_frame_index}] {} bytes",
                                encoded_frame.len()
                            );

                            if let Some(ref writer) = h264_writer {
                                writer.write_frame(EncodedFrame::Frame((
                                    total_frame_index,
                                    encoded_frame,
                                )));
                            }
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
                    self.wait_stop(
                        resize_handles,
                        h264_writer,
                        Some(video_encoder),
                        denoise_progress_cb,
                        combine_progress_cb,
                    )?;
                    break;
                }
            }
        }

        return Ok(ProgressState::Stopped);
    }

    /// Enable audio recording with specified device
    fn enable_audio(&mut self, device_name: &str) -> Result<(), RecorderError> {
        let audio_recorder = AudioRecorder::new(if self.config.enable_audio_channel_user {
            Some(USER_CHANNEL_SIZE)
        } else {
            None
        })
        .map_err(|e: AudioError| RecorderError::AudioError(e.to_string()))?;

        let audio_recorder = if self.config.audio_amplification.is_some() {
            audio_recorder
                .with_amplification(self.config.audio_amplification.clone().unwrap())
                .with_real_time_denoise(self.config.enable_denoise && self.config.real_time_denoise)
        } else {
            audio_recorder
        };

        let audio_file_path = self
            .config
            .output_path
            .with_extension(INPUT_AUDIO_EXTENSION);

        let streaming_recorder = StreamingAudioRecorder::start(
            audio_recorder,
            device_name,
            audio_file_path,
            self.config.disable_save_file,
        )
        .map_err(|e: AudioError| RecorderError::AudioError(e.to_string()))?;

        self.audio_recorder = Some(streaming_recorder);

        Ok(())
    }

    fn enable_speaker_audio(&mut self) -> Result<(), RecorderError> {
        let stop_sig = self.stop_sig.clone();
        let save_path = self
            .config
            .output_path
            .with_extension(SPEAKER_AUDIO_EXTENSION);

        let (sender, receiver) = if self.config.enable_speaker_channel_user {
            let (tx, rx) = bounded(USER_CHANNEL_SIZE);
            (Some(Arc::new(tx)), Some(Arc::new(rx)))
        } else {
            (None, None)
        };

        let amplification = self.config.speaker_amplification.clone();
        let disable_save_file = self.config.disable_save_file;
        let handle = thread::spawn(move || {
            let recorder = SpeakerRecorder::new(save_path, stop_sig, sender, disable_save_file)
                .map_err(|e| RecorderError::SpeakerError(e.to_string()))?;

            let mut recorder = if amplification.is_some() {
                recorder.with_amplification(amplification.unwrap())
            } else {
                recorder
            };

            recorder
                .start_recording()
                .map_err(|e| RecorderError::SpeakerError(e.to_string()))?;
            Ok(())
        });

        self.speaker_recorder_worker = Some(handle);
        self.speaker_level_receiver_user = receiver;

        Ok(())
    }

    fn resize_workers(
        capture_receiver: Receiver<Frame>,
        encoder_sender: Sender<EncoderChannelData>,
        frame_sender_user: Option<Sender<FrameUser>>,
        thread_counts: usize,
        start_time: Instant,
        resolution: Resolution,
        total_frame_count: Arc<AtomicU64>,
        loss_frame_count: Arc<AtomicU64>,
        enable_preview_mode: bool,
    ) -> Vec<JoinHandle<()>> {
        let mut thread_handles = vec![];
        let (collect_sender, collect_receiver) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        let (handles, resize_senders) = Self::resize_workers_main(
            collect_sender.clone(),
            thread_counts,
            resolution,
            loss_frame_count.clone(),
        );

        thread_handles.extend(handles.into_iter());

        thread_handles.push(Self::resize_collect_worker(
            collect_receiver,
            encoder_sender,
            loss_frame_count.clone(),
        ));

        thread_handles.push(Self::resize_forward_worker(
            capture_receiver,
            resize_senders,
            frame_sender_user,
            start_time,
            total_frame_count,
            loss_frame_count,
            enable_preview_mode,
        ));

        thread_handles
    }

    fn resize_workers_main(
        collect_sender: Sender<(usize, u64, ResizedImageBuffer)>,
        thread_counts: usize,
        resolution: Resolution,
        loss_frame_count: Arc<AtomicU64>,
    ) -> (Vec<JoinHandle<()>>, Vec<Sender<ResizeChannelData>>) {
        let mut thread_handles = vec![];
        let mut resize_senders = vec![];

        for index in 0..thread_counts {
            let loss_frame_count_clone = loss_frame_count.clone();
            let collect_sender_clone = collect_sender.clone();

            let (resize_sender, resize_receiver) = bounded(RESIZE_WORKER_CHANNEL_SIZE);
            resize_senders.push(resize_sender);

            let handle = thread::spawn(move || {
                loop {
                    match resize_receiver.recv() {
                        Ok((total_frame_index, frame)) => {
                            match Self::resize_frame(frame, resolution) {
                                Err(e) => {
                                    log::warn!("resize thread[{index}] handle frame failed: {e}")
                                }
                                Ok(img) => {
                                    if let Err(e) = collect_sender_clone.try_send((
                                        index,
                                        total_frame_index,
                                        img,
                                    )) {
                                        loss_frame_count_clone.fetch_add(1, Ordering::Relaxed);
                                        log::warn!(
                                            "resize thread[{index}] try send to collect_reciever failed: {e}"
                                        );
                                    }
                                }
                            }
                        }
                        _ => {
                            log::info!("resize thread[{index}] exit");
                            return;
                        }
                    }
                }
            });

            thread_handles.push(handle);
        }

        (thread_handles, resize_senders)
    }

    fn resize_collect_worker(
        collect_receiver: Receiver<(usize, u64, ResizedImageBuffer)>,
        encoder_sender: Sender<EncoderChannelData>,
        loss_frame_count: Arc<AtomicU64>,
    ) -> JoinHandle<()> {
        let loss_frame_count_clone = loss_frame_count.clone();
        let handle = thread::spawn(move || {
            let mut expect_total_frame_index = 1;
            let mut disorder_frame_counts = 0;
            let mut frame_cache: HashMap<u64, (u64, ResizedImageBuffer)> = HashMap::new();

            loop {
                match collect_receiver.recv() {
                    Ok((thread_index, total_frame_index, img)) => {
                        // log::debug!("+++ {total_frame_index} - {expect_total_frame_index}");
                        if expect_total_frame_index == total_frame_index {
                            disorder_frame_counts = 0;

                            if let Err(e) = encoder_sender.try_send((total_frame_index, img)) {
                                loss_frame_count_clone.fetch_add(1, Ordering::Relaxed);
                                log::warn!(
                                    "collected thread try send to encoder reciever failed: {e}"
                                );
                            }

                            loop {
                                expect_total_frame_index += 1;
                                match frame_cache.remove(&expect_total_frame_index) {
                                    Some(frame) => {
                                        if let Err(e) = encoder_sender.try_send(frame) {
                                            loss_frame_count_clone.fetch_add(1, Ordering::Relaxed);
                                            log::warn!(
                                                "collected thread try send to encoder reciever failed: {e}"
                                            );
                                        }
                                    }
                                    _ => break,
                                }
                            }
                        } else if expect_total_frame_index > total_frame_index {
                            loss_frame_count_clone.fetch_add(1, Ordering::Relaxed);
                            log::warn!(
                                "too late thread[{thread_index}] frame, frame index: {total_frame_index}, expected index: {expect_total_frame_index}"
                            );
                        } else {
                            // log::warn!(
                            //     "total_frame_index: {total_frame_index}, expect_total_frame_index {expect_total_frame_index}"
                            // );
                            frame_cache.insert(total_frame_index, (total_frame_index, img));
                            disorder_frame_counts += 1;

                            if disorder_frame_counts > 10 {
                                disorder_frame_counts = 0;

                                log::warn!(
                                    "disorder frame counts > 10. So moving to next expected frame index: {expect_total_frame_index} -> {}",
                                    expect_total_frame_index + 1
                                );

                                loop {
                                    expect_total_frame_index += 1;
                                    match frame_cache.remove(&expect_total_frame_index) {
                                        Some(frame) => {
                                            if let Err(e) = encoder_sender.try_send(frame) {
                                                loss_frame_count_clone
                                                    .fetch_add(1, Ordering::Relaxed);
                                                log::warn!(
                                                    "collected thread try send to encoder reciever failed: {e}"
                                                );
                                            }
                                        }
                                        _ => break,
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        drop(encoder_sender);
                        log::info!("resize collected thread exit");
                        return;
                    }
                }
            }
        });

        handle
    }

    fn resize_forward_worker(
        capture_receiver: Receiver<Frame>,
        resize_senders: Vec<Sender<ResizeChannelData>>,
        frame_sender_user: Option<Sender<FrameUser>>,
        start_time: Instant,
        total_frame_count: Arc<AtomicU64>,
        loss_frame_count: Arc<AtomicU64>,
        enable_preview_mode: bool,
    ) -> JoinHandle<()> {
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

                        if let Some(ref sender) = frame_sender_user {
                            let frame_user = FrameUser {
                                stats: StatsUser {
                                    fps: fps_counter.add_frame(frame.timestamp),
                                    total_frames: total_frame_count,
                                    loss_frames: loss_frame_count.load(Ordering::Relaxed),
                                },

                                frame: if enable_preview_mode {
                                    Some(frame.clone())
                                } else {
                                    None
                                },
                            };
                            if let Err(e) = sender.try_send(frame_user) {
                                log::warn!("try send frame to user frame channel failed: {e}");
                            }
                        }

                        log::debug!("{}", {
                            let mut s = String::default();
                            for (index, sender) in resize_senders.iter().enumerate() {
                                s.push_str(&format!(
                                    "send[{index}] remained: {}. ",
                                    sender.capacity().unwrap_or_default() - sender.len()
                                ));
                            }
                            s
                        });

                        let sender = resize_senders
                            .iter()
                            .min_by(|a, b| a.len().cmp(&b.len()))
                            .unwrap();

                        if let Err(e) = sender.try_send((total_frame_count, frame)) {
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

    fn wait_stop(
        mut self,
        resize_handles: Vec<JoinHandle<()>>,
        mut h264_writer: Option<H264Writer>,
        encoder: Option<VideoEncoder>,
        denoise_progress_cb: Option<impl FnMut(f32)>,
        combine_progress_cb: impl FnMut(f32),
    ) -> Result<(), RecorderError> {
        if let Some(audio_recorder) = self.audio_recorder.take() {
            if let Err(e) = audio_recorder.stop() {
                log::warn!("Failed to stop audio recording: {}", e);
            } else {
                if !self.config.disable_save_file {
                    log::info!(
                        "Successfully save audio recorder file: {}",
                        self.config
                            .output_path
                            .with_extension(INPUT_AUDIO_EXTENSION)
                            .display()
                    );
                }
            }
        }

        if let Some(speaker_recorder_handle) = self.speaker_recorder_worker.take() {
            if let Err(e) = speaker_recorder_handle.join() {
                log::warn!("join speaker recorder thread failed: {:?}", e);
            }
        }

        for (i, thread) in self.capture_workers.into_iter().enumerate() {
            if let Err(e) = thread.join() {
                log::warn!("join capture thread[{i}] failed: {:?}", e);
            }
        }

        for (index, resize_handle) in resize_handles.into_iter().enumerate() {
            if let Err(e) = resize_handle.join() {
                log::warn!("join resize thread[{index}] failed: {:?}", e);
            }
        }

        // Flush encoder if provided to process any delayed frames
        if let Some(encoder) = encoder
            && let Some(ref writer) = h264_writer
        {
            match encoder.flush() {
                Ok(mut flush) => {
                    while let Some(result) = flush.next() {
                        match result {
                            Ok((data, _)) => {
                                let frame_data = data.entirety().to_vec();
                                writer.write_frame(EncodedFrame::Frame((u64::MAX, frame_data)));
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

        if let Some(writer) = h264_writer.take() {
            writer.finish()?;
        }

        if !self.config.disable_save_file {
            if self.config.enable_denoise
                && !self.config.real_time_denoise
                && self.config.audio_device_name.is_some()
            {
                let input_file = self
                    .config
                    .output_path
                    .with_extension(INPUT_AUDIO_EXTENSION);
                let output_file = self
                    .config
                    .output_path
                    .with_extension(INPUT_AUDIO_DENOISE_EXTENSION);

                let denoiser = Denoise::new(input_file, output_file)
                    .map_err(|e| RecorderError::DenoiseError(e.to_string()))?;

                if ProgressState::Stopped
                    == denoiser
                        .process(self.stop_sig_denoise.clone(), denoise_progress_cb)
                        .map_err(|e| RecorderError::DenoiseError(e.to_string()))?
                {
                    return Ok(());
                }
            }

            let tmp_output_file = self
                .config
                .output_path
                .with_extension(TMP_OUTPUT_VIDEO_EXTENSION);

            let combine_config = MergeTracksConfig {
                h264_path: self.config.output_path.with_extension(RAW_VIDEO_EXTENSION),
                input_wav_path: if self.config.audio_device_name.is_some() {
                    Some(
                        if self.config.enable_denoise && !self.config.real_time_denoise {
                            self.config
                                .output_path
                                .with_extension(INPUT_AUDIO_DENOISE_EXTENSION)
                        } else {
                            self.config
                                .output_path
                                .with_extension(INPUT_AUDIO_EXTENSION)
                        },
                    )
                } else {
                    None
                },
                speaker_wav_path: if self.config.enable_recording_speaker {
                    Some(
                        self.config
                            .output_path
                            .with_extension(SPEAKER_AUDIO_EXTENSION),
                    )
                } else {
                    None
                },
                output_path: tmp_output_file.clone(),
                fps: self.config.fps,
                stop_sig: self.stop_sig_combine,
                convert_input_wav_to_mono: self.config.convert_input_wav_to_mono,
            };

            let combine_tracks_state = merge_tracks(combine_config, combine_progress_cb)?;

            if tmp_output_file.exists() {
                _ = fs::rename(&tmp_output_file, &self.config.output_path);
            }

            if self.config.output_path.exists() {
                log::info!(
                    "Successfully save recorded file: {} ",
                    self.config.output_path.display(),
                );
            } else {
                if combine_tracks_state == ProgressState::Finished {
                    return Err(RecorderError::Ffmpeg(format!(
                        "Save recorded file: {} failed. Something wrong with ffmpeg operation",
                        self.config.output_path.display()
                    )));
                }
            }

            if self.config.remove_cache_files && self.config.output_path.exists() {
                if self.config.enable_recording_speaker {
                    _ = fs::remove_file(
                        self.config
                            .output_path
                            .with_extension(SPEAKER_AUDIO_EXTENSION),
                    );
                }

                if self.config.audio_device_name.is_some() {
                    _ = fs::remove_file(
                        self.config
                            .output_path
                            .with_extension(INPUT_AUDIO_EXTENSION),
                    );

                    if self.config.enable_denoise && !self.config.real_time_denoise {
                        _ = fs::remove_file(
                            self.config
                                .output_path
                                .with_extension(INPUT_AUDIO_DENOISE_EXTENSION),
                        );
                    }
                }

                _ = fs::remove_file(self.config.output_path.with_extension(RAW_VIDEO_EXTENSION));
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

    /// Resize a captured image to the target dimensions.
    ///
    /// This method uses high-performance image resizing with the `fast_image_resize`
    /// library to scale captured screen frames to the desired output resolution
    /// while maintaining image quality.
    ///
    /// # Arguments
    ///
    /// * `capture` - The captured screen data
    /// * `target_size` - Target dimensions as `(width, height)`
    ///
    /// # Returns
    ///
    /// `Ok(ResizedImageBuffer)` containing the resized image, or
    /// `Err(RecorderError)` if resizing failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::RecordingSession;
    /// use capture::Capture;
    ///
    /// // Assuming you have a Capture instance
    /// let capture = Capture::new(/* ... */);
    /// let resized = RecordingSession::resize_image(capture, (1920, 1080)).unwrap();
    /// ```
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

        // Create resizer with optimized filter for performance
        let mut resizer = fast_image_resize::Resizer::new();

        // Use fastest algorithm - Nearest neighbor
        let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
            // fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
            fast_image_resize::ResizeAlg::SuperSampling(fast_image_resize::FilterType::Lanczos3, 2),
        );

        // Resize the image
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

    /// Stop the recording session.
    ///
    /// This method signals all recording threads to stop capturing frames.
    /// The session will complete processing of any already-captured frames
    /// when `wait()` is called.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into());
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// // Stop recording after 5 seconds
    /// std::thread::sleep(std::time::Duration::from_secs(5));
    /// session.stop();
    ///
    /// // Wait for processing to complete
    /// session.wait(|_| {}).unwrap();
    /// ```
    pub fn stop(&self) {
        self.stop_sig.store(true, Ordering::Relaxed);
    }

    /// Get the stop signal for external control.
    ///
    /// This method returns a clone of the internal stop signal, which can be used
    /// to monitor or control the recording session from external code.
    ///
    /// # Returns
    ///
    /// An `Arc<AtomicBool>` that can be used to check or set the stop state.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into());
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// let stop_sig = session.stop_sig();
    ///
    /// // In another thread, monitor the stop signal
    /// while !stop_sig.load(std::sync::atomic::Ordering::Relaxed) {
    ///     // Continue processing
    ///     std::thread::sleep(std::time::Duration::from_millis(100));
    /// }
    /// ```
    pub fn stop_sig(&self) -> Arc<AtomicBool> {
        self.stop_sig.clone()
    }

    /// Stop the track combining process.
    ///
    /// This method signals the track combining phase to stop immediately.
    /// Useful for interrupting long-running combining operations.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into());
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// // Stop track combining if it takes too long
    /// session.stop_combine_tracks();
    /// ```
    pub fn stop_combine_tracks(&self) {
        self.stop_sig_combine.store(true, Ordering::Relaxed);
    }

    pub fn get_stop_combine_tracks(&self) -> Arc<AtomicBool> {
        self.stop_sig_combine.clone()
    }

    pub fn stop_denoise(&self) {
        self.stop_sig_denoise.store(true, Ordering::Relaxed);
    }

    pub fn get_stop_denoise(&self) -> Arc<AtomicBool> {
        self.stop_sig_denoise.clone()
    }

    /// Get the user frame receiver if frame channel is enabled.
    ///
    /// This method returns the receiver for the user frame channel, which allows
    /// external code to receive captured frames in real-time. The frame channel
    /// must be enabled in the configuration for this to return a value.
    ///
    /// # Returns
    ///
    /// `Some(Arc<Receiver<Frame>>)` if frame channel is enabled, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into())
    ///     .with_enable_frame_channel_user(true);
    ///
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// if let Some(receiver) = session.get_frame_receiver_user() {
    ///     while let Ok(frame) = receiver.recv() {
    ///         println!("Received frame from thread {}", frame.thread_id);
    ///     }
    /// }
    /// ```
    pub fn get_frame_receiver_user(&self) -> Option<Arc<Receiver<FrameUser>>> {
        self.frame_receiver_user.clone()
    }

    /// Get the user audio level receiver if audio channel is enabled.
    ///
    /// This method returns the receiver for the input audio level channel,
    /// which provides real-time audio level measurements in decibels.
    /// The audio channel must be enabled in the configuration for this to return a value.
    ///
    /// # Returns
    ///
    /// `Some(Arc<Receiver<f32>>)` if audio channel is enabled, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into())
    ///     .with_enable_audio_channel_user(true)
    ///     .with_audio_device_name(Some("default".to_string()));
    ///
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// if let Some(receiver) = session.get_audio_level_receiver_user() {
    ///     while let Ok(level) = receiver.recv() {
    ///         println!("Input audio level: {:.1} dB", level);
    ///     }
    /// }
    /// ```
    pub fn get_audio_level_receiver_user(&self) -> Option<Arc<Receiver<f32>>> {
        if let Some(ref recorder) = self.audio_recorder {
            recorder.get_audio_level_receiver()
        } else {
            None
        }
    }

    /// Get the user speaker level receiver if speaker channel is enabled.
    ///
    /// This method returns the receiver for the speaker audio level channel,
    /// which provides real-time speaker audio level measurements in decibels.
    /// The speaker channel must be enabled in the configuration for this to return a value.
    ///
    /// # Returns
    ///
    /// `Some(Arc<Receiver<f32>>)` if speaker channel is enabled, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::{RecordingSession, RecorderConfig};
    ///
    /// let config = RecorderConfig::new("output".to_string(), Default::default(), "output.mp4".into())
    ///     .with_enable_speaker_channel_user(true)
    ///     .with_enable_recording_speaker(true);
    ///
    /// let mut session = RecordingSession::new(config);
    /// session.start().unwrap();
    ///
    /// if let Some(receiver) = session.get_speaker_level_receiver_user() {
    ///     while let Ok(level) = receiver.recv() {
    ///         println!("Speaker audio level: {:.1} dB", level);
    ///     }
    /// }
    /// ```
    pub fn get_speaker_level_receiver_user(&self) -> Option<Arc<Receiver<f32>>> {
        self.speaker_level_receiver_user.clone()
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
