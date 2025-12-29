use crate::aac_encoder::{AacEncoder, AacEncoderConfig};
use bytes::Bytes;
use crossbeam::channel::Receiver;
use derivative::Derivative;
use derive_setters::Setters;
use rml_rtmp::{
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    rml_amf0::Amf0Value,
    sessions::{
        ClientSession, ClientSessionConfig, ClientSessionError, ClientSessionEvent,
        ClientSessionResult, StreamMetadata,
    },
    time::RtmpTimestamp,
};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Clone, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct RtmpClientConfig {
    /// RTMP server URL (e.g., "rtmp://localhost:1935")
    #[setters(skip)]
    pub rtmp_url: String,

    /// app name (e.g., "live")
    #[setters(skip)]
    pub app: String,

    /// Stream key (e.g., "stream_key")
    #[setters(skip)]
    pub stream_key: String,

    #[derivative(Default(value = "1935"))]
    pub port: u16,

    /// Audio sample rate (e.g., 44100, 48000)
    #[derivative(Default(value = "44100"))]
    pub audio_sample_rate: u32,

    /// Audio channels (1 = mono, 2 = stereo)
    #[derivative(Default(value = "2"))]
    pub audio_channels: u8,

    /// Video width in pixels
    #[derivative(Default(value = "1920"))]
    pub video_width: u32,

    /// Video height in pixels
    #[derivative(Default(value = "1080"))]
    pub video_height: u32,

    /// Video framerate (fps)
    #[derivative(Default(value = "25.0"))]
    pub framerate: f64,

    /// Maximum allowed frame backlog before dropping frames
    /// If channel backlog exceeds this value, old frames will be dropped to maintain low latency
    #[derivative(Default(value = "5"))]
    pub max_frame_backlog: usize,

    // 100M
    #[derivative(Default(value = "104857600"))]
    pub max_write_buffer: usize,
}

impl RtmpClientConfig {
    pub fn new(rtmp_url: String, app: String, stream_key: String) -> Self {
        Self {
            rtmp_url,
            app,
            stream_key,
            ..Default::default()
        }
    }

    pub fn extract_host(&self) -> Result<String, RtmpClientError> {
        let url = self.rtmp_url.trim_start_matches("rtmp://");
        let parts: Vec<&str> = url.split('/').collect();
        if parts.is_empty() {
            if url.trim().is_empty() {
                return Err(RtmpClientError::InvalidUrl(self.rtmp_url.clone()));
            } else {
                return Ok(url.trim().to_string());
            }
        }
        Ok(parts[0].to_string())
    }
}

#[derive(Debug, Clone)]
pub struct VideoData {
    /// Timestamp in milliseconds
    pub timestamp: u32,

    /// Video data (H264 NALU)
    pub data: Vec<u8>,

    /// Whether this frame is a keyframe (I-frame/IDR)
    /// Important for frame dropping: must drop entire GOP until next keyframe
    pub is_keyframe: bool,

    /// Mark if this is AVCDecoderConfigurationRecord (Sequence Header)
    /// Contains SPS/PPS information needed to initialize decoder
    pub is_sequence_header: bool,
}

impl VideoData {
    pub fn new(timestamp: u32, data: Vec<u8>) -> Self {
        let is_keyframe = mp4m::Mp4Processor::is_keyframe_annexb(&data);

        Self {
            timestamp,
            data,
            is_keyframe,
            is_sequence_header: false,
        }
    }

    // Convert annexb format to AVCDecoderConfigurationRecord for RTMP/FLV
    pub fn new_with_sequence_header(data: Vec<u8>) -> Result<Self, RtmpClientError> {
        let data = match annexb_to_avc_decoder_config(&data) {
            Ok(avc_config) => {
                log::info!(
                    "Successfully converted to AVC decoder config: {} bytes, first 20: {:02x?}",
                    avc_config.len(),
                    &avc_config[..avc_config.len().min(20)]
                );

                avc_config
            }
            Err(e) => {
                log::error!("Failed to convert annexb to AVC decoder config: {:?}", e);
                return Err(e.into());
            }
        };

        Ok(Self {
            timestamp: 0,
            data,
            is_keyframe: true,
            is_sequence_header: true,
        })
    }

