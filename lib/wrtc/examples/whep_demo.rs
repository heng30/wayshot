//! WHEP Demo Server
//!
//! This example demonstrates how to create a WebRTC server that can stream H264 video
//! files to WHEP clients. The server reads a local H264 file and publishes it
//! as a WebRTC stream that clients can consume using the WHEP protocol.
//!
//! Usage:
//! ```bash
//! # Use default H264 file
//! cargo run --example whep_demo
//!
//! # Specify custom H264 file and FPS
//! cargo run --example whep_demo -- ./data/test.h264 30
//! ```
//!
//! Then open http://127.0.0.1:8080 in your browser to test with the WebRTC client.

use bytes::BytesMut;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use streamhub::{
    define::{DataSender, InformationSender, PacketData, PublishType, PublisherInfo, StreamHubEvent,
        StreamHubEventSender, SubscribeType, TStreamHandler,
    },
    errors::{StreamHubError, StreamHubErrorValue},
    statistics::StatisticsStream,
    stream::StreamIdentifier,
    utils::{RandomDigitCount, Uuid},
};
use tokio::sync::mpsc;
use tokio::time::sleep;
use wrtc::webrtc::WebRTCServer;

/// H264 file reader that reads NAL units and streams them as video packets
pub struct H264FileReader {
    file_path: String,
    frame_duration: Duration,
    fps: u32,
}

impl H264FileReader {
    pub fn new(file_path: String, fps: u32) -> Self {
        Self {
            file_path,
            frame_duration: Duration::from_secs_f32(1.0 / fps as f32),
            fps,
        }
    }

