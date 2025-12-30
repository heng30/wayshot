use crate::{
    AudioRecorder, RecorderError, RecordingSession, SpeakerRecorder, platform_speaker_recoder,
    recorder::ENCODER_WORKER_CHANNEL_SIZE, speaker_recorder::SpeakerRecorderConfig,
};
use crossbeam::channel::{Receiver, Sender, bounded};
use hound::WavSpec;
use mp4m::{
    AudioConfig, AudioProcessor, AudioProcessorConfigBuilder, Mp4Processor,
    Mp4ProcessorConfigBuilder, OutputDestination, VideoConfig, VideoFrameType,
};
use once_cell::sync::Lazy;
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{Notify, broadcast};
use wrtc::client::convert_annexb_to_length_prefixes;
use wrtc::{
    Event, OpusChannels, PacketData, PacketDataSender, WebRTCServer, WebRTCServerConfig,
    opus::OpusCoder,
    session::{AudioInfo, MediaInfo, VideoInfo, WebRTCServerSessionConfig},
};

pub(crate) const AUDIO_MIXER_CHANNEL_SIZE: usize = 1024;
pub(crate) static SHARE_SCREEN_CONNECTIONS_COUNT: AtomicU32 = AtomicU32::new(0);
static SHARE_SCREEN_CONNECTIONS: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::default()));