    // Add FLV video tag prefix
    // Format: [FrameType(4) | CodecID(4)] [PacketType] [CompositionTime(3 bytes)] [Data]
    // FrameType: 1 = keyframe, 2 = inter frame
    // CodecID: 7 = AVC/H.264
    // PacketType: 0x00 = sequence header, 0x01 = AVC NALU
    // CompositionTime: 0x00 0x00 0x00 for now
    pub fn tagged_video(&self) -> Vec<u8> {
        let mut tagged_video = if self.is_sequence_header {
            // Sequence header: keyframe + AVC + sequence header
            vec![0x17, 0x00, 0x00, 0x00, 0x00]
        } else if self.is_keyframe {
            // Keyframe: keyframe + AVC + NALU
            vec![0x17, 0x01, 0x00, 0x00, 0x00]
        } else {
            // Inter frame: inter frame + AVC + NALU
            vec![0x27, 0x01, 0x00, 0x00, 0x00]
        };

        tagged_video.extend_from_slice(&self.data);

        tagged_video
    }
}

#[derive(Debug, Clone)]
pub struct AudioData {
    /// Timestamp in milliseconds
    pub timestamp: u32,

    /// PCM audio data (f32 samples in range [-1.0, 1.0])
    pub data: Vec<f32>,
}

impl AudioData {
    pub fn new(timestamp: u32, data: Vec<f32>) -> Self {
        Self { timestamp, data }
    }

    /// Get FLV audio tag sound format byte based on sample rate and channels
    /// FLV audio tag format: [SoundFormat(4) | SoundRate(2) | SoundSize(1) | SoundType(1)]
    /// - SoundFormat: 10 = AAC
    /// - SoundRate: 00=5.5kHz, 01=11kHz, 10=22kHz, 11=44kHz
    /// - SoundSize: 0=8-bit, 1=16-bit (always 1 for AAC)
    /// - SoundType: 0=Mono, 1=Stereo
    fn get_sound_format_byte(sample_rate: u32, channels: u8) -> u8 {
        // SoundFormat: AAC = 10 (binary) = 0xA0
        let sound_format = 0xA0;

        // SoundRate mapping (FLV only has 4 rate categories):
        // 0 -> 5.5kHz: 8000
        // 1 -> 11kHz: 11025, 12000
        // 2 -> 22kHz: 16000, 22050, 24000
        // 3 -> 44kHz: 32000, 44100, 48000, 64000, 88200, 96000
        let rate_bits = match sample_rate {
            8000 => 0b00,
            11025 | 12000 => 0b01,
            16000 | 22050 | 24000 => 0b10,
            32000 | 44100 | 48000 | 64000 | 88200 | 96000 => 0b11,
            _ => 0b11, // Default to 44kHz for unknown rates
        };

        // SoundSize: always 16-bit for AAC = 1 << 1 = 0x02
        let sound_size = 0b10;

        // SoundType: 0 = Mono, 1 = Stereo (channels - 1)
        let sound_type = if channels == 0 { 0 } else { 1 };

        sound_format | (rate_bits << 2) | sound_size | sound_type
    }

    // Send AAC AudioSpecificConfig as sequence header first
    // FLV audio tag format: [SoundType(1) | SoundSize(1) | SoundRate(2) | SoundFormat(4)]
    // Followed by packet type: 0x00 = sequence header, 0x01 = raw data
    pub fn tagged_aac_sequence_header(
        header_data: &[u8],
        sample_rate: u32,
        channels: u8,
    ) -> Vec<u8> {
        let sound_byte = Self::get_sound_format_byte(sample_rate, channels);
        let mut tagged_header = vec![sound_byte, 0x00];
        tagged_header.extend_from_slice(header_data);
        tagged_header
    }

    // Add FLV audio tag prefix: [sound_byte] 0x01 [AAC data]
    // sound_byte encodes sample rate, channels, codec (AAC), and bit depth (16-bit)
    // 0x01 = AAC raw data (not sequence header)
    pub fn tagged_aac_data(aac_data: &[u8], sample_rate: u32, channels: u8) -> Vec<u8> {
        let sound_byte = Self::get_sound_format_byte(sample_rate, channels);
        let mut tagged_audio = vec![sound_byte, 0x01];
        tagged_audio.extend_from_slice(aac_data);
        tagged_audio
    }
}

