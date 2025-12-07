use crate::opus::OpusCoder;
use anyhow::Result;
use audiopus::Channels;
use log::{debug, error, info, warn};
use openh264::{decoder::Decoder, formats::YUVSource};
use std::sync::Arc;
use tokio::sync::{Notify, mpsc};
use tokio::time::Duration;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine};
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::track::track_remote::TrackRemote;

pub type RGBFrame = (u32, u32, Vec<u8>); // (width, height, rgb_data)
pub type AudioSamples = (u32, Vec<f32>); // (sample_rate, f32_samples)

// Parse H264 RTP payload according to RFC 6184
fn parse_h264_rtp_payload(payload: &[u8]) -> Vec<u8> {
    if payload.is_empty() {
        return Vec::new();
    }

    let nal_header = payload[0];
    let f_bit = (nal_header >> 7) & 0x1;
    let nal_type = nal_header & 0x1F;

    debug!("Parsing RTP payload: NAL type {}, F bit {}", nal_type, f_bit);

    match nal_type {
        // Focus on important frame types
        5 => {
            // IDR frame - key frame
            debug!("IDR frame (type 5), processing");
            let mut result = Vec::with_capacity(payload.len() + 4);
            result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            result.extend_from_slice(payload);
            result
        }

        1 => {
            // Non-IDR frame
            debug!("Non-IDR frame (type 1), processing");
            let mut result = Vec::with_capacity(payload.len() + 4);
            result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            result.extend_from_slice(payload);
            result
        }

        7 => {
            // SPS - Sequence Parameter Set
            debug!("SPS (type 7), processing");
            let mut result = Vec::with_capacity(payload.len() + 4);
            result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            result.extend_from_slice(payload);
            result
        }

        8 => {
            // PPS - Picture Parameter Set
            debug!("PPS (type 8), processing");
            let mut result = Vec::with_capacity(payload.len() + 4);
            result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            result.extend_from_slice(payload);
            result
        }

        // STAP-A contains multiple NAL units including SPS/PPS
        24 => {
            debug!("STAP-A packet, extracting NAL units");
            parse_stap_a(payload)
        }

        // Skip other types for now
        _ => {
            debug!("Skipping NAL unit type: {}", nal_type);
            Vec::new()
        }
    }
}

fn parse_stap_a(payload: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut pos = 1; // Skip STAP-A header

    debug!("STAP-A: Processing {} bytes", payload.len());

    while pos < payload.len() {
        if pos + 2 > payload.len() {
            break;
        }

        // Read NAL unit size (16 bits)
        let nal_size = u16::from_be_bytes([payload[pos], payload[pos + 1]]) as usize;
        pos += 2;

        if pos + nal_size > payload.len() {
            warn!("Invalid STAP-A NAL unit size: {}", nal_size);
            break;
        }

        // Get NAL unit type
        if nal_size > 0 {
            let nal_type = payload[pos] & 0x1F;
            debug!("STAP-A: Found NAL unit type {}, size: {}", nal_type, nal_size);
        }

        // Add start code and NAL unit
        result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        result.extend_from_slice(&payload[pos..pos + nal_size]);

        pos += nal_size;
    }

    debug!("STAP-A: Extracted {} bytes total", result.len());
    result
}

// Store FU-A fragments temporarily - this is a simple implementation
thread_local! {
    static FU_A_BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(Vec::new());
    static FU_A_STARTED: std::cell::RefCell<bool> = std::cell::RefCell::new(false);
}