    /// Read H264 file and stream NAL units in real-time
    pub async fn stream_file(
        &self,
        packet_sender: mpsc::UnboundedSender<PacketData>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new(&self.file_path).exists() {
            return Err(format!("H264 file not found: {}", self.file_path).into());
        }

        let mut file = File::open(&self.file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        log::info!(
            "Streaming H264 file: {} (size: {} bytes, {} FPS)",
            self.file_path,
            buffer.len(),
            self.fps
        );

        // Parse H264 NAL units
        let nal_units = H264FileReader::parse_nal_units(&buffer);
        log::info!("Found {} NAL units in H264 file", nal_units.len());

        if nal_units.is_empty() {
            return Err("No valid NAL units found in H264 file".into());
        }

        // Extract SPS and PPS NAL units for codec initialization
        let mut sps_data = Vec::new();
        let mut pps_data = Vec::new();

        for (i, nal_unit) in nal_units.iter().enumerate() {
            if nal_unit.len() >= 2 {
                let nal_header = nal_unit[0] & 0x1F; // NAL unit header (first 5 bits)
                let nal_type = nal_header & 0x1F; // NAL unit type (last 5 bits)

                match nal_type {
                    7 => { // SPS (Sequence Parameter Set)
                        log::info!("Found SPS NAL unit at index {} ({} bytes)", i, nal_unit.len());
                        sps_data = nal_unit.clone();
                    }
                    8 => { // PPS (Picture Parameter Set)
                        log::info!("Found PPS NAL unit at index {} ({} bytes)", i, nal_unit.len());
                        pps_data = nal_unit.clone();
                    }
                    5 | 1 => { // IDR frame or slice data - we have video data
                        log::debug!("Found video NAL unit type {} at index {} ({} bytes)", nal_type, i, nal_unit.len());
                        break; // Stop after finding first video data
                    }
                    _ => {
                        log::debug!("Found other NAL unit type {} at index {}", nal_type, i);
                    }
                }
            }
        }

        if !sps_data.is_empty() && !pps_data.is_empty() {
            log::info!("Extracted SPS ({} bytes) and PPS ({} bytes) for codec initialization",
                       sps_data.len(), pps_data.len());
        } else {
            log::warn!("Could not extract both SPS and PPS from H264 file");
        }

        let start_time = Instant::now();
        let mut frame_count = 0;

        // Stream NAL units with proper timing
        for (i, nal_unit) in nal_units.iter().enumerate() {
            // Calculate when this frame should be sent
            let target_time = start_time + self.frame_duration * (i as u32);
            let now = Instant::now();

            if target_time > now {
                sleep(target_time - now).await;
            }

            // Create RTP timestamp (90kHz clock rate for H264)
            let rtp_timestamp = frame_count * 90000 / self.fps;

            // Send NAL unit as video packet
            let packet_data = PacketData::Video {
                timestamp: rtp_timestamp,
                data: BytesMut::from(&nal_unit[..]),
            };

            if let Err(err) = packet_sender.send(packet_data) {
                log::error!("Failed to send video packet: {}", err);
                break;
            }

            frame_count += 1;

            // Log progress every 30 frames
            if frame_count % 30 == 0 {
                log::info!(
                    "Streamed {} frames ({}%)",
                    frame_count,
                    (frame_count * 100) / nal_units.len() as u32
                );
            }
        }

        log::info!(
            "Finished streaming H264 file. Total frames: {}",
            frame_count
        );
        Ok(())
    }

    /// Create a simple test H264 file with basic NAL units for demonstration
    #[allow(dead_code)]
    fn create_test_h264_file(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Creating test H264 file at: {}", file_path);

        // Create a minimal H264 test pattern with SPS, PPS, and IDR frame NAL units
        // These are simplified NAL units for demonstration purposes
        let test_data = vec![
            // SPS (Sequence Parameter Set) -NAL unit type 7
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1E, 0x9A, 0x74, 0x05, 0x81, 0xEC, 0x80,
            // PPS (Picture Parameter Set) - NAL unit type 8
            0x00, 0x00, 0x00, 0x01, 0x68, 0xCE, 0x3C, 0x80,
            // IDR Frame start - NAL unit type 5
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00, 0x28, 0xFF, 0xFF, 0xFF,
            // Additional slice data - NAL unit type 1
            0x00, 0x00, 0x00, 0x01, 0x41, 0x9A, 0x74, 0x05, 0x81, 0xEC, 0x80,
            // Repeat some frames for demonstration
            0x00, 0x00, 0x00, 0x01, 0x41, 0x9A, 0x74, 0x05, 0x81, 0xEC, 0x81, 0x00, 0x00, 0x00,
            0x01, 0x41, 0x9A, 0x74, 0x05, 0x81, 0xEC, 0x82,
        ];

        let mut file = File::create(file_path)?;
        file.write_all(&test_data)?;

        log::info!("Test H264 file created successfully at: {}", file_path);
        Ok(())
    }

    /// Parse H264 NAL units from raw H264 stream
    fn parse_nal_units(buffer: &[u8]) -> Vec<Vec<u8>> {
        let mut nal_units = Vec::new();
        let mut start = None;
        let mut i = 0;

        while i < buffer.len() {
            // Look for NAL unit start codes
            if i + 3 < buffer.len() && buffer[i] == 0x00 && buffer[i + 1] == 0x00 {
                let start_code_len = if buffer[i + 2] == 0x01 {
                    3
                } else if i + 4 < buffer.len() && buffer[i + 2] == 0x00 && buffer[i + 3] == 0x01 {
                    4
                } else {
                    i += 1;
                    continue;
                };

                if let Some(prev_start) = start {
                    // Found a new NAL unit, save the previous one
                    nal_units.push(buffer[prev_start..i].to_vec());
                }

                start = Some(i + start_code_len);
                i += start_code_len;
            } else {
                i += 1;
            }
        }

        // Add the last NAL unit
        if let Some(prev_start) = start {
            nal_units.push(buffer[prev_start..].to_vec());
        }

        nal_units
    }
}

/// Stream handler for H264 file streaming
pub struct H264StreamHandler {
    packet_sender: Arc<Mutex<Option<mpsc::UnboundedSender<PacketData>>>>,
    sps: Arc<Mutex<Vec<u8>>>,
    pps: Arc<Mutex<Vec<u8>>>,
}

impl Default for H264StreamHandler {
    fn default() -> Self {
        Self {
            packet_sender: Arc::new(Mutex::new(None)),
            sps: Arc::new(Mutex::new(Vec::new())),
            pps: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl H264StreamHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_packet_sender(&self, sender: mpsc::UnboundedSender<PacketData>) {
        *self.packet_sender.lock().unwrap() = Some(sender);
    }

    /// Set SPS and PPS data for H264 codec initialization
    pub async fn set_codec_data(&self, sps: Vec<u8>, pps: Vec<u8>) {
        *self.sps.lock().unwrap() = sps;
        *self.pps.lock().unwrap() = pps;
        log::info!("Set H264 codec data - SPS: {} bytes, PPS: {} bytes",
                   self.sps.lock().unwrap().len(),
                   self.pps.lock().unwrap().len());
    }
}

#[async_trait::async_trait]
impl TStreamHandler for H264StreamHandler {
    async fn send_prior_data(
        &self,
        data_sender: DataSender,
        _sub_type: SubscribeType,
    ) -> Result<(), StreamHubError> {
        // Send SPS (Sequence Parameter Set) first - critical for H264 decoding
        let sps_data = self.sps.lock().unwrap().clone();
        let pps_data = self.pps.lock().unwrap().clone();

        match data_sender {
            DataSender::Packet { sender } => {
                // Send SPS
                if !sps_data.is_empty() {
                    log::info!("Sending SPS data to new subscriber: {} bytes", sps_data.len());
                    let sps_packet = PacketData::Video {
                        timestamp: 0,
                        data: BytesMut::from(&sps_data[..]),
                    };
                    if let Err(err) = sender.send(sps_packet) {
                        log::error!("Failed to send SPS packet: {}", err);
                        return Err(StreamHubError {
                            value: StreamHubErrorValue::SendVideoError,
                        });
                    }
                } else {
                    log::warn!("No SPS data available for H264 stream");
                }

                // Send PPS (Picture Parameter Set) second - critical for H264 decoding
                if !pps_data.is_empty() {
                    log::info!("Sending PPS data to new subscriber: {} bytes", pps_data.len());
                    let pps_packet = PacketData::Video {
                        timestamp: 0,
                        data: BytesMut::from(&pps_data[..]),
                    };
                    if let Err(err) = sender.send(pps_packet) {
                        log::error!("Failed to send PPS packet: {}", err);
                        return Err(StreamHubError {
                            value: StreamHubErrorValue::SendVideoError,
                        });
                    }
                } else {
                    log::warn!("No PPS data available for H264 stream");
                }
            }
            _ => {
                return Err(StreamHubError {
                    value: StreamHubErrorValue::NotCorrectDataSenderType,
                });
            }
        }

        Ok(())
    }

    async fn get_statistic_data(&self) -> Option<StatisticsStream> {
        None
    }

    async fn send_information(&self, _sender: InformationSender) {}
}

/// H264 streaming server that publishes H264 files as WebRTC streams
pub struct H264StreamingServer {
    stream_hub_sender: StreamHubEventSender,
    file_path: String,
    fps: u32,
    frame_duration: Duration,
}

impl H264StreamingServer {
    pub fn new(stream_hub_sender: StreamHubEventSender, file_path: String, fps: u32) -> Self {
        Self {
            stream_hub_sender,
            file_path,
            fps,
            frame_duration: Duration::from_secs_f32(1.0 / fps as f32),
        }
    }

    /// Start streaming H264 file to the specified app and stream
    pub async fn start_streaming(
        &self,
        app_name: String,
        stream_name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!(
            "Starting H264 streaming for app: '{}', stream: '{}'",
            app_name,
            stream_name
        );

        // Read H264 file
        if !Path::new(&self.file_path).exists() {
            return Err(format!("H264 file not found: {}", self.file_path).into());
        }

        let mut file = File::open(&self.file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        log::info!(
            "Streaming H264 file: {} (size: {} bytes, {} FPS)",
            self.file_path,
            buffer.len(),
            self.fps
        );

        // Read H264 file and extract NAL units
        let mut file = File::open(&self.file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        log::info!(
            "Streaming H264 file: {} (size: {} bytes, {} FPS)",
            self.file_path,
            buffer.len(),
            self.fps
        );

        // Parse H264 NAL units
        let nal_units = H264FileReader::parse_nal_units(&buffer);
        log::info!("Found {} NAL units in H264 file", nal_units.len());

        if nal_units.is_empty() {
            return Err("No valid NAL units found in H264 file".into());
        }

        // Extract SPS and PPS NAL units for codec initialization
        let mut sps_data = Vec::new();
        let mut pps_data = Vec::new();

        for (i, nal_unit) in nal_units.iter().enumerate() {
            if nal_unit.len() >= 2 {
                let nal_header = nal_unit[0] & 0x1F; // NAL unit header (first 5 bits)
                let nal_type = nal_header & 0x1F; // NAL unit type (last 5 bits)

                match nal_type {
                    7 => { // SPS (Sequence Parameter Set)
                        log::info!("Found SPS NAL unit at index {} ({} bytes)", i, nal_unit.len());
                        sps_data = nal_unit.clone();
                    }
                    8 => { // PPS (Picture Parameter Set)
                        log::info!("Found PPS NAL unit at index {} ({} bytes)", i, nal_unit.len());
                        pps_data = nal_unit.clone();
                    }
                    5 | 1 => { // IDR frame or slice data - we have video data
                        log::debug!("Found video NAL unit type {} at index {} ({} bytes)", nal_type, i, nal_unit.len());
                        break; // Stop after finding first video data
                    }
                    _ => {
                        log::debug!("Found other NAL unit type {} at index {}", nal_type, i);
                    }
                }
            }
        }

        if !sps_data.is_empty() && !pps_data.is_empty() {
            log::info!("Extracted SPS ({} bytes) and PPS ({} bytes) for codec initialization",
                       sps_data.len(), pps_data.len());
        } else {
            log::warn!("Could not extract both SPS and PPS from H264 file");
        }

        let start_time = Instant::now();
        let mut frame_count = 0;

        // Stream NAL units with proper timing
        for (i, _nal_unit) in nal_units.iter().enumerate() {
            // Calculate when this frame should be sent
            let target_time = start_time + self.frame_duration * (i as u32);
            let now = Instant::now();

            if target_time > now {
                sleep(target_time - now).await;
            }

            // Create RTP timestamp (90kHz clock rate for H264)
            let rtp_timestamp = frame_count * 90000 / self.fps;

            // This streaming logic is now handled in the background task below
            // Note: nal_unit is unused here since streaming is handled elsewhere
            log::debug!("Would send frame {} at timestamp {}", frame_count, rtp_timestamp);

            frame_count += 1;

            // Log progress every 30 frames
            if frame_count % 30 == 0 {
                log::info!(
                    "Streamed {} frames ({}%)",
                    frame_count,
                    (frame_count * 100) / nal_units.len() as u32
                );
            }
        }

        // Create stream handler and set codec data
        let stream_handler = Arc::new(H264StreamHandler::new());
        stream_handler.set_codec_data(sps_data, pps_data).await;

        // Create publisher info
        let publisher_info = PublisherInfo {
            id: Uuid::new(RandomDigitCount::Zero),
            pub_type: PublishType::WhipPush,
            pub_data_type: streamhub::define::PubDataType::Both,
            notify_info: streamhub::define::NotifyInfo {
                request_url: format!("h264://{}", stream_name),
                remote_addr: "127.0.0.1".to_string(),
            },
        };

        // Send publish event to stream hub
        let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
        let publish_event = StreamHubEvent::Publish {
            identifier: StreamIdentifier::WebRTC {
                app_name: app_name.clone(),
                stream_name: stream_name.clone(),
            },
            result_sender,
            info: publisher_info,
            stream_handler: stream_handler.clone(),
        };

        if let Err(err) = self.stream_hub_sender.send(publish_event) {
            return Err(format!("Failed to send publish event: {}", err).into());
        }

        // Wait for publish result
        let _data_sender = match result_receiver.await {
            Ok(result) => result.map_err(|e| format!("Publish failed: {:?}", e)),
            Err(e) => Err(format!("Failed to receive publish result: {}", e)),
        }?;

        log::info!(
            "Successfully published stream: {}/{}",
            app_name,
            stream_name
        );

        // Start H264 file streaming in background
        let reader = H264FileReader::new(self.file_path.clone(), self.fps);
        let stream_hub_sender = self.stream_hub_sender.clone();
        let app_name_clone = app_name.clone();
        let stream_name_clone = stream_name.clone();

        // Create a channel for sending packets to the stream handler
        let (packet_sender_clone, mut packet_receiver) = mpsc::unbounded_channel::<PacketData>();

        // Start a task to forward packets from the channel to the stream handler
        let stream_handler_clone = stream_handler.clone();
        tokio::spawn(async move {
            while let Some(packet) = packet_receiver.recv().await {
                if let Some(sender) = stream_handler_clone.packet_sender.lock().unwrap().as_ref() {
                    if let Err(err) = sender.send(packet) {
                        log::error!("Failed to send packet to stream: {}", err);
                        break;
                    }
                }
            }
        });

        tokio::spawn(async move {
            if let Err(err) = reader.stream_file(packet_sender_clone).await {
                log::error!("H264 streaming error: {}", err);
            }

            // Cleanup: unpublish when streaming is complete
            let unpublish_event = StreamHubEvent::UnPublish {
                identifier: StreamIdentifier::WebRTC {
                    app_name: app_name_clone.clone(),
                    stream_name: stream_name_clone.clone(),
                },
                info: PublisherInfo {
                    id: Uuid::new(RandomDigitCount::Zero),
                    pub_type: PublishType::WhipPush,
                    pub_data_type: streamhub::define::PubDataType::Both,
                    notify_info: streamhub::define::NotifyInfo {
                        request_url: String::new(),
                        remote_addr: String::new(),
                    },
                },
            };

            if let Err(err) = stream_hub_sender.send(unpublish_event) {
                log::error!("Failed to unpublish stream: {}", err);
            } else {
                log::info!(
                    "Successfully unpublished stream: {}/{}",
                    app_name_clone,
                    stream_name_clone
                );
            }
        });

        Ok(())
    }
}

/// Main demo function that sets up WebRTC server and starts H264 streaming
pub async fn run_h264_streaming_demo_with_config(
    _h264_file_path: String,
    _fps: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("=== WHEP Demo Server Starting ===");

    // Create stream hub event channel
    let (stream_hub_sender, mut stream_hub_receiver) = mpsc::unbounded_channel();

    // Get server address from environment or use default
    let server_address =
        std::env::var("WRTC_SERVER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    // Start WebRTC server
    let mut webrtc_server = WebRTCServer::new(
        server_address.clone(),
        stream_hub_sender.clone(),
        None, // No authentication for demo
    );

    log::info!("Starting WebRTC server on http://{}", server_address);

    // Spawn WebRTC server task
    let webrtc_handle = tokio::spawn(async move {
        if let Err(err) = webrtc_server.run().await {
            log::error!("WebRTC server error: {}", err);
        }
    });

    // Wait a bit for server to start
    sleep(Duration::from_secs(2)).await;

    // Get H264 file path from environment or use default
    let h264_file_path =
        std::env::var("H264_FILE_PATH").unwrap_or_else(|_| "./data/test.h264".to_string());

    // Get FPS from environment or use default
    let fps = std::env::var("H264_FPS")
        .unwrap_or_else(|_| "30".to_string())
        .parse::<u32>()
        .unwrap_or(30);

    // Get stream names from environment or use defaults
    let app_name = std::env::var("WRTC_APP_NAME").unwrap_or_else(|_| "live".to_string());
    let stream_name = std::env::var("WRTC_STREAM_NAME").unwrap_or_else(|_| "test".to_string());

    // Create H264 streaming server
    let streaming_server =
        H264StreamingServer::new(stream_hub_sender.clone(), h264_file_path.clone(), fps);

    // Start streaming
    streaming_server
        .start_streaming(app_name.clone(), stream_name.clone())
        .await?;

    log::info!("=== H264 Streaming Demo Started Successfully ===");
    log::info!("Server: http://{}", server_address);
    log::info!("Stream: {}/{}", app_name, stream_name);
    log::info!("H264 File: {}", h264_file_path);
    log::info!("FPS: {}", fps);
    log::info!("");
    log::info!("To test:");
    log::info!("1. Open http://{} in your browser", server_address);
    log::info!("2. Set App Name: '{}'", app_name);
    log::info!("3. Set Stream Name: '{}'", stream_name);
    log::info!("4. Click 'Start WHEP' to begin streaming");
    log::info!("");
    log::info!("Press Ctrl+C to stop the server");

    // Handle stream hub events in background
    let stream_hub_handle = tokio::spawn(async move {
        while let Some(_event) = stream_hub_receiver.recv().await {
            log::debug!("Received stream hub event");
        }
    });

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("Received Ctrl+C, shutting down...");
        }
        _ = webrtc_handle => {
            log::info!("WebRTC server task completed");
        }
        _ = stream_hub_handle => {
            log::info!("Stream hub event handler completed");
        }
    }

    log::info!("=== WHEP Demo Server Shutdown ===");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Parse command-line arguments
    let (h264_file_path, fps) = match args.len() {
        0 | 1 => {
            // Default H264 file, default FPS
            (
                std::env::var("H264_FILE_PATH").unwrap_or_else(|_| "./data/test.h264".to_string()),
                std::env::var("H264_FPS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse::<u32>()
                    .unwrap_or(30),
            )
        }
        3 => {
            // Custom H264 file and FPS from command line
            (args[1].clone(), args[2].parse::<u32>().unwrap_or(30))
        }
        _ => {
            eprintln!("Usage: cargo run --example whep_demo [H264_FILE_PATH] [FPS]");
            eprintln!("Example: cargo run --example whep_demo ./data/test.h264 30");
            std::process::exit(1);
        }
    };

    run_h264_streaming_demo_with_config(h264_file_path, fps).await
}