#[derive(Error, Debug)]
pub enum RtmpClientError {
    #[error("Invalid RTMP URL: {0}")]
    InvalidUrl(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Handshake error: {0}")]
    HandshakeError(String),

    #[error("Publish error: {0}")]
    PublishError(String),

    #[error("Send error: {0}")]
    SendError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("AAC encoder error: {0}")]
    AacEncoderError(String),

    #[error("RTMP protocol error: {0}")]
    ProtocolError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Client session error: {0}")]
    ClientSessionError(#[from] ClientSessionError),

    #[error("RTMP handshake error: {0}")]
    RtmpHandshakeError(#[from] rml_rtmp::handshake::HandshakeError),

    #[error("Annexb to AVC decoder config error: {0}")]
    AnnexbToAvc(String),
}

pub struct RtmpClient {
    config: RtmpClientConfig,
    video_receiver: Receiver<VideoData>,
    audio_receiver: Receiver<AudioData>,
    exit_sig: Arc<AtomicBool>,
    aac_encoder: AacEncoder,
    write_buffer: Vec<u8>,
}

impl RtmpClient {
    pub fn new(
        config: RtmpClientConfig,
        aac_encoder_config: AacEncoderConfig,
        video_receiver: Receiver<VideoData>,
        audio_receiver: Receiver<AudioData>,
        exit_sig: Arc<AtomicBool>,
    ) -> Result<Self, RtmpClientError> {
        let aac_encoder = AacEncoder::new(aac_encoder_config)
            .map_err(|e| RtmpClientError::AacEncoderError(e.to_string()))?;

        Ok(Self {
            config,
            video_receiver,
            audio_receiver,
            exit_sig,
            aac_encoder,
            write_buffer: Vec::new(),
        })
    }

    pub fn start(&mut self) -> Result<(), RtmpClientError> {
        log::info!("Starting RTMP client");

        // Build RTMP URL and extract host
        let host = self.config.extract_host()?;
        let address = if host.contains(':') {
            host.to_string()
        } else {
            format!("{}:{}", host, self.config.port)
        };

        let app_name = self.config.app.clone();
        let stream_name = self.config.stream_key.clone();

        log::info!("RTMP Target: {}", address);
        log::info!("App: {}, Stream: {}", app_name, stream_name);

        let mut stream = TcpStream::connect(&address).map_err(|e| {
            RtmpClientError::ConnectionError(format!("Failed to connect to {}: {}", address, e))
        })?;
        log::info!("TCP connection established to {}", address);

        self.perform_handshake(&mut stream)?;
        log::info!("RTMP handshake completed");

        stream
            .set_nonblocking(true)
            .map_err(RtmpClientError::IoError)?;

        let config = ClientSessionConfig::new();
        let (mut client_session, initial_results) =
            ClientSession::new(config).map_err(|e| RtmpClientError::ClientSessionError(e))?;

        self.cache_session_results(&initial_results)?;
        self.try_flush_buffer(&mut stream)?;

        self.establish_rtmp_session(&mut stream, &mut client_session, &app_name, &stream_name)?;
        log::info!("RTMP session established and publishing started");

        self.forward_data(&mut stream, &mut client_session)?;

        log::info!("Streaming completed successfully");
        Ok(())
    }

    fn perform_handshake(&self, stream: &mut TcpStream) -> Result<(), RtmpClientError> {
        let mut handshake = Handshake::new(PeerType::Client);
        let c0c1 = handshake.generate_outbound_p0_and_p1()?;

        stream
            .write_all(&c0c1)
            .map_err(|e| RtmpClientError::IoError(e))?;
        stream.flush().map_err(|e| RtmpClientError::IoError(e))?;
        log::debug!("Sent C0+C1 handshake data ({} bytes)", c0c1.len());

        // Set read timeout to prevent hanging on non-responsive servers
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| RtmpClientError::IoError(e))?;

        let mut buffer = [0u8; 4096];
        loop {
            // Read some data (may be partial)
            let n = match stream.read(&mut buffer) {
                Ok(0) => {
                    return Err(RtmpClientError::HandshakeError(
                        "Server closed connection during handshake".to_string(),
                    ));
                }
                Ok(n) => n,
                Err(e) => return Err(RtmpClientError::IoError(e)),
            };

            let result = handshake
                .process_bytes(&buffer[..n])
                .map_err(|e| RtmpClientError::HandshakeError(e.to_string()))?;

            match result {
                HandshakeProcessResult::InProgress { response_bytes } => {
                    if !response_bytes.is_empty() {
                        stream
                            .write_all(&response_bytes)
                            .map_err(|e| RtmpClientError::IoError(e))?;
                        stream.flush().map_err(|e| RtmpClientError::IoError(e))?;

                        log::debug!("Sent handshake response ({} bytes)", response_bytes.len());
                    }
                }
                HandshakeProcessResult::Completed {
                    response_bytes,
                    remaining_bytes,
                } => {
                    if !response_bytes.is_empty() {
                        stream
                            .write_all(&response_bytes)
                            .map_err(|e| RtmpClientError::IoError(e))?;
                        stream.flush().map_err(|e| RtmpClientError::IoError(e))?;

                        log::debug!(
                            "Sent final handshake response ({} bytes)",
                            response_bytes.len()
                        );
                    }

                    // Handle remaining bytes (should be rare)
                    if !remaining_bytes.is_empty() {
                        log::warn!(
                            "Handshake completed with {} remaining bytes (data after handshake)",
                            remaining_bytes.len()
                        );
                    }

                    log::info!("RTMP handshake completed successfully");
                    break;
                }
            }
        }

        stream
            .set_read_timeout(None)
            .map_err(|e| RtmpClientError::IoError(e))?;

        Ok(())
    }

    fn try_flush_buffer(&mut self, stream: &mut TcpStream) -> Result<bool, RtmpClientError> {
        if self.write_buffer.is_empty() {
            return Ok(true);
        }

        let mut total_written = 0;
        let mut would_block = false;

        while total_written < self.write_buffer.len() {
            match stream.write(&self.write_buffer[total_written..]) {
                Ok(0) => {
                    return Err(RtmpClientError::ConnectionError(
                        "Write returned 0 bytes, connection closed".to_string(),
                    ));
                }
                Ok(n) => total_written += n,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    would_block = true;
                    break; // Stop writing, keep remaining data in buffer
                }
                Err(e) => return Err(RtmpClientError::IoError(e)),
            }
        }

        if total_written > 0 {
            self.write_buffer.drain(0..total_written);
        }

        Ok(!would_block)
    }