fn parse_fu_a(payload: &[u8]) -> Vec<u8> {
    if payload.len() < 3 {
        return Vec::new();
    }

    let fu_header = payload[1];
    let s_bit = (fu_header >> 7) & 0x1;
    let e_bit = (fu_header >> 6) & 0x1;
    let nal_type = fu_header & 0x1F;
    let original_nal_header = (payload[0] & 0xE0) | nal_type;

    debug!("FU-A: S={}, E={}, Type={}", s_bit, e_bit, nal_type);

    FU_A_BUFFER.with(|buffer| {
        FU_A_STARTED.with(|started| {
            let mut buf = buffer.borrow_mut();
            let mut is_started = started.borrow_mut();

            if s_bit == 1 {
                // Start of new fragment
                buf.clear();
                buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                buf.push(original_nal_header);
                buf.extend_from_slice(&payload[2..]);
                *is_started = true;
                debug!("FU-A: Started new fragment");
                Vec::new() // Don't return incomplete data
            } else if *is_started {
                // Middle or end fragment
                buf.extend_from_slice(&payload[2..]);

                if e_bit == 1 {
                    // End of fragment - return complete NAL unit
                    *is_started = false;
                    let result = buf.clone();
                    buf.clear();
                    debug!("FU-A: Completed fragment, {} bytes", result.len());
                    result
                } else {
                    // Middle fragment - continue accumulating
                    Vec::new()
                }
            } else {
                // Fragment without start
                Vec::new()
            }
        })
    })
}

// H264 decoder for Annex-B format RTP streams
pub struct H264Decoder {
    decoder: Decoder,
    width: u32,
    height: u32,
    frame_count: u32,
}

impl H264Decoder {
    pub fn new() -> Result<Self> {
        let decoder = Decoder::new()
            .map_err(|e| anyhow::anyhow!("Failed to create OpenH264 decoder: {:?}", e))?;

        Ok(Self {
            decoder,
            width: 1920, // Default, will be updated when decoding first frame
            height: 1080,
            frame_count: 0,
        })
    }

    pub fn decode(&mut self, h264_data: &[u8]) -> Result<RGBFrame> {
        if h264_data.is_empty() {
            // Return empty frame if no data
            return Ok((
                self.width,
                self.height,
                vec![0u8; (self.width * self.height * 3) as usize],
            ));
        }

        // h264_data already comes from parse_h264_rtp_payload() with Annex-B start codes
        debug!("Processing pre-processed H264 data: {} bytes", h264_data.len());

        // Skip start code to get NAL header for logging
        let nal_type = if h264_data.len() >= 5 {
            h264_data[4] & 0x1F
        } else {
            debug!("H264 data too short for NAL header");
            anyhow::bail!("H264 data too short");
        };

        debug!("NAL unit type {}, size: {}", nal_type, h264_data.len());

        match self.decoder.decode(h264_data) {
            Ok(Some(yuv_frame)) => {
                info!("Successfully decoded YUV frame from NAL type {}", nal_type);

                // Convert YUV to RGB
                match Self::yuv420_to_rgb(&yuv_frame, self.width, self.height) {
                    Ok(rgb_data) => {
                        self.frame_count += 1;
                        info!("Successfully converted H264 frame to RGB: {}x{}", self.width, self.height);
                        return Ok((self.width, self.height, rgb_data));
                    }
                    Err(e) => {
                        warn!("YUV to RGB conversion error: {:?}", e);
                    }
                }
            }
            Ok(None) => {
                debug!("No frame produced from NAL type {}, continuing", nal_type);
            }
            Err(e) => {
                debug!("Decode error on NAL type {}: {:?}", nal_type, e);
            }
        }

        // If no frame decoded, return error
        anyhow::bail!("Failed to decode any H264 frame from the input data")
    }

    fn parse_stap_a_single(&self, payload: &[u8]) -> Result<Vec<Vec<u8>>> {
        if payload.is_empty() || payload[0] & 0x1F != 24 {
            return Ok(Vec::new());
        }

        debug!("STAP-A packet, extracting NAL units");
        let mut nal_units = Vec::new();
        let mut offset = 1; // Skip STAP-A header

        while offset < payload.len() {
            if offset + 2 > payload.len() {
                break;
            }

            // Read NAL unit size (16 bits, big endian)
            let nal_size = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
            offset += 2;

            if offset + nal_size > payload.len() {
                warn!("STAP-A: NAL unit size {} exceeds remaining data", nal_size);
                break;
            }

            // Extract NAL unit
            let nal_data = payload[offset..offset + nal_size].to_vec();
            let nal_type = nal_data[0] & 0x1F;
            debug!("STAP-A: Found NAL unit type {}, size: {}", nal_type, nal_size);

            nal_units.push(nal_data);
            offset += nal_size;
        }

        debug!("STAP-A: Extracted {} NAL units", nal_units.len());
        Ok(nal_units)
    }