impl RecordingSession {
    pub(crate) fn mix_audio_tracks(
        &mut self,
    ) -> Result<
        (
            Option<Sender<Vec<f32>>>,
            Option<Sender<Vec<f32>>>,
            Option<Receiver<Vec<f32>>>,
            Option<u16>,
            Option<u32>,
        ),
        RecorderError,
    > {
        let mut specs = vec![];
        let (mut audio_sender, mut speak_sender) = (None, None);
        let mut mix_audio_receiver = None;
        let mut mix_audio_sample_rate = None;
        let mut mix_audio_channels = None;

        if let Some(ref device_name) = self.config.audio_device_name {
            specs.push(AudioRecorder::new().spec(device_name)?);
        }

        if self.config.enable_recording_speaker {
            specs.push(platform_speaker_recoder(SpeakerRecorderConfig::default())?.spec());
        }

        if !specs.is_empty() {
            let (mix_audios_tx, mix_audio_rx) = bounded(AUDIO_MIXER_CHANNEL_SIZE);
            mix_audio_receiver = Some(mix_audio_rx);

            let target_sample_rate = specs
                .iter()
                .max_by_key(|item| item.sample_rate)
                .unwrap()
                .sample_rate;
            mix_audio_sample_rate = Some(target_sample_rate);

            let target_channels = if self.config.convert_to_mono {
                1
            } else {
                specs
                    .iter()
                    .max_by_key(|item| item.channels)
                    .unwrap()
                    .channels
            };
            mix_audio_channels = Some(target_channels);

            let config = AudioProcessorConfigBuilder::default()
                .target_sample_rate(target_sample_rate)
                .channel_size(AUDIO_MIXER_CHANNEL_SIZE)
                .convert_to_mono(self.config.convert_to_mono)
                .output_destination(Some(OutputDestination::<f32>::Channel(mix_audios_tx)))
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

                    thread::sleep(Duration::from_millis(10));
                }
            });

            self.audio_mixer_worker = Some(handle);
        }

        Ok((
            audio_sender,
            speak_sender,
            mix_audio_receiver,
            mix_audio_channels,
            mix_audio_sample_rate,
        ))
    }

    pub(crate) fn mp4_worker(
        &mut self,
        video_encoder_header_data: Option<Vec<u8>>,
        mut mix_audio_receiver: Option<Receiver<Vec<f32>>>,
        mix_audio_channels: Option<u16>,
        mix_audio_sample_rate: Option<u32>,
    ) -> Result<Option<Sender<VideoFrameType>>, RecorderError> {
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

        let mut mp4_audio_sender = if let Some(sample_rate) = mix_audio_sample_rate
            && let Some(channels) = mix_audio_channels
        {
            let sender = mp4_processor.add_audio_track(AudioConfig {
                convert_to_mono: false,
                spec: WavSpec {
                    channels: channels,
                    sample_rate: sample_rate,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            })?;
            Some(sender)
        } else {
            None
        };

        if let Some(mp4_audio_tx) = mp4_audio_sender.take()
            && let Some(mix_audio_rx) = mix_audio_receiver.take()
        {
            let stop_sig = self.stop_sig.clone();
            thread::spawn(move || {
                loop {
                    if stop_sig.load(Ordering::Relaxed) {
                        break;
                    }

                    while let Ok(data) = mix_audio_rx.try_recv() {
                        if let Err(e) = mp4_audio_tx.try_send(data) {
                            log::warn!("forward mix audio samples to mp4 processor faild: {e}");
                        }
                    }

                    thread::sleep(Duration::from_millis(10));
                }
            });
        }

        let h264_frame_sender = Some(mp4_processor.h264_sender());
        let handle = thread::spawn(move || {
            if let Err(e) = mp4_processor.run_processing_loop(video_encoder_header_data) {
                log::warn!("MP4 processing error: {}", e);
            }
        });
        self.mp4_writer_worker = Some(handle);

        Ok(h264_frame_sender)
    }

    pub(crate) fn share_screen_worker(
        &mut self,
        rt_handle: tokio::runtime::Handle,
        video_encoder_header_data: Option<Vec<u8>>,
        mix_audio_receiver: Option<Receiver<Vec<f32>>>,
        mix_audio_channels: Option<u16>,
        mix_audio_sample_rate: Option<u32>,
    ) -> Result<Option<Sender<VideoFrameType>>, RecorderError> {
        let exit_notify = Arc::new(Notify::new());
        let (packet_sender, _) = broadcast::channel(ENCODER_WORKER_CHANNEL_SIZE);

        let (mp4_mix_audio_sender, mp4_mix_audio_receiver) =
            if self.config.share_screen_config.save_mp4 && mix_audio_receiver.is_some() {
                let (tx, rx) = bounded::<Vec<f32>>(AUDIO_MIXER_CHANNEL_SIZE);
                (Some(tx), Some(rx))
            } else {
                (None, None)
            };

        let mp4_h264_frame_sender = if self.config.share_screen_config.save_mp4 {
            log::info!("start mp4_worker...");
            let converted_header_data =
                video_encoder_header_data.map(|data| convert_annexb_to_length_prefixes(&data));

            self.mp4_worker(
                converted_header_data,
                mp4_mix_audio_receiver,
                mix_audio_channels,
                mix_audio_sample_rate,
            )?
        } else {
            None
        };

        let h264_frame_sender = self.send_share_screen_packets(
            packet_sender.clone(),
            mp4_h264_frame_sender,
            mp4_mix_audio_sender,
            mix_audio_receiver,
            mix_audio_channels,
            mix_audio_sample_rate,
            exit_notify.clone(),
        );

        self.start_share_screen_server(
            rt_handle,
            packet_sender,
            mix_audio_channels,
            mix_audio_sample_rate,
            exit_notify,
        );

        Ok(Some(h264_frame_sender))
    }

    pub(crate) fn push_stream_worker(
        &mut self,
        _rt_handle: tokio::runtime::Handle,
        video_encoder_header_data: Option<Vec<u8>>,
        mix_audio_receiver: Option<Receiver<Vec<f32>>>,
        mix_audio_channels: Option<u16>,
        mix_audio_sample_rate: Option<u32>,
    ) -> Result<Option<Sender<VideoFrameType>>, RecorderError> {
        let (mp4_mix_audio_sender, mp4_mix_audio_receiver) =
            if self.config.push_stream_config.save_mp4
                && mix_audio_sample_rate.is_some()
                && mix_audio_channels.is_some()
            {
                let (tx, rx) = bounded::<Vec<f32>>(AUDIO_MIXER_CHANNEL_SIZE);
                (Some(tx), Some(rx))
            } else {
                (None, None)
            };

        let mp4_h264_frame_sender = if self.config.push_stream_config.save_mp4 {
            log::info!("start mp4_worker...");
            let converted_header_data = video_encoder_header_data
                .clone()
                .map(|data| convert_annexb_to_length_prefixes(&data));

            self.mp4_worker(
                converted_header_data,
                mp4_mix_audio_receiver,
                mix_audio_channels,
                mix_audio_sample_rate,
            )?
        } else {
            None
        };

        let h264_frame_sender = self.start_push_stream(
            video_encoder_header_data,
            mp4_h264_frame_sender,
            mp4_mix_audio_sender,
            mix_audio_receiver,
            mix_audio_channels,
            mix_audio_sample_rate,
        )?;

        Ok(Some(h264_frame_sender))
    }

    fn send_share_screen_packets(
        &mut self,
        packet_sender: PacketDataSender,
        mp4_h264_frame_sender: Option<Sender<VideoFrameType>>,
        mp4_mix_audio_sender: Option<Sender<Vec<f32>>>,
        mix_audio_receiver: Option<Receiver<Vec<f32>>>,
        mix_audio_channels: Option<u16>,
        mix_audio_sample_rate: Option<u32>,
        exit_notify: Arc<Notify>,
    ) -> Sender<VideoFrameType> {
        let stop_sig = self.stop_sig.clone();
        let (h264_frame_sender, h264_frame_receiver) =
            bounded::<VideoFrameType>(ENCODER_WORKER_CHANNEL_SIZE);

        let handle = thread::spawn(move || {
            let mut no_data = true;
            let mut mix_audio_samples = vec![];

            let mut opus_coder = if let Some(channels) = mix_audio_channels
                && let Some(sample_rate) = mix_audio_sample_rate
            {
                let channels = if channels == 1 {
                    OpusChannels::Mono
                } else if channels == 2 {
                    OpusChannels::Stereo
                } else {
                    unreachable!("audio mixer channels count greater than 2");
                };

                Some(OpusCoder::new(sample_rate, channels).unwrap())
            } else {
                None
            };

            loop {
                if stop_sig.load(Ordering::Relaxed) {
                    if let Some(ref sender) = mp4_h264_frame_sender
                        && let Err(e) = sender.try_send(VideoFrameType::End)
                    {
                        log::warn!("mp4_h264_frame_sender try send `End` failed: {e}");
                    }

                    exit_notify.notify_waiters();
                    break;
                }

                if let Some(ref receiver) = mix_audio_receiver
                    && let Ok(data) = receiver.try_recv()
                {
                    if let Some(ref sender) = mp4_mix_audio_sender
                        && let Err(e) = sender.try_send(data.clone())
                    {
                        log::warn!("try send audio data to mp4_worker failed: {e}");
                    }

                    if let Some(ref mut opus_coder) = opus_coder
                        && !SHARE_SCREEN_CONNECTIONS.lock().unwrap().is_empty()
                    {
                        mix_audio_samples.extend_from_slice(&data);

                        let mut sent_frame_count = 0;
                        let samples_per_frame = opus_coder.input_samples_per_frame();

                        for (frame_idx, frame) in mix_audio_samples
                            .chunks_exact(samples_per_frame)
                            .enumerate()
                        {
                            sent_frame_count += 1;

                            match opus_coder.encode(&frame) {
                                Ok(opus_data) => {
                                    if let Err(e) = packet_sender.send(PacketData::Audio {
                                        timestamp: Instant::now(),
                                        duration: opus_coder.frame_duration(),
                                        data: opus_data.into(),
                                    }) {
                                        log::warn!("send audio data failed: {e}");
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Encoding frame {} failed: {}", frame_idx + 1, e);
                                    if let Err(e) = packet_sender.send(PacketData::Audio {
                                        timestamp: Instant::now(),
                                        duration: opus_coder.frame_duration(),
                                        data: vec![].into(),
                                    }) {
                                        log::warn!("send empty audio data failed: {e}");
                                    }
                                }
                            }
                        }

                        mix_audio_samples.drain(0..sent_frame_count * samples_per_frame);
                    }
                }

                if let Some(ref c) = mix_audio_receiver {
                    no_data = c.is_empty();
                }

                if let Ok(data) = h264_frame_receiver.try_recv() {
                    log::trace!(
                        "receiver h264 frame: {} bytes",
                        match data {
                            VideoFrameType::Frame(ref content) => content.len(),
                            _ => 0,
                        }
                    );
                    no_data = false;

                    if let Some(ref sender) = mp4_h264_frame_sender {
                        let converted_data = match data {
                            VideoFrameType::Frame(ref content) => {
                                VideoFrameType::Frame(convert_annexb_to_length_prefixes(&content))
                            }
                            VideoFrameType::End => VideoFrameType::End,
                        };

                        if let Err(e) = sender.try_send(converted_data) {
                            log::warn!("try send h264 frame to mp4_worker failed: {e}");
                        }
                    }

                    if let VideoFrameType::Frame(data) = data
                        && !SHARE_SCREEN_CONNECTIONS.lock().unwrap().is_empty()
                        && let Err(e) = packet_sender.send(PacketData::Video {
                            timestamp: Instant::now(),
                            data: data.into(),
                        })
                    {
                        log::warn!("share screen send h264 nal data failed: {e}");
                    };
                }

                if no_data {
                    no_data = h264_frame_receiver.is_empty();
                }

                log::trace!(
                    "connections: {}, h264_frame_receiver: {}, mix_audio_receiver: {:?}",
                    SHARE_SCREEN_CONNECTIONS.lock().unwrap().len(),
                    h264_frame_receiver.len(),
                    if let Some(ref c) = mix_audio_receiver {
                        Some(c.len())
                    } else {
                        None
                    }
                );

                if no_data {
                    thread::sleep(Duration::from_millis(10));
                }
            }

            log::info!("share_screen_worker exit...");
        });
        self.share_screen_worker = Some(handle);

        h264_frame_sender
    }

    fn start_share_screen_server(
        &mut self,
        rt_handle: tokio::runtime::Handle,
        packet_sender: PacketDataSender,
        mix_audio_channels: Option<u16>,
        mix_audio_sample_rate: Option<u32>,
        exit_notify: Arc<Notify>,
    ) {
        let audio_info = if let Some(channels) = mix_audio_channels
            && let Some(sample_rate) = mix_audio_sample_rate
        {
            Some(
                AudioInfo::default()
                    .with_channels(channels)
                    .with_sample_rate(sample_rate),
            )
        } else {
            None
        };

        let (encoder_width, encoder_height) = self.config.resolution.dimensions(
            self.config.screen_size.width as u32,
            self.config.screen_size.height as u32,
        );

        let video_info = VideoInfo::default()
            .with_width(encoder_width as i32)
            .with_height(encoder_height as i32)
            .with_fps(self.config.fps.to_u32() as u16);

        let mut media_info = MediaInfo::default()
            .with_audio(audio_info)
            .with_video(video_info)
            .with_disable_host_ipv6(self.config.share_screen_config.disable_host_ipv6);

        if self.config.share_screen_config.stun_server.is_some() {
            media_info.ice_servers.clear(); // contain default stun serever
        }

        if let Some(ref stun) = self.config.share_screen_config.stun_server {
            media_info.ice_servers.push(stun.clone());
        }

        if let Some(ref turn) = self.config.share_screen_config.turn_server {
            media_info.ice_servers.push(turn.clone());
        }

        let (event_sender, mut event_receiver) = broadcast::channel(ENCODER_WORKER_CHANNEL_SIZE);
        let config = WebRTCServerConfig::new(
            self.config.share_screen_config.listen_addr.clone(),
            self.config.share_screen_config.auth_token.clone(),
        )
        .with_enable_https(self.config.share_screen_config.enable_https)
        .with_cert_file(self.config.share_screen_config.cert_file.clone())
        .with_key_file(self.config.share_screen_config.key_file.clone());

        let session_config = WebRTCServerSessionConfig::default()
            .with_media_info(media_info)
            .with_host_ips(self.config.share_screen_config.host_ips.clone());

        let mut server = WebRTCServer::new(
            config,
            session_config,
            packet_sender,
            event_sender,
            exit_notify.clone(),
        );

        let stop_sig = self.stop_sig.clone();
        let error_sender = self.config.async_error_sender.clone();

        let rt_handle_clone = rt_handle.clone();
        std::thread::spawn(move || {
            rt_handle_clone.block_on(async move {
                match server.run().await {
                    Ok(_) => log::info!("WebRTCServer exit..."),
                    Err(e) => {
                        let err = format!("WebRTCServer run failed: {e}");
                        log::warn!("{err}");

                        if let Some(ref sender) = error_sender {
                            if let Err(e) = sender.try_send(err) {
                                log::warn!("async_error_sender try send failed: {e}");
                            }
                        }
                    }
                }
                stop_sig.store(true, Ordering::Relaxed);
            });
        });

        std::thread::spawn(move || {
            rt_handle.block_on(async move {
                loop {
                    tokio::select! {
                        event = event_receiver.recv() => {
                            match event {
                                Ok(Event::PeerConnected(addr)) => {
                                    let mut connections = SHARE_SCREEN_CONNECTIONS.lock().unwrap();
                                    connections.insert(addr);
                                    SHARE_SCREEN_CONNECTIONS_COUNT.store(connections.len() as u32, Ordering::Relaxed);
                                    log::info!("connections count: {}", connections.len());
                                }
                                Ok(Event::LocalClosed(addr)) => {
                                    log::info!("LocalClosed({addr})");

                                    let mut connections = SHARE_SCREEN_CONNECTIONS.lock().unwrap();
                                    connections.remove(&addr);
                                    SHARE_SCREEN_CONNECTIONS_COUNT.store(connections.len() as u32, Ordering::Relaxed);
                                    log::info!("connections count: {}", connections.len());
                                }
                                Ok(Event::PeerClosed(addr)) => {
                                    log::info!("PeerClosed({addr})");

                                    let mut connections = SHARE_SCREEN_CONNECTIONS.lock().unwrap();
                                    connections.remove(&addr);
                                    SHARE_SCREEN_CONNECTIONS_COUNT.store(connections.len() as u32, Ordering::Relaxed);
                                    log::info!("connections count: {}", connections.len());
                                }
                                Ok(Event::PeerConnecting(addr)) => log::info!("{addr} is connecting"),
                                Err(e) => log::warn!("event_receiver failed: {e}"),
                            }
                        }
                        _ = exit_notify.notified() => {
                            SHARE_SCREEN_CONNECTIONS.lock().unwrap().clear();
                            SHARE_SCREEN_CONNECTIONS_COUNT.store(0, Ordering::Relaxed);
                            log::info!("event_receiver receive `exit_notify`.");
                            break;
                        }
                    }
                }
            });
        });
    }

    fn start_push_stream(
        &mut self,
        mut video_encoder_header_data: Option<Vec<u8>>,
        mp4_h264_frame_sender: Option<Sender<VideoFrameType>>,
        mp4_mix_audio_sender: Option<Sender<Vec<f32>>>,
        mix_audio_receiver: Option<Receiver<Vec<f32>>>,
        mix_audio_channels: Option<u16>,
        mix_audio_sample_rate: Option<u32>,
    ) -> Result<Sender<VideoFrameType>, RecorderError> {
        use srtmp::{AacEncoderConfig, AudioData, RtmpClient, RtmpClientConfig, VideoData};

        let stop_sig = self.stop_sig.clone();
        let (h264_frame_sender, h264_frame_receiver) =
            bounded::<VideoFrameType>(ENCODER_WORKER_CHANNEL_SIZE);

        let (video_tx, video_rx) = bounded(ENCODER_WORKER_CHANNEL_SIZE / 2);
        let (audio_tx, audio_rx) = bounded(ENCODER_WORKER_CHANNEL_SIZE);

        let (encoder_width, encoder_height) = self.config.resolution.dimensions(
            self.config.screen_size.width as u32,
            self.config.screen_size.height as u32,
        );

        let config = RtmpClientConfig::new(
            self.config.push_stream_config.server_addr.clone(),
            self.config.push_stream_config.app.clone(),
            self.config.push_stream_config.stream_key.clone(),
        )
        .with_query_params(self.config.push_stream_config.query_params.clone())
        .with_video_width(encoder_width)
        .with_video_height(encoder_height)
        .with_framerate(self.config.fps.to_u32() as f64);

        let aac_config = if let Some(sample_rate) = mix_audio_sample_rate
            && let Some(channels) = mix_audio_channels
        {
            Some(
                AacEncoderConfig::default()
                    .with_sample_rate(sample_rate)
                    .with_channels(channels as u8),
            )
        } else {
            None
        };

        let error_sender = self.config.async_error_sender.clone();
        let mut client = RtmpClient::new(config, aac_config, video_rx, audio_rx, stop_sig.clone())?;
        let audio_input_frame_size = client.aac_encoder_input_frame_size();

        thread::spawn(move || match client.start() {
            Ok(_) => log::info!("Streaming completed successfully"),
            Err(e) => {
                let err = format!("Push Stream error: {e}");
                log::warn!("{err}");

                if let Some(sender) = error_sender {
                    if let Err(e) = sender.try_send(err) {
                        log::warn!("async_error_sender try send failed: {e}");
                    }
                }
            }
        });

        if let Some(headers_data) = video_encoder_header_data.take() {
            let packet = VideoData::new_with_sequence_header(headers_data)?;
            if let Err(e) = video_tx.send(packet) {
                return Err(RecorderError::Other(format!(
                    "send h264 sequence header failed: {e:?}"
                )));
            }
        }

        let handle = thread::spawn(move || {
            let mut no_data = true;
            let mut mix_audio_samples = vec![];
            let start_time = Instant::now();

            loop {
                if stop_sig.load(Ordering::Relaxed) {
                    if let Some(ref sender) = mp4_h264_frame_sender
                        && let Err(e) = sender.try_send(VideoFrameType::End)
                    {
                        log::warn!("mp4_h264_frame_sender try send `End` failed: {e}");
                    }

                    break;
                }

                if let Some(ref receiver) = mix_audio_receiver
                    && let Ok(data) = receiver.try_recv()
                {
                    if let Some(channels) = mix_audio_channels
                        && channels > 0
                    {
                        let mut sent_frame_count = 0;
                        let samples_per_frame = audio_input_frame_size * channels as usize;
                        mix_audio_samples.extend_from_slice(&data);

                        for frame in mix_audio_samples.chunks_exact(samples_per_frame) {
                            sent_frame_count += 1;

                            if let Err(e) = audio_tx.try_send(AudioData::new(
                                start_time.elapsed().as_millis() as u32,
                                frame.to_vec(),
                            )) {
                                log::warn!("try send audio data failed: {e}");
                            }
                        }

                        mix_audio_samples.drain(0..sent_frame_count * samples_per_frame);
                    }

                    if let Some(ref sender) = mp4_mix_audio_sender
                        && let Err(e) = sender.try_send(data)
                    {
                        log::warn!("try send audio data to mp4_worker failed: {e}");
                    }
                }

                if let Some(ref c) = mix_audio_receiver {
                    no_data = c.is_empty();
                }

                if let Ok(data) = h264_frame_receiver.try_recv() {
                    no_data = false;

                    if let Some(ref sender) = mp4_h264_frame_sender {
                        let converted_data = match data {
                            VideoFrameType::Frame(ref content) => {
                                VideoFrameType::Frame(convert_annexb_to_length_prefixes(&content))
                            }
                            VideoFrameType::End => VideoFrameType::End,
                        };

                        if let Err(e) = sender.try_send(converted_data) {
                            log::warn!("try send h264 frame to mp4_worker failed: {e}");
                        }
                    }

                    if let VideoFrameType::Frame(data) = data
                        && let Err(e) = video_tx.try_send(VideoData::new(
                            start_time.elapsed().as_millis() as u32,
                            data,
                        ))
                    {
                        log::warn!("push stream try send h264 nal data failed: {e}");
                    };
                }

                if no_data {
                    no_data = h264_frame_receiver.is_empty();
                }

                log::trace!(
                    "h264_frame_receiver: {}, mix_audio_receiver: {:?}",
                    h264_frame_receiver.len(),
                    if let Some(ref c) = mix_audio_receiver {
                        Some(c.len())
                    } else {
                        None
                    }
                );

                if no_data {
                    thread::sleep(Duration::from_millis(10));
                }
            }

            log::info!("push_stream_worker exit...");
        });
        self.push_stream_worker = Some(handle);

        Ok(h264_frame_sender)
    }
}