    fn establish_rtmp_session(
        &mut self,
        stream: &mut TcpStream,
        client_session: &mut ClientSession,
        app_name: &str,
        stream_name: &str,
    ) -> Result<(), RtmpClientError> {
        log::info!("Establishing RTMP session...");

        log::info!("Phase 1: Requesting connection to app: {}", app_name);
        let connect_result = client_session.request_connection(app_name.to_string())?;
        self.cache_session_results(&[connect_result])?;

        loop {
            match self.try_flush_buffer(stream)? {
                true => break, // Buffer fully flushed
                false => std::thread::sleep(Duration::from_millis(1)),
            }
        }

        log::info!("Processing connection response...");
        let (connection_accepted, _) = self.process_establish_responses(stream, client_session)?;
        if !connection_accepted {
            return Err(RtmpClientError::ConnectionError(
                "Connection request was not accepted by server".to_string(),
            ));
        }
        log::info!("✓ Connection confirmed accepted");

        log::info!("Phase 2: Requesting publish to stream: {}", stream_name);
        let publish_result = client_session.request_publishing(
            stream_name.to_string(),
            rml_rtmp::sessions::PublishRequestType::Live,
        )?;

        self.cache_session_results(&[publish_result])?;
        loop {
            match self.try_flush_buffer(stream)? {
                true => break, // Buffer fully flushed
                false => std::thread::sleep(Duration::from_millis(1)),
            }
        }

        log::info!("Processing publish response...");
        let (_, publish_accepted) = self.process_establish_responses(stream, client_session)?;
        if !publish_accepted {
            return Err(RtmpClientError::PublishError(
                "Publish request was not accepted by server".to_string(),
            ));
        }
        log::info!("✓ Publish confirmed accepted");

        log::info!("Phase 3: Sending metadata");
        let mut metadata = StreamMetadata::new();
        let mut metadata_props = HashMap::new();
        metadata_props.insert(
            "width".to_string(),
            Amf0Value::Number(self.config.video_width as f64),
        );
        metadata_props.insert(
            "height".to_string(),
            Amf0Value::Number(self.config.video_height as f64),
        );
        metadata_props.insert(
            "framerate".to_string(),
            Amf0Value::Number(self.config.framerate),
        );
        metadata_props.insert("videocodecid".to_string(), Amf0Value::Number(7.0)); // H.264
        metadata_props.insert("audiocodecid".to_string(), Amf0Value::Number(10.0)); // AAC
        metadata.apply_metadata_values(metadata_props);

        let metadata_result = client_session.publish_metadata(&metadata)?;
        self.cache_session_results(&[metadata_result])?;
        loop {
            match self.try_flush_buffer(stream)? {
                true => break, // Buffer fully flushed
                false => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        log::info!("✓ Metadata sent");

        Ok(())
    }

    fn cache_session_results(
        &mut self,
        results: &[ClientSessionResult],
    ) -> Result<(), RtmpClientError> {
        for (i, result) in results.iter().enumerate() {
            match result {
                ClientSessionResult::OutboundResponse(packet) => {
                    self.write_buffer.extend_from_slice(&packet.bytes);
                }
                ClientSessionResult::RaisedEvent(event) => {
                    log::debug!("Received event[{}]: {:?}", i, event);
                }
                ClientSessionResult::UnhandleableMessageReceived(payload) => {
                    log::warn!("Received unhandleable message[{}]: {:?}", i, payload);
                }
            }
        }

        Ok(())
    }

    fn process_establish_responses(
        &mut self,
        stream: &mut TcpStream,
        client_session: &mut ClientSession,
    ) -> Result<(bool, bool), RtmpClientError> {
        let mut buffer = [0u8; 4096];
        let mut total_bytes = 0usize;
        let mut total_responses = 0usize;
        let mut connection_accepted = false;
        let mut publish_accepted = false;
        let mut wouldblock_count = 0usize;

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    log::warn!("Server closed connection (read returned 0)");
                    return Err(RtmpClientError::ConnectionError(
                        "Server closed connection".to_string(),
                    ));
                }
                Ok(bytes_read) => {
                    wouldblock_count = 0; // Reset counter on successful read
                    total_bytes += bytes_read;
                    let results = client_session.handle_input(&buffer[..bytes_read])?;
                    total_responses += results.len();

                    for result in &results {
                        match result {
                            ClientSessionResult::RaisedEvent(event) => match event {
                                ClientSessionEvent::ConnectionRequestAccepted => {
                                    log::info!("✓ Connection request accepted by server");
                                    connection_accepted = true;
                                }
                                ClientSessionEvent::PublishRequestAccepted => {
                                    log::info!("✓ Publish request accepted by server");
                                    publish_accepted = true;
                                }
                                ClientSessionEvent::ConnectionRequestRejected { description } => {
                                    log::error!("✗ Connection rejected: {}", description);
                                    return Err(RtmpClientError::ConnectionError(format!(
                                        "Connection rejected: {description}"
                                    )));
                                }
                                _ => {
                                    log::debug!("Received event: {:?}", event);
                                }
                            },
                            ClientSessionResult::OutboundResponse(_) => {
                                log::trace!("Outbound response to send");
                            }
                            _ => {}
                        }
                    }

                    self.cache_session_results(&results)?;
                    loop {
                        match self.try_flush_buffer(stream)? {
                            true => break, // Buffer fully flushed
                            false => std::thread::sleep(Duration::from_millis(1)),
                        }
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    wouldblock_count += 1;
                    if wouldblock_count >= 10 {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    log::error!("Read error: {:?}", e);
                    return Err(RtmpClientError::IoError(e));
                }
            }
        }

        log::debug!(
            "Finished processing responses: {} bytes, {} results",
            total_bytes,
            total_responses
        );
        Ok((connection_accepted, publish_accepted))
    }

    fn forward_data(
        &mut self,
        stream: &mut TcpStream,
        client_session: &mut ClientSession,
    ) -> Result<(), RtmpClientError> {
        let mut video_packet_count = 0u64;
        let mut dropped_video_packet = 0u64;
        let mut total_video_packet_count = 0u64;
        let mut total_audio_packet_count = 0u64;

        let max_backlog = self.config.max_frame_backlog;
        let max_write_buffer = self.config.max_write_buffer.max(10 * 1024 * 1024);

        log::info!("Sending AAC AudioSpecificConfig as sequence header");
        let audio_config = self.aac_encoder.audio_specific_config();
        let aac_sequence_header = AudioData::tagged_aac_sequence_header(
            &audio_config,
            self.aac_encoder.sample_rate(),
            self.aac_encoder.channels(),
        );

        log::info!("AudioSpecificConfig: {:02x?}", audio_config);
        log::info!(
            "AAC sequence header with FLV tag: {:02x?}",
            aac_sequence_header
        );

        let result = client_session.publish_audio_data(
            Bytes::from(aac_sequence_header),
            RtmpTimestamp::new(0),
            true,
        )?;
        self.cache_session_results(&[result])?;

        loop {
            if self.exit_sig.load(std::sync::atomic::Ordering::Relaxed) {
                log::info!("Exit signal received, stopping data forwarding");
                break;
            }

            self.try_flush_buffer(stream)?;

            // Read network data (handle ACKs, let TCP window slide to enable more writes)
            self.process_incoming_data(stream, client_session)?;

            if self.write_buffer.len() > max_write_buffer {
                log::warn!(
                    "Network congestion: write buffer full ({} bytes), pausing encoding input",
                    self.write_buffer.len()
                );
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }

            crossbeam::channel::select! {
                recv(self.video_receiver) -> msg => {
                    match msg {
                        Ok(mut video_data) => {
                            let backlog = self.video_receiver.len();

                            if backlog > max_backlog && !video_data.is_keyframe {
                                let mut dropped_before_keyframe = 0;

                                // drop half queued frames
                                while self.video_receiver.len() > backlog / 2 {
                                    match self.video_receiver.try_recv() {
                                        Ok(frame) => {
                                            if frame.is_keyframe {
                                                video_data = frame;
                                                break;
                                            }
                                            dropped_before_keyframe += 1;
                                        }
                                        Err(_) => break,
                                    }
                                }

                                dropped_video_packet += dropped_before_keyframe;

                                log::info!("Dropped {} frames (backlog: {}, now at keyframe)",
                                    dropped_before_keyframe, (backlog as u64).max(dropped_before_keyframe));
                            }

                            let tagged_video = video_data.tagged_video();
                            let result = client_session.publish_video_data(
                                Bytes::from(tagged_video),
                                RtmpTimestamp::new(video_data.timestamp),
                                false
                            )?;
                            self.cache_session_results(&[result])?;

                            video_packet_count += 1;
                            total_video_packet_count += 1;
                        }
                        Err(_) => {
                            log::warn!("Video channel disconnected");
                            break;
                        }
                    }
                }
                recv(self.audio_receiver) -> msg => {
                    match msg {
                        Ok(audio_data) => {
                            match self.aac_encoder.encode(&audio_data.data) {
                                Ok(aac_data) => {
                                    // Log first AAC frame for debugging
                                    if total_audio_packet_count == 0 {
                                        log::info!("First AAC encoded data: {} bytes, first 20: {:02x?}",
                                            aac_data.len(), &aac_data[..aac_data.len().min(20)]);
                                    }

                                    let tagged_audio = AudioData::tagged_aac_data(
                                        &aac_data,
                                        self.aac_encoder.sample_rate(),
                                        self.aac_encoder.channels()
                                    );

                                    if total_audio_packet_count < 5 {
                                        log::info!("Audio packet {} with FLV tag: {} bytes, first 20: {:02x?}",
                                            total_audio_packet_count, tagged_audio.len(),
                                            &tagged_audio[..tagged_audio.len().min(20)]);
                                    }

                                    let result = client_session.publish_audio_data(
                                        Bytes::from(tagged_audio),
                                        RtmpTimestamp::new(audio_data.timestamp),
                                        false
                                    )?;
                                    self.cache_session_results(&[result])?;
                                    total_audio_packet_count += 1;
                                }
                                Err(e) => log::error!("AAC encoding error: {e}"),
                            }
                        }
                        Err(_) => {
                            log::warn!("Audio channel disconnected");
                            break;
                        }
                    }
                }
                default(Duration::from_millis(1)) => (),
            }

            if video_packet_count > 100 {
                log::debug!(
                    "Data forwarding: processed {} packets, dropped {} frames, buffer: {} bytes",
                    video_packet_count,
                    dropped_video_packet,
                    self.write_buffer.len()
                );
                video_packet_count = 0;
            }
        }

        log::info!(
            "Data forwarding loop completed (total: {} video packets, dropped: {} video packets, total: {} audio packets)",
            total_video_packet_count,
            dropped_video_packet,
            total_audio_packet_count
        );
        Ok(())
    }

    fn process_incoming_data(
        &mut self,
        stream: &mut TcpStream,
        client_session: &mut ClientSession,
    ) -> Result<(), RtmpClientError> {
        let mut buffer = [0u8; 4096];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    return Err(RtmpClientError::ConnectionError(
                        "Server closed connection".to_string(),
                    ));
                }
                Ok(bytes_read) => match client_session.handle_input(&buffer[..bytes_read]) {
                    Ok(results) => {
                        self.cache_session_results(&results)?;
                        self.try_flush_buffer(stream)?;
                    }
                    Err(e) => log::warn!("Protocol error from server (continuing): {}", e),
                },
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => {
                    return Err(RtmpClientError::IoError(e));
                }
            }
        }

        self.try_flush_buffer(stream)?;

        Ok(())
    }

    pub fn stop(&mut self) {
        log::info!("Stopping RTMP client");
        self.exit_sig
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for RtmpClient {
    fn drop(&mut self) {
        self.exit_sig
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Convert H.264 annexb format to AVCDecoderConfigurationRecord
// Find SPS (NAL type 7) and PPS (NAL type 8) in annexb data
// Annexb format: [start code 0x00000001] [NAL header] [data] [start code] ...
pub fn annexb_to_avc_decoder_config(annexb_data: &[u8]) -> Result<Vec<u8>, RtmpClientError> {
    let mut sps_data: Option<Vec<u8>> = None;
    let mut pps_data: Option<Vec<u8>> = None;

    let mut i = 0;
    while i < annexb_data.len() {
        // Look for start code (0x00000001)
        if i + 4 <= annexb_data.len() && annexb_data[i..i + 4] == [0, 0, 0, 1] {
            // Move past start code
            i += 4;

            if i >= annexb_data.len() {
                break;
            }

            // Get NAL unit type (lower 5 bits of first byte after start code)
            let nal_type = annexb_data[i] & 0x1F;

            // Find next start code to get NAL unit length
            let nal_start = i;
            let mut nal_end = i + 1;
            while nal_end + 4 <= annexb_data.len() {
                if annexb_data[nal_end..nal_end + 4] == [0, 0, 0, 1] {
                    break;
                }
                nal_end += 1;
            }

            let nal_data = &annexb_data[nal_start..nal_end];

            match nal_type {
                7 => sps_data = Some(nal_data.to_vec()),
                8 => pps_data = Some(nal_data.to_vec()),
                _ => {}
            }

            i = nal_end;
        } else {
            i += 1;
        }
    }

    let sps = sps_data.ok_or(RtmpClientError::AnnexbToAvc(
        "SPS not found in H.264 headers".to_string(),
    ))?;
    let pps = pps_data.ok_or(RtmpClientError::AnnexbToAvc(
        "PPS not found in H.264 headers".to_string(),
    ))?;

    log::info!("Found SPS: {} bytes, PPS: {} bytes", sps.len(), pps.len());

    // Build AVCDecoderConfigurationRecord
    let mut avc_config = Vec::new();
    avc_config.push(0x01); // Configuration version

    if sps.len() >= 4 {
        avc_config.extend_from_slice(&sps[1..4]); // profile, profile_compat, level
    } else {
        return Err(RtmpClientError::AnnexbToAvc("SPS too short".to_string()));
    }

    avc_config.push(0xFF); // Length size minus one (with reserved bits = 11111111)
    avc_config.push(0xE1); // Number of SPS (with reserved bits = 11100001)

    // SPS length and data
    avc_config.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    avc_config.extend_from_slice(&sps);
    avc_config.push(0x01); // Number of PPS (with reserved bits = 00000001)

    // PPS length and data
    avc_config.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    avc_config.extend_from_slice(&pps);

    Ok(avc_config)
}