    fn parse_annexb_nal_units(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut nal_units = Vec::new();
        let mut i = 0;

        while i < data.len() {
            // Look for NAL unit start code (0x00000001 or 0x000001)
            let start_code_len = self.find_start_code(data, i);

            if start_code_len == 0 {
                break; // No more start codes found
            }

            i += start_code_len;

            // Find next start code or end of data
            let next_start_pos = self.find_next_start_code(data, i);

            if next_start_pos > i {
                // Extract NAL unit data (excluding start code)
                let nal_data = data[i..next_start_pos].to_vec();
                if !nal_data.is_empty() {
                    nal_units.push(nal_data);
                }
            }

            i = next_start_pos;
        }

        nal_units
    }

    fn find_start_code(&self, data: &[u8], start_pos: usize) -> usize {
        let pos = start_pos;
        let remaining = data.len() - pos;

        if remaining < 3 {
            return 0;
        }

        // Check for 4-byte start code (0x00000001)
        if remaining >= 4
            && data[pos] == 0
            && data[pos + 1] == 0
            && data[pos + 2] == 0
            && data[pos + 3] == 1
        {
            return 4;
        }

        // Check for 3-byte start code (0x000001)
        if data[pos] == 0 && data[pos + 1] == 0 && data[pos + 2] == 1 {
            return 3;
        }

        0
    }

