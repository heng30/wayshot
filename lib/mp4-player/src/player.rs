use super::{
    MP4PlayerError, Result,
    metadata::{self, AudioMetadata, MediaMetadata, VideoMetadata},
    video_decoder::VideoDecoder,
};
use crossbeam::channel::{Receiver, Sender, bounded};
use derive_setters::Setters;
use fdk_aac::dec::{Decoder, Transport};
use image::{ImageBuffer, Rgb};
use rodio::{OutputStreamBuilder, Sink, buffer::SamplesBuffer};
use std::{
    collections::VecDeque,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const FRAME_CACHE_SIZE: usize = 32;
const VIDEO_FRAME_CHANNEL_SIZE: usize = FRAME_CACHE_SIZE;

pub enum DecodedVideoFrame {
    Empty,
    Data(VideoFrame),
    EOF,
    None,
}

#[derive(Debug)]
pub struct VideoFrame {
    pub track_id: u32,
    pub image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>>,
    pub timestamp: Duration,
    pub duration: Duration,
    pub is_keyframe: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct Config {
    #[setters(skip)]
    file_path: PathBuf,

    #[setters(skip)]
    video_sender: Sender<DecodedVideoFrame>,

    #[setters(skip)]
    video_receiver: Receiver<DecodedVideoFrame>,

    stop_sig: Arc<AtomicBool>,

    sound: Arc<AtomicU32>,
}

impl Config {
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        let (video_sender, video_receiver) = bounded(VIDEO_FRAME_CHANNEL_SIZE);

        Self {
            file_path: file_path.as_ref().to_path_buf(),
            video_sender,
            video_receiver,
            stop_sig: Arc::new(AtomicBool::new(false)),
            sound: Arc::new(AtomicU32::new(100)),
        }
    }
}

pub struct Mp4Player {
    config: Config,
    metadata: MediaMetadata,
    video_thread: Option<JoinHandle<()>>,
    audio_thread: Option<JoinHandle<()>>,
    frame_extractor_thread: Option<JoinHandle<()>>,
}

impl Mp4Player {
    pub fn new(config: Config) -> Result<Self> {
        let metadata = metadata::parse(&config.file_path)?;

        if metadata.video.is_none() {
            return Err(MP4PlayerError::TrackError(
                "No found video track".to_string(),
            ));
        }

        if metadata.video.as_ref().unwrap().frame_rate <= 0.0 {
            return Err(MP4PlayerError::TrackError(
                "Video track frame rate is zero".to_string(),
            ));
        }

        if metadata.audio.len() > 1 {
            return Err(MP4PlayerError::TrackError(format!(
                "Only support one audio track. current are {} tracks",
                metadata.audio.len()
            )));
        }

        Ok(Self {
            config,
            metadata,
            video_thread: None,
            audio_thread: None,
            frame_extractor_thread: None,
        })
    }

    pub fn stop(&mut self) -> Result<()> {
        self.config.stop_sig.store(true, Ordering::Relaxed);

        if let Some(handle) = self.frame_extractor_thread.take()
            && let Err(e) = handle.join()
        {
            return Err(MP4PlayerError::PlayerStopError(format!(
                "Stop mp4 player frame extractor thread failed: {e:?}"
            )));
        }

        if let Some(handle) = self.video_thread.take()
            && let Err(e) = handle.join()
        {
            return Err(MP4PlayerError::PlayerStopError(format!(
                "Stop mp4 player video thread failed: {e:?}"
            )));
        }

        if let Some(handle) = self.audio_thread.take()
            && let Err(e) = handle.join()
        {
            return Err(MP4PlayerError::PlayerStopError(format!(
                "Stop mp4 player audio thread failed: {e:?}"
            )));
        }

        self.config.stop_sig.store(false, Ordering::Relaxed);
        while let Ok(_) = self.config.video_receiver.try_recv() {}

        Ok(())
    }

    pub fn video_frame_receiver(&self) -> Receiver<DecodedVideoFrame> {
        self.config.video_receiver.clone()
    }

    pub fn play(&mut self, start_time: Duration) {
        let (frame_request_sender, frame_request_receiver) = bounded(1);
        let (frame_response_sender, frame_response_receiver) = bounded(FRAME_CACHE_SIZE);

        // Spawn frame extractor thread
        let extractor_config = self.config.clone();
        let extractor_metadata = self.metadata.video.clone().unwrap();
        let extractor_handle = thread::spawn(move || {
            if let Err(e) = Self::video_frame_extractor_loop(
                extractor_config,
                extractor_metadata,
                frame_request_receiver,
                frame_response_sender,
            ) {
                log::warn!("Error in frame extractor thread: {}", e);
            }
            log::info!("Exit frame extractor thread");
        });
        self.frame_extractor_thread = Some(extractor_handle);

        let config = self.config.clone();
        let metadata = self.metadata.video.clone().unwrap();
        let timing_handle = thread::spawn(move || {
            config.stop_sig.store(false, Ordering::Relaxed);
            if let Err(e) = Self::video_timing_loop(
                config,
                metadata,
                start_time,
                frame_request_sender,
                frame_response_receiver,
            ) {
                log::warn!("Error in timing thread: {}", e);
            }
            log::info!("Exit timing thread");
        });
        self.video_thread = Some(timing_handle);

        if let Some(metadata) = self.metadata.audio.first() {
            let config = self.config.clone();
            let metadata = metadata.clone();
            let handle = thread::spawn(move || {
                if let Err(e) = Self::play_audio(config, metadata, start_time) {
                    log::warn!("play mp4 audio failed: {e}");
                }

                log::info!("Exit mp4 audio thread");
            });

            self.audio_thread = Some(handle)
        }
    }

    fn video_frame_extractor_loop(
        config: Config,
        metadata: VideoMetadata,
        frame_request_receiver: Receiver<(u32, u32)>,
        frame_response_sender: Sender<DecodedVideoFrame>,
    ) -> Result<()> {
        let mut mp4_reader = Self::initialize_mp4_reader(&config.file_path)?;
        let mut decoder = VideoDecoder::new(metadata.width, metadata.height)?;

        loop {
            if config.stop_sig.load(Ordering::Relaxed) {
                break;
            }

            match frame_request_receiver.recv_timeout(Duration::from_millis(10)) {
                Ok((start_frame, frame_count)) => {
                    if start_frame >= metadata.sample_count {
                        if let Err(e) = frame_response_sender.send(DecodedVideoFrame::EOF) {
                            log::warn!(
                                "video_frame_extractor_loop send `DecodedVideoFrame::EOF` failed: {e:?}"
                            );
                        } else {
                            log::info!(
                                "video_frame_extractor_loop send `DecodedVideoFrame::EOF` sucessfully"
                            );
                        }
                        break;
                    }

                    let frames_to_load =
                        std::cmp::min(frame_count as u32, metadata.sample_count - start_frame);

                    Self::extract_and_decode_video_frames(
                        &mut mp4_reader,
                        &metadata,
                        start_frame,
                        frames_to_load as usize,
                        &mut decoder,
                        frame_response_sender.clone(),
                        config.stop_sig.clone(),
                    );

                    if start_frame + frame_count >= metadata.sample_count {
                        if let Err(e) = frame_response_sender.send(DecodedVideoFrame::EOF) {
                            log::warn!(
                                "extract_and_decode_video_frames send `DecodedVideoFrame::EOF` failed: {e:?}"
                            );
                        } else {
                            log::info!(
                                "extract_and_decode_video_frames send `DecodedVideoFrame::EOF` sucessfully"
                            );
                        }
                        break;
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn video_timing_loop(
        config: Config,
        metadata: VideoMetadata,
        start_time: Duration,
        frame_request_sender: Sender<(u32, u32)>,
        frame_response_receiver: Receiver<DecodedVideoFrame>,
    ) -> Result<()> {
        let total_video_frames = metadata.sample_count;
        let frame_duration = Duration::from_secs_f64(1.0 / metadata.frame_rate);
        let mut request_video_frame = (start_time.as_secs_f64() * metadata.frame_rate) as u32 + 1;
        let mut frame_cache: VecDeque<DecodedVideoFrame> = VecDeque::new();
        let mut current_frame_index = 0u32;
        let mut last_request_finished = false;

        if request_video_frame >= total_video_frames {
            return Err(MP4PlayerError::FrameError(format!(
                "request_video_frame[{request_video_frame}] >= total_video_frames[{total_video_frames}] "
            )));
        }

        _ = frame_request_sender.send((request_video_frame, FRAME_CACHE_SIZE as u32));

        let start_instant = Instant::now();
        let start_frame_index = request_video_frame;

        'out: loop {
            let mut reach_end = false;
            if let Some(frame) = frame_cache.pop_front() {
                reach_end = matches!(frame, DecodedVideoFrame::EOF);

                if let Err(e) = config.video_sender.try_send(frame) {
                    log::warn!("Failed to send video frame: {e}");
                }
                current_frame_index += 1;
            }

            let mut try_counts = 0;
            while let Ok(frame) = frame_response_receiver.try_recv() {
                if config.stop_sig.load(Ordering::Relaxed) {
                    break 'out;
                }

                match frame {
                    DecodedVideoFrame::None => {
                        last_request_finished = true;
                    }
                    DecodedVideoFrame::EOF => {
                        frame_cache.push_back(frame);
                        last_request_finished = true;
                    }
                    DecodedVideoFrame::Data(_) | DecodedVideoFrame::Empty => {
                        frame_cache.push_back(frame);
                        request_video_frame += 1;
                        last_request_finished = false;
                    }
                }

                if try_counts > 5 {
                    break;
                }
                try_counts += 1;
            }

            if last_request_finished
                && frame_cache.len() < FRAME_CACHE_SIZE / 2
                && request_video_frame < total_video_frames
            {
                log::debug!(
                    "start request more frames. frame cache len: {}, video sender channel remained: {}",
                    frame_cache.len(),
                    config.video_sender.capacity().unwrap() - config.video_sender.len()
                );

                if frame_request_sender
                    .try_send((request_video_frame, FRAME_CACHE_SIZE as u32 / 2))
                    .is_ok()
                {
                    last_request_finished = false;
                }
            }

            if config.stop_sig.load(Ordering::Relaxed) {
                break;
            }

            if start_frame_index + current_frame_index >= total_video_frames || reach_end {
                // resend `DecodedVideoFrame::EOF` to ensure `DecodedVideoFrame::EOF` be sent
                if let Err(e) = config.video_sender.try_send(DecodedVideoFrame::EOF) {
                    log::warn!("Failed to send video frame: {e}");
                }

                config.stop_sig.store(true, Ordering::Relaxed);
                break;
            }

            spin_sleep::sleep_until(start_instant + current_frame_index * frame_duration);
        }

        log::info!("Video timing loop completed, processed {current_frame_index} frames");
        Ok(())
    }

    fn extract_and_decode_video_frames(
        mp4_reader: &mut mp4::Mp4Reader<BufReader<File>>,
        metadata: &VideoMetadata,
        start_frame: u32,
        max_frames: usize,
        decoder: &mut VideoDecoder,
        frame_response_sender: Sender<DecodedVideoFrame>,
        stop_sig: Arc<AtomicBool>,
    ) {
        let mut decoded_frame_count = 0;
        let mut empty_frame_count = 0;
        let end_frame = std::cmp::min(start_frame + max_frames as u32, metadata.sample_count);

        log::debug!(
            "Extracting video frames from sample {} to {} of {}",
            start_frame,
            end_frame,
            metadata.sample_count
        );

        for id in start_frame..end_frame {
            if stop_sig.load(Ordering::Relaxed) {
                break;
            }

            match mp4_reader.read_sample(metadata.track_id, id) {
                Ok(Some(sample)) => {
                    let timestamp = Duration::from_secs_f64(
                        sample.start_time as f64 / metadata.timescale as f64,
                    );
                    let duration =
                        Duration::from_secs_f64(sample.duration as f64 / metadata.timescale as f64);

                    if sample.bytes.is_empty() {
                        if let Err(e) = frame_response_sender.send(DecodedVideoFrame::Empty) {
                            log::warn!("frame_response_sender send decoded frame failed: {e}");
                        }

                        empty_frame_count += 1;
                        continue;
                    }

                    match decoder.decode_frame(&sample.bytes) {
                        Ok(Some(decoded_frame)) => match decoded_frame.to_image_buffer() {
                            Ok(image_buffer) => {
                                let frame = DecodedVideoFrame::Data(VideoFrame {
                                    track_id: metadata.track_id,
                                    image_buffer,
                                    timestamp,
                                    duration,
                                    is_keyframe: sample.is_sync,
                                    width: metadata.width,
                                    height: metadata.height,
                                });

                                if let Err(e) = frame_response_sender.send(frame) {
                                    log::warn!(
                                        "frame_response_sender send decoded frame failed: {e}"
                                    );
                                }

                                decoded_frame_count += 1;
                            }
                            Err(e) => {
                                if let Err(e) = frame_response_sender.send(DecodedVideoFrame::Empty)
                                {
                                    log::warn!(
                                        "frame_response_sender send decoded frame failed: {e}"
                                    );
                                }

                                log::warn!("Failed to convert decoded frame to image buffer: {e}");
                                empty_frame_count += 1;
                            }
                        },
                        Ok(None) => {
                            if let Err(e) = frame_response_sender.send(DecodedVideoFrame::Empty) {
                                log::warn!("frame_response_sender send decoded frame failed: {e}");
                            }

                            log::debug!(
                                "Decoder needs more data for frame at timestamp: {timestamp:?}"
                            );
                            empty_frame_count += 1;
                        }
                        Err(e) => {
                            if let Err(e) = frame_response_sender.send(DecodedVideoFrame::Empty) {
                                log::warn!("frame_response_sender send decoded frame failed: {e}");
                            }

                            log::warn!("Failed to decode frame at timestamp {timestamp:?}: {e}");
                            empty_frame_count += 1;
                        }
                    }
                }
                Ok(None) => {
                    log::debug!("No sample data for video sample {id}");
                    break;
                }
                Err(e) => log::warn!("Error reading video sample {id}: {e}"),
            }
        }

        if let Err(e) = frame_response_sender.send(DecodedVideoFrame::None) {
            log::warn!("frame_response_sender send `DecodedVideoFrame::None` failed: {e}");
        }

        log::debug!(
            "Decoded {decoded_frame_count} frames, empty {empty_frame_count} frames of {}",
            end_frame - start_frame
        );
    }

    fn play_audio(config: Config, metadata: AudioMetadata, start_time: Duration) -> Result<()> {
        let mut mp4_reader = Self::initialize_mp4_reader(&config.file_path)?;
        let start_sample = Self::find_start_audio_sample_id(&mut mp4_reader, &metadata, start_time)
            .ok_or(MP4PlayerError::FrameError(format!(
                "No found matched sample id for {start_time:.2?}"
            )))?;

        let decoder = Decoder::new(Transport::Adts);
        let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
            MP4PlayerError::TrackError(format!("Failed to create audio output stream: {e}"))
        })?;
        let sink = Sink::connect_new(stream.mixer());

        Self::process_audio_samples(
            &config,
            &mut mp4_reader,
            &metadata,
            start_sample,
            decoder,
            &sink,
        );

        while !sink.empty() && !config.stop_sig.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
        }

        log::debug!("Audio playback duration completed");
        Ok(())
    }

    fn initialize_mp4_reader(file_path: &Path) -> Result<mp4::Mp4Reader<BufReader<File>>> {
        let file = File::open(file_path)?;
        let file_size = file.metadata()?.len();
        Ok(mp4::Mp4Reader::read_header(
            BufReader::new(file),
            file_size,
        )?)
    }

    fn find_start_audio_sample_id(
        mp4_reader: &mut mp4::Mp4Reader<BufReader<File>>,
        metadata: &AudioMetadata,
        start_time: Duration,
    ) -> Option<u32> {
        let target_time = (start_time.as_secs_f64() * metadata.timescale as f64) as u64;

        for sample_id in 1..metadata.sample_count {
            match mp4_reader.read_sample(metadata.track_id, sample_id) {
                Ok(Some(sample)) => {
                    if sample.start_time >= target_time {
                        return Some(sample_id);
                    }
                }
                Ok(None) => {
                    log::debug!("No sample data for audio sample {sample_id}");
                    break;
                }
                Err(e) => {
                    log::warn!("Error reading audio sample {sample_id}: {e}");
                    break;
                }
            }
        }

        None
    }

    fn process_audio_samples(
        config: &Config,
        mp4_reader: &mut mp4::Mp4Reader<BufReader<File>>,
        metadata: &AudioMetadata,
        start_sample: u32,
        mut decoder: Decoder,
        sink: &Sink,
    ) {
        log::info!(
            "Starting audio processing loop: frames {} to {} (total: {})",
            start_sample,
            metadata.sample_count,
            metadata.sample_count - start_sample
        );

        for id in start_sample..metadata.sample_count {
            if config.stop_sig.load(Ordering::Relaxed) {
                break;
            }

            sink.play();
            sink.set_volume(config.sound.load(Ordering::Relaxed) as f32 / 100.0);

            match mp4_reader.read_sample(metadata.track_id, id) {
                Ok(Some(sample)) => {
                    if let Err(e) = decoder.fill(&sample.bytes) {
                        log::debug!("Failed to fill AAC decoder with frame {}: {}", id, e);
                        continue;
                    }

                    let mut pcm_buffer = vec![0i16; 1024 * metadata.channels as usize];
                    match decoder.decode_frame(&mut pcm_buffer) {
                        Ok(()) => {
                            let actual_frame_size = decoder.decoded_frame_size();
                            if actual_frame_size == 0 {
                                continue;
                            }

                            let f32_samples = pcm_buffer[..actual_frame_size]
                                .iter()
                                .map(|&sample| sample as f32 / i16::MAX as f32)
                                .collect::<Vec<_>>();

                            sink.append(SamplesBuffer::new(
                                metadata.channels,
                                metadata.sample_rate,
                                f32_samples,
                            ));

                            while sink.len() > 10 && !config.stop_sig.load(Ordering::Relaxed) {
                                std::thread::sleep(Duration::from_millis(10));
                            }
                        }
                        Err(e) => log::debug!("Failed to decode frame {id}: {e}"),
                    }
                }
                Ok(None) => {
                    log::info!(
                        "No sample data for audio sample {id} - this might be the end of audio track"
                    );
                    break;
                }
                Err(e) => log::warn!("Error reading audio sample {id}: {e}"),
            }
        }

        sink.stop();
    }
}

impl Drop for Mp4Player {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            log::warn!("Mp4Player stop failed: {e}");
        }
    }
}