    fn find_next_start_code(&self, data: &[u8], start_pos: usize) -> usize {
        let mut pos = start_pos;
        let mut remaining = data.len() - pos;

        while remaining >= 3 {
            // Look for potential start code pattern
            if data[pos] == 0 && data[pos + 1] == 0 {
                if pos + 2 < data.len() && data[pos + 2] == 0 {
                    if pos + 3 < data.len() && data[pos + 3] == 1 {
                        return pos; // Found 4-byte start code
                    }
                    pos += 1;
                } else if pos + 2 < data.len() && data[pos + 2] == 1 {
                    return pos; // Found 3-byte start code
                } else {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
            remaining = data.len() - pos;
        }

        data.len() // No more start codes, return end of data
    }

    // Parse H264 RTP payload according to RFC 6184
  
    fn yuv420_to_rgb(
        yuv_frame: &openh264::decoder::DecodedYUV,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>> {
        // Get YUV plane data
        let y_plane = yuv_frame.y();
        let u_plane = yuv_frame.u();
        let v_plane = yuv_frame.v();

        let width_usize = width as usize;
        let height_usize = height as usize;
        let mut rgb_data = vec![0u8; width_usize * height_usize * 3];

        // Debug: Check if we have valid YUV data
        info!("YUV420 to RGB conversion: {}x{}", width, height);
        info!("Y plane size: {}, U plane size: {}, V plane size: {}",
              y_plane.len(), u_plane.len(), v_plane.len());

        // Sample first few YUV values
        if y_plane.len() > 10 {
            info!("First 10 Y values: {:?}", &y_plane[..10]);
        }
        if u_plane.len() > 10 {
            info!("First 10 U values: {:?}", &u_plane[..10]);
        }
        if v_plane.len() > 10 {
            info!("First 10 V values: {:?}", &v_plane[..10]);
        }

        // Simple YUV420 to RGB conversion
        let mut non_zero_pixels = 0usize; // Use usize to avoid overflow
        let mut max_y = 0u8;
        let mut min_y = 255u8;

        for y in 0..height_usize {
            for x in 0..width_usize {
                let y_idx = y * width_usize + x;
                let uv_y = y / 2;
                let uv_x = x / 2;

                let y_val = y_plane[y_idx] as f32;
                let u_val = u_plane[uv_y * width_usize / 2 + uv_x] as f32 - 128.0;
                let v_val = v_plane[uv_y * width_usize / 2 + uv_x] as f32 - 128.0;

                // Track Y range
                let y_byte = y_plane[y_idx];
                max_y = max_y.max(y_byte);
                min_y = min_y.min(y_byte);

                // YUV to RGB conversion (BT.601)
                let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
                let g = (y_val - 0.344 * u_val - 0.714 * v_val).clamp(0.0, 255.0) as u8;
                let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

                let rgb_idx = y_idx * 3;
                rgb_data[rgb_idx] = r;
                rgb_data[rgb_idx + 1] = g;
                rgb_data[rgb_idx + 2] = b;

                if r > 0 || g > 0 || b > 0 {
                    non_zero_pixels += 1;
                }
            }
        }

        info!("YUV analysis - Y range: {} to {}, Non-zero RGB pixels: {}", min_y, max_y, non_zero_pixels);

        Ok(rgb_data)
    }

  }

pub struct WHEPClient {
    pub video_sender: mpsc::UnboundedSender<RGBFrame>,
    pub audio_sender: mpsc::UnboundedSender<AudioSamples>,
}

impl WHEPClient {
    pub fn new() -> (
        Self,
        mpsc::UnboundedReceiver<RGBFrame>,
        mpsc::UnboundedReceiver<AudioSamples>,
    ) {
        let (video_tx, video_rx) = mpsc::unbounded_channel();
        let (audio_tx, audio_rx) = mpsc::unbounded_channel();

        (
            Self {
                video_sender: video_tx,
                audio_sender: audio_tx,
            },
            video_rx,
            audio_rx,
        )
    }
}

async fn process_video_track(
    track: Arc<TrackRemote>,
    video_sender: mpsc::UnboundedSender<RGBFrame>,
    notify: Arc<Notify>,
) -> Result<()> {
    let mut h264_decoder = match H264Decoder::new() {
        Ok(decoder) => decoder,
        Err(e) => {
            error!("Failed to create H264 decoder: {}", e);
            return Ok(());
        }
    };

    info!("H264 video decoder initialized");

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    // Extract H264 payload from RTP packet
                    let payload = rtp_packet.payload;
                    debug!("Received H264 RTP packet: {} bytes payload", payload.len());

                    // Debug: Print first few bytes to understand the format
                    if payload.len() > 0 {
                        let first_bytes = &payload[..payload.len().min(10)];
                        debug!("First bytes: {:?}", first_bytes);

                        // Check if this looks like RTP payload format (RFC 6184)
                        if payload.len() >= 2 {
                            let nal_header = payload[0];
                            let f_bit = (nal_header >> 7) & 0x1;
                            let nri = (nal_header >> 5) & 0x3;
                            let nal_type = nal_header & 0x1F;
                            debug!("NAL header: 0x{:02x} (F={}, NRI={}, Type={})", nal_header, f_bit, nri, nal_type);
                        }
                    }

                    // Parse H264 RTP payload according to RFC 6184
                    let h264_data = parse_h264_rtp_payload(&payload);

                    // Decode H264 packet to RGB frame
                    match h264_decoder.decode(&h264_data) {
                        Ok(rgb_frame) => {
                            if video_sender.send(rgb_frame).is_err() {
                                info!("Video channel closed, stopping video processing");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("H264 decoding error: {}", e);
                            // Continue processing even if a frame fails to decode
                        }
                    }
                } else {
                    info!("Video track ended");
                    return Ok(());
                }
            }
            _ = notify.notified() => {
                info!("Video processing stopped by notification");
                return Ok(());
            }
        }
    }
    Ok(())
}

async fn process_audio_track(
    track: Arc<TrackRemote>,
    audio_sender: mpsc::UnboundedSender<AudioSamples>,
    notify: Arc<Notify>,
) -> Result<()> {
    let mut opus_decoder = match OpusCoder::new(48000, Channels::Stereo) {
        Ok(decoder) => decoder,
        Err(e) => {
            error!("Failed to create Opus decoder: {}", e);
            return Ok(());
        }
    };

    info!("Opus audio decoder initialized");

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    // Extract Opus payload from RTP packet
                    let payload = rtp_packet.payload;

                    // Decode Opus packet to f32 audio samples
                    match opus_decoder.decode(&payload) {
                        Ok(audio_samples) => {
                            let audio_data: AudioSamples = (48000, audio_samples);
                            if audio_sender.send(audio_data).is_err() {
                                info!("Audio channel closed, stopping audio processing");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Opus decoding error: {}", e);
                            // Continue processing even if a packet fails to decode
                        }
                    }
                } else {
                    info!("Audio track ended");
                    return Ok(());
                }
            }
            _ = notify.notified() => {
                info!("Audio processing stopped by notification");
                return Ok(());
            }
        }
    }
    Ok(())
}

impl WHEPClient {
    pub async fn connect(&self, server_url: &str) -> Result<()> {
        // Everything below is the WebRTC-rs API! Thanks for using it ❤️.

        // Create a MediaEngine object to configure the supported codec
        let mut m = MediaEngine::default();

        // Setup the codecs you want to use.
        // We'll use a H264 and Opus but you can also define your own
        m.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_H264.to_owned(),
                    clock_rate: 90000,
                    channels: 0,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                payload_type: 102,
                ..Default::default()
            },
            RTPCodecType::Video,
        )?;

        m.register_codec(
            RTCRtpCodecParameters {
                capability: RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_OPUS.to_owned(),
                    clock_rate: 48000,
                    channels: 2,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                payload_type: 111,
                ..Default::default()
            },
            RTPCodecType::Audio,
        )?;

        // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
        let mut registry = Registry::new();

        // Use the default set of Interceptors
        registry = register_default_interceptors(registry, &mut m)?;

        // Create the API object with the MediaEngine
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        // Prepare the configuration
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);

        // Allow us to receive 1 audio track, and 1 video track
        peer_connection
            .add_transceiver_from_kind(RTPCodecType::Audio, None)
            .await?;
        peer_connection
            .add_transceiver_from_kind(RTPCodecType::Video, None)
            .await?;

        let notify_tx = Arc::new(Notify::new());
        let notify_rx = notify_tx.clone();

        let video_sender = self.video_sender.clone();
        let audio_sender = self.audio_sender.clone();

        // Set a handler for when a new remote track starts
        let pc = Arc::downgrade(&peer_connection);
        peer_connection.on_track(Box::new(move |track, _, _| {
            // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
            let media_ssrc = track.ssrc();
            let pc2 = pc.clone();
            tokio::spawn(async move {
                let mut result = Result::<usize>::Ok(0);
                while result.is_ok() {
                    let timeout = tokio::time::sleep(Duration::from_secs(3));
                    tokio::pin!(timeout);

                    tokio::select! {
                        _ = timeout.as_mut() =>{
                            if let Some(pc) = pc2.upgrade(){
                                result = pc.write_rtcp(&[Box::new(PictureLossIndication{
                                    sender_ssrc: 0,
                                    media_ssrc,
                                })]).await.map_err(Into::into);
                            }else {
                                break;
                            }
                        }
                    };
                }
            });

            let notify_rx2 = Arc::clone(&notify_rx);
            let video_sender2 = video_sender.clone();
            let audio_sender2 = audio_sender.clone();

            Box::pin(async move {
                let codec = track.codec();
                let mime_type = codec.capability.mime_type.to_lowercase();
                if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                    info!("Got Opus track, processing audio");
                    tokio::spawn(async move {
                        let _ = process_audio_track(track, audio_sender2, notify_rx2).await;
                    });
                } else if mime_type == MIME_TYPE_H264.to_lowercase() {
                    info!("Got H264 track, processing video");
                    tokio::spawn(async move {
                        let _ = process_video_track(track, video_sender2, notify_rx2).await;
                    });
                }
            })
        }));

        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Set the handler for ICE connection state
        let notify_tx2 = notify_tx.clone();
        peer_connection.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                debug!("Connection State has changed {connection_state}");

                if connection_state == RTCIceConnectionState::Connected {
                    info!("WHEP client connected to server");
                } else if connection_state == RTCIceConnectionState::Failed {
                    notify_tx2.notify_waiters();
                    error!("WHEP client connection failed");

                    let _ = done_tx.try_send(());
                }
                Box::pin(async {})
            },
        ));

        // Implement WHEP protocol: HTTP POST with SDP offer
        info!("WHEP client connecting to: {}", server_url);

        // Create offer
        let offer = peer_connection.create_offer(None).await?;

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        peer_connection.set_local_description(offer).await?;

        // Block until ICE Gathering is complete, disabling trickle ICE
        let _ = gather_complete.recv().await;

        // Get the local description to send to server
        if let Some(local_desc) = peer_connection.local_description().await {
            // Send SDP offer to WHEP server via HTTP POST
            let client = reqwest::Client::new();
            let offer_sdp = local_desc.sdp;

            info!("Sending WHEP request to: {}/whep", server_url);
            debug!("SDP Offer length: {} bytes", offer_sdp.len());

            // Log headers before sending for debugging
            debug!("Setting HTTP headers:");
            debug!("  Content-Type: application/sdp");
            debug!("  Authorization: Bearer 123");

            let response = client
                .post(&format!("{}/whep", server_url))
                .header("Content-Type", "application/sdp")
                .header("Authorization", "Bearer 123")
                .body(offer_sdp)
                .send()
                .await?;

            info!("WHEP response status: {}", response.status());

            if response.status().is_success() {
                let answer_sdp = response.text().await?;
                debug!("SDP Answer received, length: {} bytes", answer_sdp.len());

                // Parse SDP answer
                let answer = RTCSessionDescription::answer(answer_sdp)?;

                // Set the remote description (server's answer)
                peer_connection.set_remote_description(answer).await?;
                info!("WHEP connection established successfully");
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                anyhow::bail!("WHEP server returned error: {} - {}", status, error_text);
            }
        } else {
            anyhow::bail!("Failed to generate local description");
        }

        // Wait for connection to be established or fail
        tokio::select! {
            _ = done_rx.recv() => {
                info!("WHEP client connection ended");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("WHEP client stopped by user");
            }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                info!("WHEP client connection timeout after 60 seconds");
            }
        };

        peer_connection.close().await?;
        Ok(())
    }
}

#[cfg(test)]
pub async fn test_whep_client() -> Result<()> {
    let (client, mut video_rx, mut audio_rx) = WHEPClient::new();

    // Spawn a task to handle incoming video frames
    let video_task = tokio::spawn(async move {
        while let Some((width, height, rgb_data)) = video_rx.recv().await {
            info!(
                "Received video frame: {}x{} ({} bytes)",
                width,
                height,
                rgb_data.len()
            );
        }
        info!("Video channel closed");
    });

    // Spawn a task to handle incoming audio samples
    let audio_task = tokio::spawn(async move {
        while let Some((sample_rate, samples)) = audio_rx.recv().await {
            debug!(
                "Received audio samples: {} Hz, {} samples",
                sample_rate,
                samples.len()
            );
        }
        info!("Audio channel closed");
    });

    // Connect to WHEP server (replace with actual server URL)
    let server_url = "http://localhost:9090/whep";
    info!("Connecting to WHEP server: {}", server_url);

    if let Err(e) = client.connect(server_url).await {
        error!("Failed to connect to WHEP server: {}", e);
    }

    // Wait for tasks to complete
    let _ = tokio::join!(video_task, audio_task);

    Ok(())
}
